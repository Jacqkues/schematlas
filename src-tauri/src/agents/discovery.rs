//! Find known executables without launching agents or reading their credentials.
use serde::Serialize;
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledAgent {
    pub name: String,
    pub executable: String,
    pub args: Vec<String>,
    pub acp_ready: bool,
    pub note: String,
}
fn executable(path: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        path.metadata()
            .is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
    }
    #[cfg(not(unix))]
    {
        path.is_file()
    }
}
/// Directories searched for coding agents: the process PATH plus conventional
/// CLI install locations, because Finder-launched apps inherit a minimal PATH.
pub fn search_dirs() -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).collect())
        .unwrap_or_default();
    dirs.extend(["/opt/homebrew/bin", "/usr/local/bin", "/usr/bin"].map(PathBuf::from));
    if let Some(home) = std::env::var_os("HOME") {
        let home = PathBuf::from(home);
        dirs.extend(
            [
                ".local/bin",
                ".npm-global/bin",
                ".bun/bin",
                ".opencode/bin",
                ".volta/bin",
            ]
            .map(|p| home.join(p)),
        );
        if let Ok(versions) = std::fs::read_dir(home.join(".nvm/versions/node")) {
            for version in versions.flatten().take(40) {
                dirs.push(version.path().join("bin"));
            }
        }
    }
    dirs
}
/// First absolute, executable `command` found in `dirs`.
pub fn locate_in(command: &str, dirs: &[PathBuf]) -> Option<PathBuf> {
    dirs.iter()
        .map(|dir| dir.join(command))
        .find(|path| path.is_absolute() && executable(path))
}
pub fn discover() -> Vec<InstalledAgent> {
    let dirs = search_dirs();
    let mut result = scan(&dirs);
    let bundled = Path::new("/Applications/ChatGPT.app/Contents/Resources/codex");
    if !result.iter().any(|a| a.name == "Codex") && executable(bundled) {
        result.push(candidate("Codex", bundled, &[], false));
    }
    result.sort_by(|a, b| b.acp_ready.cmp(&a.acp_ready).then(a.name.cmp(&b.name)));
    result
}
fn candidate(name: &str, path: &Path, args: &[&str], ready: bool) -> InstalledAgent {
    InstalledAgent{name:name.into(),executable:path.to_string_lossy().into(),args:args.iter().map(|s|(*s).into()).collect(),acp_ready:ready,note:if ready{"ACP launch preset. Compatibility is checked when connecting."}else{"Installed CLI. Install its ACP adapter to connect here, or configure a custom ACP executable."}.into()}
}
fn scan(dirs: &[PathBuf]) -> Vec<InstalledAgent> {
    let presets: [(&str, &str, &[&str], bool); 6] = [
        ("Claude ACP", "claude-agent-acp", &[], true),
        ("Codex ACP", "codex-acp", &[], true),
        ("Gemini CLI", "gemini", &["--acp"], true),
        ("OpenCode", "opencode", &["acp"], true),
        ("Claude Code", "claude", &[], false),
        ("Codex", "codex", &[], false),
    ];
    let mut seen = HashSet::new();
    let mut result = Vec::new();
    for (name, command, args, ready) in presets {
        for dir in dirs {
            let path = dir.join(command);
            if !path.is_absolute() || !executable(&path) {
                continue;
            }
            let canonical = path.canonicalize().unwrap_or(path.clone());
            if seen.insert(canonical) {
                result.push(candidate(name, &path, args, ready));
            }
            break;
        }
    }
    result
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn detects_executables_and_distinguishes_cli_from_acp() {
        #[cfg(unix)]
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        for name in ["claude", "claude-agent-acp", "gemini"] {
            let path = dir.path().join(name);
            std::fs::write(&path, "#!/bin/sh\nexit 99\n").unwrap();
            #[cfg(unix)]
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        let result = scan(&[dir.path().into(), dir.path().into()]);
        assert_eq!(result.len(), 3);
        assert!(
            !result
                .iter()
                .find(|a| a.name == "Claude Code")
                .unwrap()
                .acp_ready
        );
        assert_eq!(
            result.iter().find(|a| a.name == "Gemini CLI").unwrap().args,
            vec!["--acp"]
        );
    }
    fn fixture_executable(dir: &Path) -> PathBuf {
        let path = dir.join("claude");
        std::fs::write(&path, "#!/bin/sh\nexit 0\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        path
    }

    #[test]
    fn locates_first_executable_and_ignores_directories() {
        let unusable = tempfile::tempdir().unwrap();
        let first = tempfile::tempdir().unwrap();
        let second = tempfile::tempdir().unwrap();
        // A directory is unusable on every platform; Windows does not have Unix execute bits.
        std::fs::create_dir(unusable.path().join("claude")).unwrap();
        let first_path = fixture_executable(first.path());
        let second_path = fixture_executable(second.path());
        let dirs = [
            unusable.path().to_path_buf(),
            first.path().to_path_buf(),
            second.path().to_path_buf(),
        ];
        assert_eq!(locate_in("claude", &dirs), Some(first_path));
        assert_eq!(locate_in("claude", &dirs[2..]), Some(second_path));
        assert_eq!(locate_in("missing", &dirs), None);
    }

    #[cfg(unix)]
    #[test]
    fn ignores_files_without_execute_permission() {
        use std::os::unix::fs::PermissionsExt;
        let plain = tempfile::tempdir().unwrap();
        let executable_dir = tempfile::tempdir().unwrap();
        let plain_path = plain.path().join("claude");
        std::fs::write(&plain_path, "not executable").unwrap();
        std::fs::set_permissions(&plain_path, std::fs::Permissions::from_mode(0o644)).unwrap();
        let path = fixture_executable(executable_dir.path());
        let dirs = [
            plain.path().to_path_buf(),
            executable_dir.path().to_path_buf(),
        ];
        assert_eq!(locate_in("claude", &dirs), Some(path));
    }
}
