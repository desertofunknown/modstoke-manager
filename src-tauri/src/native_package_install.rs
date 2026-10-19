use super::native_archive::{NativeArchive, NativeManifest};
use super::*;
use crate::native_inventory;
use std::collections::{BTreeSet, VecDeque};
use std::time::{SystemTime, UNIX_EPOCH};

pub(super) const PROVIDER: &str = "modstoke-native-local";
pub(super) const DEPENDENCY_BASIS: &str = "modstoke-native-required-versions";

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NativePackagePreview {
    archive: NativeArchive,
    action: String,
    profile_revision: Option<String>,
    plan_id: Option<String>,
    notices: Vec<String>,
}

struct Installed {
    manifest: NativeManifest,
    observation: InstallObservation,
}

pub(super) struct Prepared {
    pub(super) steps: Vec<InstallStep>,
    pub(super) observed: Vec<(PathBuf, String)>,
    pub(super) action: String,
}

pub(super) fn same<T: Serialize>(left: &T, right: &T) -> Result<bool, String> {
    Ok(serde_json::to_value(left).map_err(|e| e.to_string())?
        == serde_json::to_value(right).map_err(|e| e.to_string())?)
}

pub(super) fn require_stopped() -> Result<(), String> {
    if crate::client_admission::game_process_running()? {
        return Err(
            "Close Sons of the Forest and its dedicated server before changing native packages."
                .into(),
        );
    }
    Ok(())
}

pub(super) fn validate_package_record(package: &ProfilePackage) -> Result<(), String> {
    if package.dependency_basis != DEPENDENCY_BASIS {
        return if package.native.is_none() && package.source.provider != PROVIDER {
            Ok(())
        } else {
            Err(invalid_profile())
        };
    }
    if package.source.provider == "installed" && package.native.is_none() {
        return Ok(());
    }
    let archive = package.native.as_ref().ok_or_else(invalid_profile)?;
    native_archive::validate_record(archive)?;
    let expected: HashSet<_> = archive.required.iter().map(|dep| key(&dep.id)).collect();
    if package.source.provider != PROVIDER
        || package.source.author.is_some()
        || package.source.slug.is_some()
        || package.source.url.as_deref() != Some(format!("urn:sha256:{}", archive.sha256).as_str())
        || package.id != archive.id
        || package.name != archive.name
        || package.version != archive.version
        || package.ownership != "native"
        || !package.enabled
        || package
            .archive_sha256
            .as_ref()
            .is_some_and(|value| value != &archive.sha256)
        || package.dependencies.len() != expected.len()
        || package
            .dependencies
            .iter()
            .map(|dep| key(dep))
            .collect::<HashSet<_>>()
            != expected
    {
        return Err(invalid_profile());
    }
    Ok(())
}

// Catalog callers may carry native records through a profile, but cannot replace
// their provenance, file ownership, pending plan or exact dependency contract.
pub(super) fn require_catalog_write(
    current: &ProfileState,
    proposed: &ModProfile,
) -> Result<(), String> {
    let is_native = |package: &ProfilePackage| package.dependency_basis == DEPENDENCY_BASIS;
    if proposed
        .pending_plan
        .as_ref()
        .is_some_and(|plan| plan.steps.iter().any(|step| is_native(&step.package)))
        || current
            .profile
            .as_ref()
            .and_then(|profile| profile.pending_plan.as_ref())
            .is_some_and(|plan| plan.steps.iter().any(|step| is_native(&step.package)))
    {
        return Err(
            "Resume native package installation from Native packages, or use Pack recovery.".into(),
        );
    }
    let old: Vec<_> = current
        .profile
        .as_ref()
        .map(|profile| {
            profile
                .packages
                .iter()
                .filter(|pkg| is_native(pkg))
                .cloned()
                .collect()
        })
        .unwrap_or_default();
    let new: Vec<_> = proposed
        .packages
        .iter()
        .filter(|pkg| is_native(pkg))
        .cloned()
        .collect();
    if !same(&old, &new)?
        || proposed.pending_plan.as_ref().is_some_and(|plan| {
            plan.steps
                .iter()
                .any(|step| old.iter().any(|pkg| key(&pkg.id) == key(&step.package.id)))
        })
    {
        return Err("A catalog package conflicts with a recorded native mod ID. Its native package and profile were preserved.".into());
    }
    Ok(())
}

