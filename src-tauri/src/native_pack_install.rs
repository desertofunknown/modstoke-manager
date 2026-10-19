use super::native_archive::NativeArchive;
use super::native_pack::NativePackDocument;
use super::*;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NativePackInstallPreview {
    document: NativePackDocument,
    packages: Vec<PackageAction>,
    profile_revision: Option<String>,
    plan_id: Option<String>,
    notices: Vec<String>,
}

#[derive(Serialize)]
struct PackageAction {
    id: String,
    name: String,
    version: String,
    action: String,
    status: String,
}

pub(super) fn validate_plan(plan: &InstallPlan) -> Result<(), String> {
    let Some(document) = &plan.native_pack else { return Ok(()); };
    if native_pack::canonicalize_document(document.clone())? != *document
        || plan.steps.len() != document.packages.len()
        || plan.requested_roots != document.packages.iter().map(|package| package.id.clone()).collect::<Vec<_>>() {
        return Err(invalid_profile());
    }
    let mut files = 0usize;
    let mut decoded = 0u64;
    let mut pending_seen = false;
    let mut active_seen = false;
    for step in &plan.steps {
        if step.action == "preserve" || step.package.source.provider != native_local::PROVIDER
            || step.package.dependency_basis != native_local::DEPENDENCY_BASIS {
            return Err(invalid_profile());
        }
        let archive = step.package.native.as_ref().ok_or_else(invalid_profile)?;
        let declared = document.packages.iter().find(|package| package.id == archive.id).ok_or_else(invalid_profile)?;
        if native_pack::describe_archive(archive)? != *declared { return Err(invalid_profile()); }
        files += archive.files.len();
        decoded += archive.files.iter().map(|file| file.size).sum::<u64>();
        if files > 32768 || decoded > 2 * 1024 * 1024 * 1024 { return Err(invalid_profile()); }
        if step.status == "applied" {
            if pending_seen { return Err(invalid_profile()); }
        } else {
            if step.status != "planned" {
                if pending_seen || active_seen { return Err(invalid_profile()); }
                active_seen = true;
            }
            pending_seen = true;
        }
    }
    Ok(())
}

fn pending<'a>(profile: Option<&'a ModProfile>, document: &NativePackDocument) -> Result<Option<&'a InstallPlan>, String> {
    let Some(profile) = profile else { return Ok(None); };
    rollback::require_forward(profile)?;
    let Some(plan) = &profile.pending_plan else { return Ok(None); };
    if plan.native_pack.as_ref() != Some(document) {
        return Err("This installation has a different unfinished plan. Load its pending pack and select all matching ZIPs, or use Pack recovery.".into());
    }
    Ok(Some(plan))
}

fn require_progress(plan: &InstallPlan, prepared: &native_local::Prepared) -> Result<(), String> {
    if plan.steps.len() != prepared.steps.len() {
        return Err("The native pack's dependency plan changed. Restore its dependencies or use Pack recovery.".into());
    }
    for (saved, fresh) in plan.steps.iter().zip(&prepared.steps) {
        let mut expected = saved.package.clone();
        expected.archive_sha256 = fresh.package.archive_sha256.clone();
        if !native_local::same(&expected, &fresh.package)?
            || saved.status != "applied" && (saved.action != fresh.action || !native_local::same(&saved.before, &fresh.before)?) {
            return Err(format!("The installed state of {} changed since this pack began. Restore it or use Pack recovery.", saved.package.id));
        }
    }
    Ok(())
}

