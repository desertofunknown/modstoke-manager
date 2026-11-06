// Modstoke Admin's staff permission files, which only Manager's editor and F7 may write.
pub(crate) const FOLDER: &str = "BepInEx/plugins/OpenACAIAdminTool";
pub(crate) const POLICY_FILE: &str = "permissions.json";
pub(crate) const LOCK_FILE: &str = "permissions.lock";
pub(crate) const STATUS_FILE: &str = "permissions-status.json";
pub(crate) const NODES_FILE: &str = "permission-nodes.json";

pub(crate) fn is_admin_data(key: &str) -> bool {
    key.strip_prefix(&FOLDER.to_lowercase())
        .and_then(|name| name.strip_prefix('/'))
        .is_some_and(|name| [POLICY_FILE, LOCK_FILE, STATUS_FILE, NODES_FILE].contains(&name))
}