fn read_bytes(root: &Path, relative: &Path) -> Result<Vec<u8>, String> {
    if !mod_update::validate_target(root, relative)? {
        return Err(format!("Missing native manifest: {}", relative.display()));
    }
    let mut bytes = Vec::new();
    File::open(root.join(relative))
        .map_err(|e| e.to_string())?
        .take(512 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > 512 * 1024 {
        return Err("A native manifest exceeds 512 KiB.".into());
    }
    Ok(bytes)
}

pub(super) fn known_package<'a>(profile: Option<&'a ModProfile>, id: &str) -> Option<&'a ProfilePackage> {
    profile.and_then(|profile| profile.packages.iter().find(|pkg| key(&pkg.id) == key(id)))
}

pub(super) fn check_owned(root: &Path, archive: &NativeArchive) -> Result<(), String> {
    for file in &archive.files {
        let path = Path::new(&file.path);
        mod_update::ensure_unmanaged(root, path)?;
        if !mod_update::is_native_user_file(path)
            && mod_update::target_hash(root, path)?.as_deref() != Some(&file.sha256)
        {
            return Err(format!("{} changed or is missing since Manager installed it. Its files were preserved; resolve the change before updating.", file.path));
        }
    }
    Ok(())
}

fn require_empty_directory_tree(path: &Path) -> Result<(), String> {
    let mut pending = vec![path.to_owned()];
    let mut count = 0;
    while let Some(folder) = pending.pop() {
        mod_update::validate_directory(&folder)?;
        for entry in fs::read_dir(folder).map_err(|error| error.to_string())? {
            let entry = entry.map_err(|error| error.to_string())?;
            count += 1;
            if count > 8192
                || !entry
                    .file_type()
                    .map_err(|error| error.to_string())?
                    .is_dir()
            {
                return Err("An interrupted native install destination contains unowned files. Its contents were preserved.".into());
            }
            pending.push(entry.path());
        }
    }
    Ok(())
}

fn prepare(
    root: &Path,
    profile: Option<&ModProfile>,
    archive: &NativeArchive,
) -> Result<Prepared, String> {
    prepare_many(root, profile, std::slice::from_ref(archive))
}