fn check_capacity(root: &Path, profile: &ModProfile) -> Result<(), String> {
    let mut completed = profile.clone();
    completed.rollback = None;
    encode_profile(root, &completed).map_err(|error| format!("The pack plan does not fit this installation's profile limits: {error}"))?;
    let plan = completed.pending_plan.as_ref().ok_or_else(invalid_profile)?;
    let roots = plan.requested_roots.clone();
    let count = plan.steps.len();
    for index in 0..count {
        let step = &mut completed.pending_plan.as_mut().ok_or_else(invalid_profile)?.steps[index];
        step.package.archive_sha256 = Some(step.package.native.as_ref().ok_or_else(invalid_profile)?.sha256.clone());
        complete_package(&mut completed, index);
    }
    for id in roots {
        if !completed.requested_roots.iter().any(|existing| key(existing) == key(&id)) { completed.requested_roots.push(id); }
    }
    completed.pending_plan = None;
    encode_profile(root, &completed).map_err(|error| format!("The completed pack would exceed this installation's profile limits: {error}"))?;
    Ok(())
}

fn empty_profile(root: &Path) -> ModProfile {
    ModProfile { schema_version: 1, game_root: root.to_string_lossy().into_owned(), requested_roots: Vec::new(),
        packages: Vec::new(), archives: Vec::new(), pending_plan: None, rollback: None }
}

#[tauri::command]
pub async fn inspect_native_pack_install(game_root: String, sources: Vec<String>, document: String) -> Result<NativePackInstallPreview, String> {
    let document = native_pack::parse_native_pack(document)?;
    tauri::async_runtime::spawn_blocking(move || {
        let root = profile_root(&game_root, true)?;
        native_local::require_stopped()?;
        let _lock = mod_update::acquire_update_lock(&root)?;
        mod_update::require_no_pending_updates(&root)?;
        let state = read_locked(&root)?;
        let plan = pending(state.profile.as_ref(), &document)?;
        let inspected = native_pack::prepare_archives(document.name.clone(), sources, Some(document.clone()))?;
        let prepared = native_local::prepare_many(&root, state.profile.as_ref(), &inspected.archives)?;
        if let Some(plan) = plan { require_progress(plan, &prepared)?; }
        if prepared.steps.iter().map(|step| step.package.dependencies.len()).sum::<usize>() > 1024 {
            return Err("This installation plan exceeds 1024 dependency links. Split the collection into smaller compatible packs.".into());
        }
        let mut proposed = state.profile.clone().unwrap_or_else(|| empty_profile(&root));
        if proposed.pending_plan.is_none() {
            proposed.pending_plan = Some(InstallPlan {
                id: "native-pack-review".into(), requested_roots: document.packages.iter().map(|package| package.id.clone()).collect(),
                steps: prepared.steps.clone(), native_pack: Some(document.clone()),
            });
        }
        pin_archives(&mut proposed, &inspected.archives);
        check_capacity(&root, &proposed)?;
        let steps = plan.map(|plan| &plan.steps).unwrap_or(&prepared.steps);
        Ok(NativePackInstallPreview {
            document,
            packages: steps.iter().map(|step| PackageAction {
                id: step.package.id.clone(), name: step.package.name.clone(), version: step.package.version.clone(),
                action: step.action.clone(), status: step.status.clone(),
            }).collect(),
            profile_revision: state.revision.clone(), plan_id: plan.map(|plan| plan.id.clone()),
            notices: vec![
                "Every selected version is planned together, including requirements of other installed native mods.".into(),
                "Existing config/userdata folders and .cfg/.ini files are preserved. Modified, manual or externally managed package files stop installation.".into(),
                "Packages are applied in order under one recovery point. An interrupted or failed operation must be resumed or rolled back before starting the game or server.".into(),
                "Starting a new pack replaces the previous completed recovery point. Installation does not prove runtime compatibility.".into(),
            ],
        })
    }).await.map_err(|error| error.to_string())?
}

