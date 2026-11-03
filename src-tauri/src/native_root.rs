use std::path::Path;

// The Loader's folder in a game or dedicated server root. Modstoke/ replaced OpenACAI/, and the
// Loader resolves Mods and UserData each to Modstoke/<sub> if it exists, otherwise to an existing
// OpenACAI/<sub>, otherwise to Modstoke/<sub>.
pub(crate) const ROOT: &str = "Modstoke";
pub(crate) const LEGACY_ROOT: &str = "OpenACAI";
pub(crate) const ROOTS: [&str; 2] = [ROOT, LEGACY_ROOT];

/// The root folder whose `sub` folder the Loader uses in this installation.
pub(crate) fn resolve_native_root(game_root: &Path, sub: &str) -> &'static str {
    ROOTS
        .into_iter()
        .find(|name| game_root.join(name).join(sub).is_dir())
        .unwrap_or(ROOT)
}

pub(crate) fn is_root_name(name: &str) -> bool {
    ROOTS.iter().any(|root| name.eq_ignore_ascii_case(root))
}

/// Moves a slash-separated `Modstoke/...` or `OpenACAI/...` path onto `root`.
pub(crate) fn rebase(path: &str, root: &str) -> String {
    let (first, rest) = path.split_once('/').unwrap_or((path, ""));
    if !is_root_name(first) {
        path.to_owned()
    } else if rest.is_empty() {
        root.to_owned()
    } else {
        format!("{root}/{rest}")
    }
}

/// Mods and UserData belong to the player under either root.
pub(crate) fn is_player_data(path: &Path) -> bool {
    let mut parts = path.components().map(|part| part.as_os_str().to_string_lossy());
    parts.next().is_some_and(|first| is_root_name(&first))
        && parts.next().is_some_and(|sub| {
            sub.eq_ignore_ascii_case("Mods") || sub.eq_ignore_ascii_case("UserData")
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_each_folder_like_the_loader() {
        let game = tempfile::tempdir().unwrap();
        assert_eq!(resolve_native_root(game.path(), "Mods"), ROOT);
        std::fs::create_dir_all(game.path().join("OpenACAI/Mods")).unwrap();
        std::fs::create_dir_all(game.path().join("Modstoke/Runtime")).unwrap();
        assert_eq!(resolve_native_root(game.path(), "Mods"), LEGACY_ROOT);
        assert_eq!(resolve_native_root(game.path(), "Runtime"), ROOT);
        std::fs::create_dir_all(game.path().join("Modstoke/Mods")).unwrap();
        assert_eq!(resolve_native_root(game.path(), "Mods"), ROOT);
    }

    #[test]
    fn rebases_only_the_root_folder() {
        assert_eq!(rebase("OpenACAI/Mods/Example/a.dll", ROOT), "Modstoke/Mods/Example/a.dll");
        assert_eq!(rebase("openacai/Mods", ROOT), "Modstoke/Mods");
        assert_eq!(rebase("BepInEx/plugins/OpenACAILoader/a.dll", ROOT), "BepInEx/plugins/OpenACAILoader/a.dll");
        assert!(is_player_data(Path::new("OpenACAI/UserData/settings.json")));
        assert!(!is_player_data(Path::new("Modstoke/Runtime/Modstoke.Loader.dll")));
    }
}