pub(super) fn prepare_many(
    root: &Path,
    profile: Option<&ModProfile>,
    archives: &[NativeArchive],
) -> Result<Prepared, String> {
    if archives.is_empty() || archives.len() > 128 {
        return Err("Select 1–128 native archives for dependency planning.".into());
    }
    let mut selected = HashMap::new();
    let mut folders = HashSet::new();
    for archive in archives {
        native_archive::validate_record(archive)?;
        if selected.insert(key(&archive.id), archive).is_some() {
            return Err(format!("Native package ID {} is selected more than once.", archive.id));
        }
        if !folders.insert(key(&archive.package_path)) {
            return Err(format!("Multiple selected native packages use destination {}.", archive.package_path));
        }
    }
    let inventory = native_inventory::read_inventory(&root.to_string_lossy())?;
    if inventory
        .notices
        .iter()
        .any(|notice| notice != "The OpenACAI/Mods folder does not exist.")
    {
        return Err(format!(
            "Native inventory is incomplete: {}",
            inventory.notices.join(" ")
        ));
    }
    if let Some(profile) = profile {
        rollback::require_forward(profile)?;
    }
    let mut actions = HashMap::new();
    for archive in archives {
        if let Some(profile) = profile {
            if profile.archives.iter().any(|pin| {
                key(&pin.id) == key(&archive.id)
                    && (!pin.url.starts_with("urn:sha256:")
                        || pin.version == archive.version && pin.sha256 != archive.sha256)
            }) {
                return Err("This mod identity/version conflicts with the profile's recorded archive provenance. Its existing history was preserved.".into());
            }
        }
        let matching: Vec<_> = inventory
            .packages
            .iter()
            .filter(|pkg| key(&pkg.id) == key(&archive.id))
            .collect();
        if matching.len() > 1 {
            return Err("Multiple native manifests declare this package ID. Resolve the ambiguity before installing.".into());
        }
        let known = known_package(profile, &archive.id);
        let old_archive = known.and_then(|pkg| pkg.native.as_ref());
        let action;
        if let Some(existing) = matching.first() {
            let old = old_archive.ok_or("This native package is not owned by Manager. Its manual or externally managed files were preserved.")?;
            if existing.state != "candidate"
                || old.package_path != archive.package_path
                || relative(root, &existing.package_path)? != Path::new(&old.package_path)
                || relative(root, &existing.manifest_path)? != Path::new(&old.manifest_path)
                || existing.version.as_deref() != Some(&old.version)
            {
                return Err("The installed native package no longer matches its enabled Manager record or destination. Refresh and resolve the conflict before updating.".into());
            }
            check_owned(root, old)?;
            action = "update";
        } else {
            if known.is_some() {
                return Err("This package ID is already recorded in the profile but has no matching native installation. Its records were preserved.".into());
            }
            match fs::symlink_metadata(root.join(&archive.package_path)) {
                Ok(_) if profile.and_then(|profile| profile.pending_plan.as_ref()).is_some_and(|plan|
                    plan.steps.iter().any(|step| step.action == "install" && key(&step.package.id) == key(&archive.id)
                        && step.package.native.as_ref().is_some_and(|saved| saved.sha256 == archive.sha256))) => {
                    require_empty_directory_tree(&root.join(&archive.package_path))?;
                }
                Ok(_) => return Err("The native package destination already exists without Manager ownership. Its contents were preserved.".into()),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error.to_string()),
            }
            action = "install";
        }
        let old_files: HashMap<_, _> = old_archive
            .into_iter()
            .flat_map(|old| &old.files)
            .map(|file| (key(&file.path), file))
            .collect();
        for file in &archive.files {
            let path = Path::new(&file.path);
            mod_update::ensure_unmanaged(root, path)?;
            if let Some(current) = mod_update::target_hash(root, path)? {
                if !mod_update::is_native_user_file(path)
                    && old_files
                        .get(&key(&file.path))
                        .is_none_or(|old| old.sha256 != current)
                {
                    return Err(format!("The package would replace an unowned or modified file at {}. It was preserved.", file.path));
                }
            }
        }
        actions.insert(key(&archive.id), action);
    }

    let mut installed = HashMap::<String, Installed>::new();
    let mut observed = Vec::new();
    let mut manifest_bytes = 0usize;
    for entry in &inventory.packages {
        if entry.package_type == "library"
            || entry.entry_type.is_none()
            || entry.state == "disabled"
        {
            continue;
        }
        if entry.state != "candidate" {
            return Err(format!(
                "Native package {} needs attention before dependency planning.",
                entry.id
            ));
        }
        let path = relative(root, &entry.manifest_path)?;
        let bytes = read_bytes(root, &path)?;
        manifest_bytes += bytes.len();
        if manifest_bytes > 32 * 1024 * 1024 {
            return Err("Native dependency manifests exceed the planning limit.".into());
        }
        let manifest = native_archive::parse_manifest(&bytes).map_err(|error| {
            format!("Cannot plan native dependencies for {}: {error}", entry.id)
        })?;
        let assembly_path = relative(
            root,
            entry
                .assembly_path
                .as_deref()
                .ok_or("A native assembly is missing.")?,
        )?;
        let declared = relative(root, &entry.package_path)?.join(&manifest.assembly);
        if key(&declared.to_string_lossy()) != key(&assembly_path.to_string_lossy())
            || manifest.id != entry.id
            || Some(manifest.version.as_str()) != entry.version.as_deref()
        {
            return Err(format!("Native package {} changed or resolves an unexpected assembly. Refresh before installing.", entry.id));
        }
        observed.push((path, hash(&bytes)));
        observed.push((
            assembly_path.clone(),
            mod_update::target_hash(root, &assembly_path)?
                .ok_or("A native assembly disappeared.")?,
        ));
        let record = known_package(profile, &manifest.id);
        if record.is_some_and(|pkg| pkg.dependency_basis != DEPENDENCY_BASIS) {
            return Err(format!(
                "Native ID {} conflicts with a legacy catalog profile record.",
                manifest.id
            ));
        }
        let observation = InstallObservation {
            id: manifest.id.clone(),
            version: manifest.version.clone(),
            enabled: true,
            ownership: if record.is_some_and(|pkg| pkg.native.is_some()) {
                "native"
            } else {
                "manual"
            }
            .into(),
            loader_type: "modstoke-native-mod".into(),
            manifest_path: Some(entry.manifest_path.clone()),
            package_path: Some(entry.package_path.clone()),
            assembly_path: entry.assembly_path.clone(),
        };
        if installed
            .insert(
                key(&manifest.id),
                Installed {
                    manifest,
                    observation,
                },
            )
            .is_some()
        {
            return Err("Ambiguous native mod identities prevent dependency planning.".into());
        }
    }
    let mut edges: HashMap<String, Vec<String>> = HashMap::new();
    let version = |id: &str| -> Option<&str> {
        if let Some(archive) = selected.get(id) {
            Some(&archive.version)
        } else {
            installed.get(id).map(|node| node.manifest.version.as_str())
        }
    };
    for (id, required) in installed
        .iter()
        .filter(|(id, _)| !selected.contains_key(*id))
        .map(|(id, node)| (id.clone(), &node.manifest.required))
        .chain(archives.iter().map(|archive| (key(&archive.id), &archive.required)))
    {
        let mut dependencies = Vec::new();
        for dependency in required {
            let dependency_key = key(&dependency.id);
            if version(&dependency_key) != Some(dependency.version.as_str()) {
                return Err(format!("{} requires native mod {} at exactly {}. Install a compatible dependency first; this package was not applied.", id, dependency.id, dependency.version));
            }
            dependencies.push(dependency_key);
        }
        edges.insert(id, dependencies);
    }
    // Include reverse consumers in cycle/version checks so an update cannot
    // silently invalidate an already installed mod's declared requirements.
    let mut remaining = edges.clone();
    let mut ordered = Vec::new();
    while !remaining.is_empty() {
        let ready: BTreeSet<_> = remaining
            .iter()
            .filter(|(_, deps)| deps.is_empty())
            .map(|(id, _)| id.clone())
            .collect();
        if ready.is_empty() {
            return Err(
                "The native mod dependency graph contains a cycle. Resolve it before installing."
                    .into(),
            );
        }
        for id in &ready {
            remaining.remove(id);
            ordered.push(id.clone());
        }
        for deps in remaining.values_mut() {
            deps.retain(|id| !ready.contains(id));
        }
    }
    let mut required = HashSet::new();
    let mut queue: VecDeque<_> = archives.iter().map(|archive| key(&archive.id)).collect();
    while let Some(id) = queue.pop_front() {
        if required.insert(id.clone()) {
            queue.extend(edges[&id].iter().cloned());
        }
    }
    if required.len() > 128 {
        return Err("The native package dependency plan exceeds 128 mods.".into());
    }
    let mut steps = Vec::new();
    for id in ordered.into_iter().filter(|id| required.contains(id)) {
        let canonical_id = |dep: &String| {
            if let Some(archive) = selected.get(dep) {
                archive.id.clone()
            } else {
                installed[dep].manifest.id.clone()
            }
        };
        let dependencies = edges[&id].iter().map(canonical_id).collect();
        if let Some(archive) = selected.get(&id) {
            steps.push(InstallStep {
                package: ProfilePackage {
                    id: archive.id.clone(),
                    name: archive.name.clone(),
                    version: archive.version.clone(),
                    dependencies,
                    dependency_basis: DEPENDENCY_BASIS.into(),
                    source: PackageSource {
                        provider: PROVIDER.into(),
                        author: None,
                        slug: None,
                        url: Some(format!("urn:sha256:{}", archive.sha256)),
                    },
                    ownership: "native".into(),
                    enabled: true,
                    archive_sha256: None,
                    native: Some((*archive).clone()),
                },
                action: actions[&id].into(),
                before: installed.get(&id).map(|node| node.observation.clone()),
                status: "planned".into(),
                targets: None,
            });
        } else {
            let node = &installed[&id];
            let package = if let Some(known) = known_package(profile, &node.manifest.id) {
                if known.version != node.manifest.version {
                    return Err("A native dependency differs from its recorded version.".into());
                }
                if let Some(owned) = known.native.as_ref() {
                    check_owned(root, owned)?;
                }
                let mut copy = known.clone();
                copy.dependencies = dependencies;
                copy
            } else {
                ProfilePackage {
                    id: node.manifest.id.clone(),
                    name: node.manifest.name.clone(),
                    version: node.manifest.version.clone(),
                    dependencies,
                    dependency_basis: DEPENDENCY_BASIS.into(),
                    source: PackageSource {
                        provider: "installed".into(),
                        author: None,
                        slug: None,
                        url: None,
                    },
                    ownership: "manual".into(),
                    enabled: true,
                    archive_sha256: None,
                    native: None,
                }
            };
            steps.push(InstallStep {
                package,
                action: "preserve".into(),
                before: Some(node.observation.clone()),
                status: "planned".into(),
                targets: None,
            });
        }
    }
    Ok(Prepared {
        steps,
        observed,
        action: actions[&key(&archives[0].id)].into(),
    })
}