fn snapshots(root: &Path, sources: &[String], count: usize) -> Result<Vec<tempfile::TempPath>, String> {
    if sources.len() != count || sources.is_empty() || sources.len() > 128
        || sources.iter().any(|path| path.trim().is_empty() || path.encode_utf16().count() > 32768) {
        return Err("Select every ZIP from the reviewed native pack.".into());
    }
    let mut total = 0u64;
    let mut snapshots = Vec::with_capacity(sources.len());
    for source in sources {
        let file = File::open(source).map_err(|error| format!("Could not open a selected ZIP: {error}"))?;
        let metadata = file.metadata().map_err(|error| error.to_string())?;
        let limit = (256 * 1024 * 1024).min(2 * 1024 * 1024 * 1024 - total);
        if !metadata.is_file() || metadata.len() > limit { return Err("Selected native archives exceed the permitted file or size limits.".into()); }
        let snapshot = tempfile::Builder::new().prefix(".modstoke-profile-").tempfile_in(root).map_err(|error| error.to_string())?;
        let copied = std::io::copy(&mut file.take(limit + 1), &mut snapshot.as_file()).map_err(|error| error.to_string())?;
        if copied > limit { return Err("A selected archive grew beyond its permitted size.".into()); }
        total += copied;
        snapshot.as_file().sync_all().map_err(|error| error.to_string())?;
        snapshots.push(snapshot.into_temp_path());
    }
    Ok(snapshots)
}

fn pin_archives(profile: &mut ModProfile, archives: &[NativeArchive]) {
    for archive in archives {
        let url = format!("urn:sha256:{}", archive.sha256);
        if !profile.archives.iter().any(|pin| key(&pin.id) == key(&archive.id) && pin.version == archive.version && pin.url == url) {
            profile.archives.push(ArchivePin { id: archive.id.clone(), version: archive.version.clone(), url, sha256: archive.sha256.clone() });
        }
    }
}

fn apply_step(root: &Path, profile: &mut ModProfile, index: usize, source: &Path, archive: &NativeArchive, observed: &[(PathBuf, String)]) -> Result<(), String> {
    let old_files: Vec<_> = native_local::known_package(Some(profile), &archive.id).and_then(|package| package.native.as_ref())
        .into_iter().flat_map(|old| &old.files).map(|file| (PathBuf::from(&file.path), file.sha256.clone())).collect();
    let owned: HashMap<_, _> = old_files.iter().map(|(path, digest)| (key(&path.to_string_lossy()), digest.clone())).collect();
    let expected: HashMap<_, _> = archive.files.iter().map(|file| (key(&file.path), file)).collect();
    let step = &mut profile.pending_plan.as_mut().ok_or_else(invalid_profile)?.steps[index];
    step.package.archive_sha256 = Some(archive.sha256.clone());
    step.status = "downloaded".into();
    step.targets = None;
    write_locked(root, profile)?;
    mod_update::replace_native_archive_locked(source.to_string_lossy().into_owned(), root.to_string_lossy().into_owned(),
        PathBuf::from(&archive.package_path), old_files, &mut |targets, staging| {
            native_local::require_stopped()?;
            native_local::require_observed(root, observed)?;
            for file in &archive.files {
                if mod_update::target_hash(staging, Path::new(&file.path))?.as_deref() != Some(&file.sha256) {
                    return Err("Staged native package files differ from the reviewed pack.".into());
                }
            }
            for target in targets {
                let name = key(&target.relative_path.to_string_lossy());
                if target.replacement_sha256.as_deref() != expected.get(&name).map(|file| file.sha256.as_str())
                    || target.original_sha256.as_ref().is_some_and(|digest| owned.get(&name) != Some(digest)) {
                    return Err(format!("Native package ownership or bytes changed at {}. Existing files were preserved.", target.relative_path.display()));
                }
            }
            if targets.is_empty() { return Err("The native package has no installable files.".into()); }
            rollback::retain_step(root, profile, index, targets)?;
            let step = &mut profile.pending_plan.as_mut().ok_or_else(invalid_profile)?.steps[index];
            step.status = "applying".into();
            step.targets = Some(targets.iter().map(|target| TargetReceipt {
                relative_path: target.relative_path.clone(), original_sha256: target.original_sha256.clone(),
                replacement_sha256: target.replacement_sha256.clone(),
            }).collect());
            let mut completed = profile.clone();
            complete_package(&mut completed, index);
            encode_profile(root, &completed)?;
            write_locked(root, profile).map(|_| ())
        })?;
    complete_package(profile, index);
    write_locked(root, profile)?;
    Ok(())
}

