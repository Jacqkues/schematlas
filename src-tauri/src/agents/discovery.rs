//! Find known executables without launching agents or reading their credentials.
use serde::Serialize;
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledAgent {
    pub id: String,
    pub name: String,
    pub executable: String,
    pub args: Vec<String>,
    pub acp_ready: bool,
    pub note: String,
    pub setup_url: Option<String>,
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
                ".cargo/bin",
                ".asdf/shims",
                ".local/share/mise/shims",
            ]
            .map(|p| home.join(p)),
        );
        for (root, bin) in [
            (home.join(".nvm/versions/node"), "bin"),
            (
                home.join(".local/share/fnm/node-versions"),
                "installation/bin",
            ),
            (
                home.join("Library/Application Support/fnm/node-versions"),
                "installation/bin",
            ),
        ] {
            if let Ok(versions) = std::fs::read_dir(root) {
                let mut versions: Vec<_> = versions.flatten().take(100).map(|v| v.path()).collect();
                versions.sort_by(|a, b| b.cmp(a));
                dirs.extend(versions.into_iter().map(|v| v.join(bin)));
            }
        }
    }
    let mut seen = HashSet::new();
    dirs.retain(|dir| dir.is_absolute() && seen.insert(dir.clone()));
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
        result.push(candidate(&PRESETS[1], bundled, false));
    }
    result.sort_by(|a, b| b.acp_ready.cmp(&a.acp_ready).then(a.name.cmp(&b.name)));
    result
}
struct Preset {
    id: &'static str,
    name: &'static str,
    command: &'static str,
    args: &'static [&'static str],
    cli: Option<&'static str>,
    setup_url: &'static str,
}
// A local launch catalog, independent of the ACP session transport. Detection never
// starts processes or installs packages; initialize negotiates compatibility on selection.
const PRESETS: [Preset; 4] = [
    Preset {
        id: "claude",
        name: "Claude Code",
        command: "claude-agent-acp",
        args: &[],
        cli: Some("claude"),
        setup_url: "https://github.com/agentclientprotocol/claude-agent-acp",
    },
    Preset {
        id: "codex",
        name: "Codex",
        command: "codex-acp",
        args: &[],
        cli: Some("codex"),
        setup_url: "https://github.com/agentclientprotocol/codex-acp",
    },
    Preset {
        id: "gemini",
        name: "Gemini CLI",
        command: "gemini",
        args: &["--experimental-acp"],
        cli: None,
        setup_url: "https://geminicli.com/docs/cli/cli-reference/",
    },
    Preset {
        id: "opencode",
        name: "OpenCode",
        command: "opencode",
        args: &["acp"],
        cli: None,
        setup_url: "https://opencode.ai/docs/acp/",
    },
];
pub fn open_setup(agent_id: &str) -> crate::error::Result<()> {
    let preset = PRESETS
        .iter()
        .find(|p| p.id == agent_id)
        .ok_or_else(|| crate::error::AppError::Validation("Unknown agent.".into()))?;
    #[cfg(target_os = "macos")]
    let mut command = std::process::Command::new("open");
    #[cfg(all(unix, not(target_os = "macos")))]
    let mut command = std::process::Command::new("xdg-open");
    #[cfg(target_os = "windows")]
    let mut command = {
        let mut command = std::process::Command::new("rundll32");
        command.arg("url.dll,FileProtocolHandler");
        command
    };
    if !command.arg(preset.setup_url).status()?.success() {
        return Err(crate::error::AppError::Agent(
            "Could not open the agent's setup instructions.".into(),
        ));
    }
    Ok(())
}
fn candidate(preset: &Preset, path: &Path, ready: bool) -> InstalledAgent {
    InstalledAgent {
        id: preset.id.into(),
        name: preset.name.into(),
        executable: path.to_string_lossy().into(),
        args: if ready {
            preset.args.iter().map(|s| (*s).into()).collect()
        } else {
            vec![]
        },
        acp_ready: ready,
        note: if ready {
            "Installed"
        } else {
            "ACP adapter needed"
        }
        .into(),
        setup_url: (!ready).then(|| preset.setup_url.into()),
    }
}
fn scan(dirs: &[PathBuf]) -> Vec<InstalledAgent> {
    PRESETS
        .iter()
        .filter_map(|preset| {
            locate_in(preset.command, dirs)
                .map(|path| candidate(preset, &path, true))
                .or_else(|| {
                    preset
                        .cli
                        .and_then(|cli| locate_in(cli, dirs))
                        .map(|path| candidate(preset, &path, false))
                })
        })
        .collect()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn detects_executables_and_distinguishes_cli_from_acp() {
        #[cfg(unix)]
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        for name in ["claude", "claude-agent-acp", "gemini", "codex"] {
            let path = dir.path().join(name);
            std::fs::write(&path, "#!/bin/sh\nexit 99\n").unwrap();
            #[cfg(unix)]
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        let result = scan(&[dir.path().into(), dir.path().into()]);
        assert_eq!(result.len(), 3);
        assert!(!result.iter().find(|a| a.name == "Codex").unwrap().acp_ready);
        assert_eq!(
            result.iter().find(|a| a.name == "Gemini CLI").unwrap().args,
            vec!["--experimental-acp"]
        );
        let claude = result.iter().find(|a| a.id == "claude").unwrap();
        assert!(claude.acp_ready);
        assert!(claude.executable.ends_with("claude-agent-acp"));
        assert!(claude.setup_url.is_none());
        assert!(result
            .iter()
            .find(|a| a.id == "codex")
            .unwrap()
            .setup_url
            .is_some());
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