fn pending_archive(profile: &ModProfile) -> Result<&NativeArchive, String> {
    let plan = profile.pending_plan.as_ref().ok_or_else(invalid_profile)?;
    if plan.native_pack.is_some() {
        return Err("Resume this plan from Portable native packs, or use Pack recovery.".into());
    }
    let changed: Vec<_> = plan
        .steps
        .iter()
        .filter(|step| step.action != "preserve")
        .collect();
    if changed.len() != 1
        || plan
            .steps
            .iter()
            .any(|step| step.package.dependency_basis != DEPENDENCY_BASIS)
        || changed[0].package.source.provider != PROVIDER
    {
        return Err(
            "Finish the existing catalog plan or use Pack recovery before installing a native ZIP."
                .into(),
        );
    }
    changed[0]
        .package
        .native
        .as_ref()
        .ok_or_else(invalid_profile)
}

pub(super) fn require_observed(root: &Path, observed: &[(PathBuf, String)]) -> Result<(), String> {
    for (path, expected) in observed {
        if mod_update::target_hash(root, path)?.as_ref() != Some(expected) {
            return Err(format!(
                "{} changed during native package preparation. Refresh before retrying.",
                path.display()
            ));
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn inspect_native_package(
    game_root: String,
    source: String,
) -> Result<NativePackagePreview, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let root = profile_root(&game_root, true)?;
        require_stopped()?;
        let _lock = mod_update::acquire_update_lock(&root)?;
        mod_update::require_no_pending_updates(&root)?;
        let state = read_locked(&root)?;
        let archive = native_archive::inspect(Path::new(&source))?;
        let plan_id = state.profile.as_ref().and_then(|profile| profile.pending_plan.as_ref()).map(|plan| plan.id.clone());
        if let Some(profile) = state.profile.as_ref().filter(|profile| profile.pending_plan.is_some()) {
            rollback::require_forward(profile)?;
            if !same(pending_archive(profile)?, &archive)? { return Err("Select the exact ZIP recorded by the unfinished native package plan.".into()); }
        }
        let prepared = prepare(&root, state.profile.as_ref(), &archive)?;
        Ok(NativePackagePreview { archive, action: if plan_id.is_some() { "resume".into() } else { prepared.action },
            profile_revision: state.revision, plan_id,
            notices: vec!["Exact required versions are present. This checks files and declarations, not mod execution or dedicated-server compatibility.".into(),
                "Existing config/userdata folders and .cfg/.ini files are preserved. Other modified or unowned files stop an update.".into(),
                "Starting this install replaces the previous completed pack recovery point.".into()] })
    }).await.map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn install_native_package(
    game_root: String,
    source: String,
    expected_revision: Option<String>,
    expected_sha256: String,
    expected_plan_id: Option<String>,
) -> Result<ProfileState, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let root = profile_root(&game_root, true)?;
        require_stopped()?;
        let _lock = mod_update::acquire_update_lock(&root)?;
        let current = read_locked(&root)?;
        require_revision(&current, &expected_revision)?;
        if current.profile.as_ref().and_then(|profile| profile.pending_plan.as_ref()).map(|plan| &plan.id) != expected_plan_id.as_ref() {
            return Err("The pending install changed. Select the ZIP again to review its current plan.".into());
        }
        if let Some(profile) = &current.profile { rollback::require_forward(profile)?; }
        mod_update::recover_pending_updates(&root)?;
        let current = reconcile(&root, current)?;
        // Copy into a private temporary before validating. The later extraction
        // is bound to these exact archive bytes and the per-file descriptor.
        let snapshot = tempfile::Builder::new().prefix(".modstoke-profile-").tempfile_in(&root).map_err(|error| error.to_string())?;
        let source_file = File::open(&source).map_err(|error| error.to_string())?;
        if source_file.metadata().map_err(|error| error.to_string())?.len() > 256 * 1024 * 1024 { return Err("The native archive exceeds 256 MiB.".into()); }
        let mut limited = source_file.take(256 * 1024 * 1024 + 1);
        let mut output = snapshot.as_file();
        if std::io::copy(&mut limited, &mut output).map_err(|error| error.to_string())? > 256 * 1024 * 1024 { return Err("The native archive exceeds 256 MiB.".into()); }
        snapshot.as_file().sync_all().map_err(|error| error.to_string())?;
        let snapshot = snapshot.into_temp_path();
        let snapshot_path: &Path = snapshot.as_ref();
        let archive = native_archive::inspect(snapshot_path)?;
        if archive.sha256 != expected_sha256 { return Err("The selected ZIP changed since review. Select it again before installing.".into()); }
        let prepared = prepare(&root, current.profile.as_ref(), &archive)?;
        let mut profile = current.profile.clone().unwrap_or(ModProfile { schema_version: 1, game_root: root.to_string_lossy().into_owned(),
            requested_roots: Vec::new(), packages: Vec::new(), archives: Vec::new(), pending_plan: None, rollback: None });
        if profile.pending_plan.is_some() {
            if !same(pending_archive(&profile)?, &archive)? { return Err("The ZIP does not match the unfinished native plan.".into()); }
            let old = &profile.pending_plan.as_ref().expect("plan").steps;
            if old.len() != prepared.steps.len() {
                return Err("The installed native dependencies changed since this plan began. Restore them or roll back the pending plan.".into());
            }
            for (a, b) in old.iter().zip(&prepared.steps) {
                if key(&a.package.id) != key(&b.package.id) || a.package.version != b.package.version
                    || a.package.dependencies != b.package.dependencies
                    || (a.status != "applied" || a.action == "preserve") && !same(&a.before, &b.before)? {
                    return Err("The installed native dependencies changed since this plan began. Restore them or roll back the pending plan.".into());
                }
            }
        } else {
            let stamp = SystemTime::now().duration_since(UNIX_EPOCH).map_err(|error| error.to_string())?.as_nanos();
            profile.pending_plan = Some(InstallPlan { id: format!("native-{stamp}-{}", &archive.sha256[..12]),
                requested_roots: vec![archive.id.clone()], steps: prepared.steps, native_pack: None });
            let mut validation = profile.clone(); validation.rollback = None;
            validate_profile(&root, &validation)?;
            rollback::begin(&root, &current, &mut profile)?;
            write_locked(&root, &profile)?;
        }
        let url = format!("urn:sha256:{}", archive.sha256);
        if !profile.archives.iter().any(|pin| pin.id == archive.id && pin.version == archive.version && pin.url == url) {
            profile.archives.push(ArchivePin { id: archive.id.clone(), version: archive.version.clone(), url, sha256: archive.sha256.clone() });
        }
        let index = profile.pending_plan.as_ref().expect("plan").steps.iter().position(|step| step.action != "preserve").ok_or_else(invalid_profile)?;
        for earlier in 0..index { complete_package(&mut profile, earlier); }
        if profile.pending_plan.as_ref().expect("plan").steps[index].status != "applied" {
            let step = &mut profile.pending_plan.as_mut().expect("plan").steps[index];
            step.package.archive_sha256 = Some(archive.sha256.clone()); step.status = "downloaded".into(); step.targets = None;
            write_locked(&root, &profile)?;
            let old_archive = current.profile.as_ref().and_then(|profile| known_package(Some(profile), &archive.id))
                .and_then(|package| package.native.as_ref());
            let old_files: Vec<_> = old_archive.into_iter().flat_map(|old| &old.files)
                .map(|file| (PathBuf::from(&file.path), file.sha256.clone())).collect();
            let owned: HashMap<_, _> = old_files.iter().map(|(path, hash)| (key(&path.to_string_lossy()), hash.clone())).collect();
            let expected: HashMap<_, _> = archive.files.iter().map(|file| (key(&file.path), file)).collect();
            mod_update::replace_native_archive_locked(snapshot_path.to_string_lossy().into_owned(), root.to_string_lossy().into_owned(),
                PathBuf::from(&archive.package_path), old_files, &mut |targets, staging| {
                    require_stopped()?;
                    require_observed(&root, &prepared.observed)?;
                    for file in &archive.files {
                        if mod_update::target_hash(staging, Path::new(&file.path))?.as_deref() != Some(&file.sha256) {
                            return Err("Staged native package files differ from the reviewed archive.".into());
                        }
                    }
                    for target in targets {
                        let name = key(&target.relative_path.to_string_lossy());
                        let replacement = expected.get(&name).map(|file| file.sha256.as_str());
                        if target.replacement_sha256.as_deref() != replacement
                            || target.original_sha256.as_ref().is_some_and(|hash| owned.get(&name) != Some(hash)) {
                            return Err(format!("Native package ownership or bytes changed at {}. Existing files were preserved.", target.relative_path.display()));
                        }
                    }
                    if targets.is_empty() { return Err("The native package has no installable files.".into()); }
                    rollback::retain_step(&root, &mut profile, index, targets)?;
                    let step = &mut profile.pending_plan.as_mut().expect("plan").steps[index];
                    step.status = "applying".into(); step.targets = Some(targets.iter().map(|target| TargetReceipt {
                        relative_path: target.relative_path.clone(), original_sha256: target.original_sha256.clone(),
                        replacement_sha256: target.replacement_sha256.clone() }).collect());
                    let mut completed = profile.clone(); complete_package(&mut completed, index); encode_profile(&root, &completed)?;
                    write_locked(&root, &profile).map(|_| ())
                })?;
            complete_package(&mut profile, index);
            write_locked(&root, &profile)?;
        }
        if !profile.requested_roots.iter().any(|id| key(id) == key(&archive.id)) { profile.requested_roots.push(archive.id); }
        profile.pending_plan = None;
        write_locked(&root, &profile)
    }).await.map_err(|error| error.to_string())?
}