#[tauri::command]
pub async fn install_native_pack(game_root: String, sources: Vec<String>, document: String,
    expected_revision: Option<String>, expected_plan_id: Option<String>) -> Result<ProfileState, String> {
    let document = native_pack::parse_native_pack(document)?;
    tauri::async_runtime::spawn_blocking(move || {
        let root = profile_root(&game_root, true)?;
        native_local::require_stopped()?;
        let _lock = mod_update::acquire_update_lock(&root)?;
        let current = read_locked(&root)?;
        require_revision(&current, &expected_revision)?;
        if pending(current.profile.as_ref(), &document)?.map(|plan| &plan.id) != expected_plan_id.as_ref() {
            return Err("The pending pack changed. Review its installation again.".into());
        }
        mod_update::recover_pending_updates(&root)?;
        let current = reconcile(&root, current)?;
        // Keep all verified ZIP snapshots alive until the whole operation exits.
        let snapshots = snapshots(&root, &sources, document.packages.len())?;
        let paths = snapshots.iter().map(|path| path.to_string_lossy().into_owned()).collect();
        let inspected = native_pack::prepare_archives(document.name.clone(), paths, Some(document.clone()))?;
        let archives = inspected.archives;
        let prepared = native_local::prepare_many(&root, current.profile.as_ref(), &archives)?;
        let mut profile = current.profile.clone().unwrap_or_else(|| empty_profile(&root));
        pin_archives(&mut profile, &archives);
        if let Some(plan) = pending(Some(&profile), &document)? {
            require_progress(plan, &prepared)?;
            check_capacity(&root, &profile)?;
        } else {
            let stamp = SystemTime::now().duration_since(UNIX_EPOCH).map_err(|error| error.to_string())?.as_nanos();
            profile.pending_plan = Some(InstallPlan {
                id: format!("native-pack-{stamp}"), requested_roots: document.packages.iter().map(|package| package.id.clone()).collect(),
                steps: prepared.steps, native_pack: Some(document.clone()),
            });
            check_capacity(&root, &profile)?;
            rollback::begin(&root, &current, &mut profile)?;
            write_locked(&root, &profile)?;
        }
        write_locked(&root, &profile)?;
        loop {
            let plan = profile.pending_plan.as_ref().ok_or_else(invalid_profile)?;
            let Some(index) = plan.steps.iter().position(|step| step.status != "applied") else { break; };
            native_local::require_stopped()?;
            // Earlier writes change the observed manifests and assemblies. Rebuild
            // from recorded progress, retaining the original actions and receipts.
            let prepared = native_local::prepare_many(&root, Some(&profile), &archives)?;
            require_progress(plan, &prepared)?;
            let archive_index = archives.iter().position(|archive| key(&archive.id) == key(&plan.steps[index].package.id)).ok_or_else(invalid_profile)?;
            apply_step(&root, &mut profile, index, snapshots[archive_index].as_ref(), &archives[archive_index], &prepared.observed)?;
        }
        // Include the final write in the same ownership and dependency check.
        let prepared = native_local::prepare_many(&root, Some(&profile), &archives)?;
        require_progress(profile.pending_plan.as_ref().ok_or_else(invalid_profile)?, &prepared)?;
        native_local::require_observed(&root, &prepared.observed)?;
        native_local::require_stopped()?;
        for package in &document.packages {
            if !profile.requested_roots.iter().any(|id| key(id) == key(&package.id)) { profile.requested_roots.push(package.id.clone()); }
        }
        profile.pending_plan = None;
        write_locked(&root, &profile)
    }).await.map_err(|error| error.to_string())?
}
