use super::*;
use crate::mod_pack::{self, RetainedPackage};

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct PackCheckpoint {
    pub(super) plan_id: String,
    root_name: String,
    workspace: String,
    pub(super) phase: String,
    before_packages: Vec<ProfilePackage>,
    before_requested_roots: Vec<String>,
    receipts: Vec<PackReceipt>,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PackReceipt {
    step_index: usize,
    package_id: String,
    files: RetainedPackage,
    restored: bool,
}

fn same<T: Serialize>(left: &T, right: &T) -> Result<bool, String> {
    Ok(serde_json::to_value(left).map_err(|error| error.to_string())?
        == serde_json::to_value(right).map_err(|error| error.to_string())?)
}

pub(super) fn validate(profile: &ModProfile) -> Result<(), String> {
    let Some(checkpoint) = &profile.rollback else { return Ok(()); };
    if profile.schema_version != 2 || !valid_text(&checkpoint.plan_id) || !valid_text(&checkpoint.root_name)
        || !["available", "restoring", "discarding"].contains(&checkpoint.phase.as_str())
        || !checkpoint.workspace.starts_with(".modstoke-pack-") || checkpoint.workspace.len() != ".modstoke-pack-".len() + 12
        || !checkpoint.workspace[".modstoke-pack-".len()..].bytes().all(|byte| byte.is_ascii_alphanumeric())
        || checkpoint.before_packages.len() > 1024 || checkpoint.before_requested_roots.len() > 1024
        || checkpoint.receipts.len() > 128
        || !unique_ids(checkpoint.before_packages.iter().map(|package| package.id.clone()))
        || !unique_ids(checkpoint.before_requested_roots.iter().cloned()) {
        return Err(invalid_profile());
    }
    for package in &checkpoint.before_packages { validate_package(package)?; }
    let mut previous = None;
    let mut identities = HashSet::new();
    for receipt in &checkpoint.receipts {
        if receipt.step_index >= 128 || previous.is_some_and(|index| index >= receipt.step_index)
            || !valid_text(&receipt.package_id) || !identities.insert(key(&receipt.package_id))
            || checkpoint.phase == "available" && receipt.restored {
            return Err(invalid_profile());
        }
        mod_pack::validate_receipt(&checkpoint.workspace, receipt.step_index, &receipt.files)?;
        previous = Some(receipt.step_index);
        if let Some(plan) = &profile.pending_plan {
            let step = plan.steps.get(receipt.step_index).ok_or_else(invalid_profile)?;
            if key(&step.package.id) != key(&receipt.package_id) || step.action == "preserve" { return Err(invalid_profile()); }
            if let Some(targets) = &step.targets {
                if targets.len() != receipt.files.targets.len() || targets.iter().zip(&receipt.files.targets).any(|(target, saved)|
                    target.relative_path != saved.relative_path || target.original_sha256 != saved.original_sha256
                        || target.replacement_sha256 != saved.replacement_sha256) { return Err(invalid_profile()); }
            }
        }
    }
    if profile.pending_plan.as_ref().is_some_and(|plan| plan.id != checkpoint.plan_id)
        || checkpoint.phase == "discarding" && profile.pending_plan.is_some() {
        return Err(invalid_profile());
    }
    Ok(())
}

pub(super) fn require_forward(profile: &ModProfile) -> Result<(), String> {
    if profile.rollback.as_ref().is_some_and(|checkpoint| checkpoint.phase != "available") {
        return Err("This installation has unfinished pack recovery. Resume it from the Main page before installing mods.".into());
    }
    Ok(())
}

pub(super) fn check_restored_size(profile: &ModProfile, limit: u64) -> Result<(), String> {
    let Some(checkpoint) = &profile.rollback else { return Ok(()); };
    if checkpoint.phase == "discarding" { return Ok(()); }
    let mut restored = profile.clone();
    restored.packages = checkpoint.before_packages.clone();
    restored.requested_roots = checkpoint.before_requested_roots.clone();
    restored.pending_plan = None;
    let restored_checkpoint = restored.rollback.as_mut().expect("checkpoint");
    restored_checkpoint.phase = "discarding".into();
    for receipt in &mut restored_checkpoint.receipts { receipt.restored = true; }
    if serde_json::to_vec_pretty(&restored).map_err(|error| error.to_string())?.len() as u64 > limit {
        return Err("This plan leaves too little profile space to restore its previous package records. Its existing recovery point was preserved.".into());
    }
    Ok(())
}

pub(super) fn validate_write(current: Option<&ModProfile>, proposed: &ModProfile) -> Result<(), String> {
    if let Some(current) = current {
        require_forward(current)?;
        if !same(&current.rollback, &proposed.rollback)? || current.schema_version != proposed.schema_version {
            return Err("Pack recovery records are maintained by Manager's native installer. Refresh the profile before retrying.".into());
        }
        for pin in &current.archives {
            if !proposed.archives.iter().any(|candidate| candidate.id == pin.id && candidate.version == pin.version
                && candidate.url == pin.url && candidate.sha256 == pin.sha256) {
                return Err("Previously observed archive hashes cannot be removed by an install plan.".into());
            }
        }
        if let Some(plan) = &current.pending_plan {
            if let Some(next) = &proposed.pending_plan {
                if plan.id != next.id || plan.requested_roots != next.requested_roots || plan.steps.len() != next.steps.len()
                    || !same(&plan.native_pack, &next.native_pack)? {
                    return Err("The pending dependency plan changed. Resume or roll it back before starting another plan.".into());
                }
                for (before, after) in plan.steps.iter().zip(&next.steps) {
                    let mut expected_package = before.package.clone();
                    expected_package.archive_sha256 = after.package.archive_sha256.clone();
                    if before.action != after.action || !same(&before.before, &after.before)?
                        || !same(&expected_package, &after.package)? || !same(&before.targets, &after.targets)?
                        || before.status != after.status && !(before.action == "preserve" && before.status == "planned" && after.status == "applied"
                            || before.action != "preserve" && ["planned", "downloaded"].contains(&before.status.as_str()) && after.status == "downloaded") {
                        return Err("The recorded package plan cannot be rewritten during installation.".into());
                    }
                }
            } else if plan.steps.iter().any(|step| step.status != "applied") {
                return Err("The dependency plan is unfinished. Resume it or use pack rollback instead of discarding its progress.".into());
            }
        } else if proposed.pending_plan.is_some() && (!same(&current.packages, &proposed.packages)?
            || current.requested_roots != proposed.requested_roots || !same(&current.archives, &proposed.archives)?) {
            return Err("A new plan must start from the current completed profile.".into());
        }
    } else if proposed.rollback.is_some() || !proposed.packages.is_empty() || !proposed.requested_roots.is_empty()
        || !proposed.archives.is_empty() {
        return Err("A new installation profile must start without completed package or recovery records.".into());
    }
    Ok(())
}

pub(super) fn begin(root: &Path, current: &ProfileState, proposed: &mut ModProfile) -> Result<(), String> {
    let is_new = current.profile.as_ref().is_none_or(|profile| profile.pending_plan.is_none()) && proposed.pending_plan.is_some();
    if !is_new { return Ok(()); }
    let plan = proposed.pending_plan.as_ref().ok_or_else(invalid_profile)?;
    if plan.steps.iter().any(|step| step.status != "planned" || step.targets.is_some()) {
        return Err("A new dependency plan must begin before any package has been applied.".into());
    }
    let before = current.profile.as_ref();
    let root_name = plan.native_pack.as_ref().map(|pack| pack.name.clone()).or_else(||
        plan.steps.iter().find(|step| plan.requested_roots.contains(&step.package.id))
            .map(|step| step.package.name.clone())).ok_or_else(invalid_profile)?;
    proposed.schema_version = 2;
    proposed.rollback = Some(PackCheckpoint {
        plan_id: plan.id.clone(), root_name, workspace: ".modstoke-pack-AAAAAAAAAAAA".into(), phase: "available".into(),
        before_packages: before.map(|profile| profile.packages.clone()).unwrap_or_default(),
        before_requested_roots: before.map(|profile| profile.requested_roots.clone()).unwrap_or_default(), receipts: Vec::new(),
    });
    encode_profile(root, proposed)?;
    // Retire the previous completed undo point only after the next plan and its
    // metadata fit. Interrupted cleanup remains a recorded operation.
    if current.profile.as_ref().is_some_and(|profile| profile.rollback.is_some()) {
        discard(root, current.clone(), None)?;
    }
    proposed.rollback.as_mut().expect("checkpoint").workspace = mod_pack::create_workspace(root)?;
    Ok(())
}

pub(super) fn retain_step(root: &Path, profile: &mut ModProfile, index: usize, targets: &[mod_update::UpdateTarget]) -> Result<(), String> {
    let Some(checkpoint) = &mut profile.rollback else { return Ok(()); }; // Legacy plans have no retained originals.
    if checkpoint.phase != "available" { return Err("Pack recovery is already in progress.".into()); }
    let plan = profile.pending_plan.as_ref().ok_or_else(invalid_profile)?;
    let step = plan.steps.get(index).ok_or_else(invalid_profile)?;
    if checkpoint.plan_id != plan.id { return Err(invalid_profile()); }
    let files = mod_pack::retain_package(root, &checkpoint.workspace, index, targets)?;
    if let Some(receipt) = checkpoint.receipts.iter_mut().find(|receipt| receipt.step_index == index) {
        if receipt.files != files || receipt.package_id != step.package.id || receipt.restored {
            return Err("The package's retained originals differ from this retry. Its existing recovery data was preserved.".into());
        }
    } else {
        checkpoint.receipts.push(PackReceipt { step_index: index, package_id: step.package.id.clone(), files, restored: false });
        checkpoint.receipts.sort_by_key(|receipt| receipt.step_index);
    }
    Ok(())
}

fn require_checkpoint(profile: &ModProfile, plan_id: Option<&str>) -> Result<(), String> {
    let checkpoint = profile.rollback.as_ref().ok_or("This plan has no retained originals. Older installs cannot be rolled back automatically.")?;
    if plan_id.is_some_and(|id| id != checkpoint.plan_id) {
        return Err("The displayed recovery point changed. Refresh before trying again.".into());
    }
    Ok(())
}

pub(super) fn discard(root: &Path, state: ProfileState, plan_id: Option<&str>) -> Result<ProfileState, String> {
    let mut profile = state.profile.ok_or_else(invalid_profile)?;
    require_checkpoint(&profile, plan_id)?;
    if profile.pending_plan.is_some() || profile.rollback.as_ref().is_some_and(|checkpoint| checkpoint.phase == "restoring") {
        return Err("Finish the pending install or its rollback before releasing the recovery point.".into());
    }
    if profile.rollback.as_ref().expect("checkpoint").phase != "discarding" {
        profile.rollback.as_mut().expect("checkpoint").phase = "discarding".into();
        write_locked(root, &profile)?;
    }
    let checkpoint = profile.rollback.as_ref().expect("checkpoint");
    let packages: Vec<_> = checkpoint.receipts.iter().map(|receipt| (receipt.step_index, &receipt.files)).collect();
    mod_pack::discard_workspace(root, &checkpoint.workspace, &packages)
        .map_err(|error| format!("Package changes are settled, but retained-backup cleanup needs attention. Use Finish cleanup on the Main page. {error}"))?;
    profile.rollback = None;
    // Once recovery data is released, the ordinary lockfile remains readable by
    // Managers that predate pack checkpoints.
    profile.schema_version = 1;
    write_locked(root, &profile)
}

pub(super) fn restore(root: &Path, state: ProfileState, plan_id: &str) -> Result<ProfileState, String> {
    let mut profile = state.profile.ok_or_else(invalid_profile)?;
    require_checkpoint(&profile, Some(plan_id))?;
    let checkpoint = profile.rollback.as_ref().expect("checkpoint");
    if checkpoint.phase == "discarding" { return Err("The files are already settled. Finish backup cleanup on the Main page.".into()); }
    let packages: Vec<_> = checkpoint.receipts.iter().filter(|receipt| !receipt.restored)
        .map(|receipt| (receipt.step_index, &receipt.files)).collect();
    mod_pack::preflight_restore(root, &checkpoint.workspace, &packages)?;
    if checkpoint.phase != "restoring" {
        profile.rollback.as_mut().expect("checkpoint").phase = "restoring".into();
        write_locked(root, &profile)?;
    }
    let count = profile.rollback.as_ref().expect("checkpoint").receipts.len();
    for index in (0..count).rev() {
        let checkpoint = profile.rollback.as_ref().expect("checkpoint");
        let receipt = &checkpoint.receipts[index];
        if receipt.restored { continue; }
        mod_pack::restore_package(root, &checkpoint.workspace, receipt.step_index, &receipt.files)?;
        profile.rollback.as_mut().expect("checkpoint").receipts[index].restored = true;
        write_locked(root, &profile)?;
    }
    let checkpoint = profile.rollback.as_mut().expect("checkpoint");
    profile.packages = checkpoint.before_packages.clone();
    profile.requested_roots = checkpoint.before_requested_roots.clone();
    profile.pending_plan = None;
    // Observed download hashes survive rollback; reverting package versions must
    // not make a changed download for a previously seen version look new.
    checkpoint.phase = "discarding".into();
    let settled = write_locked(root, &profile)?;
    discard(root, settled, Some(plan_id))
}
