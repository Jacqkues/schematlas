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
pub fn discover() -> Vec<InstalledAgent> {
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
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        for name in ["claude", "claude-agent-acp", "gemini"] {
            let path = dir.path().join(name);
            std::fs::write(&path, "#!/bin/sh\nexit 99\n").unwrap();
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
}
