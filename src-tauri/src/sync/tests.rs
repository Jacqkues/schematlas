use super::*;
use crate::domain::Project;
use serde_json::json;
use std::path::Path;

fn git(dir: &Path, args: &[&str]) -> String {
    let out = std::process::Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .expect("git");
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).into_owned()
}
fn has_git() -> bool {
    std::process::Command::new("git")
        .arg("--version")
        .output()
        .is_ok()
}

/// A disposable bare remote and a clone with its own identity and app workspace.
async fn teammate(temp: &Path, name: &str) -> (Arc<SyncHub>, String, PathBuf) {
    let clone = temp.join(name);
    git(
        temp,
        &[
            "clone",
            "--quiet",
            temp.join("remote.git").to_str().unwrap(),
            name,
        ],
    );
    git(&clone, &["config", "user.name", name]);
    git(
        &clone,
        &["config", "user.email", &format!("{name}@example.com")],
    );
    let workspace = Arc::new(
        Workspace::open(&temp.join(format!("{name}.sqlite")))
            .await
            .unwrap(),
    );
    let project = workspace
        .repository
        .create(Project::new(name.into(), "".into()).unwrap())
        .await
        .unwrap();
    workspace
        .remember_working_directory(&project.id, clone.to_str().unwrap())
        .await
        .unwrap();
    (SyncHub::new(workspace), project.id, clone)
}
async fn until(
    hub: &SyncHub,
    id: &str,
    what: &str,
    ok: impl Fn(&SyncSnapshot) -> bool,
) -> SyncSnapshot {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
    loop {
        let snapshot = hub.sync_now(id).await.unwrap();
        if ok(&snapshot) {
            return snapshot;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "timed out waiting for {what}: {snapshot:?}"
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}
fn texts(snapshot: &SyncSnapshot) -> Vec<String> {
    snapshot
        .events
        .iter()
        .filter_map(|e| e.body["text"].as_str().map(str::to_owned))
        .collect()
}

#[tokio::test]
async fn teammates_exchange_events_through_the_branch_without_touching_their_checkout() {
    if !has_git() {
        return;
    }
    let temp = tempfile::tempdir().unwrap();
    let temp = temp.path();
    std::process::Command::new("git")
        .args(["init", "--quiet", "--bare"])
        .arg(temp.join("remote.git"))
        .status()
        .unwrap();
    let (ana, ana_id, ana_dir) = teammate(temp, "ana").await;
    std::fs::write(ana_dir.join("README.md"), "hello\n").unwrap();
    git(&ana_dir, &["add", "README.md"]);
    git(&ana_dir, &["commit", "--quiet", "-m", "init"]);
    let head = git(&ana_dir, &["rev-parse", "HEAD"]);
    // A failing pre-push hook guards code pushes, not the team channel.
    let hook = ana_dir.join(".git/hooks/pre-push");
    std::fs::write(&hook, "#!/bin/sh\nexit 1\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    assert_eq!(ana.status(&ana_id).await.unwrap().status, "off");
    let preview = ana.preview(&ana_id, None).await.unwrap();
    assert_eq!(preview.remote.as_deref(), Some("origin"));
    assert!(!preview.branch_exists);
    assert!(preview.has_email);
    assert!(ana
        .post(
            &ana_id,
            Draft {
                kind: "chat".into(),
                body: json!({"text":"x"})
            }
        )
        .await
        .is_err());
    ana.enable(&ana_id, "origin").await.unwrap();
    until(&ana, &ana_id, "branch creation", |s| {
        s.status == "idle" && s.last_sync.is_some()
    })
    .await;
    assert!(ana.preview(&ana_id, None).await.unwrap().branch_exists);

    ana.post(
        &ana_id,
        Draft {
            kind: "chat".into(),
            body: json!({"text":"hello from ana"}),
        },
    )
    .await
    .unwrap();
    assert!(ana
        .post(
            &ana_id,
            Draft {
                kind: "chat".into(),
                body: json!({"text":""})
            }
        )
        .await
        .is_err());
    until(&ana, &ana_id, "ana's push", |s| {
        s.pending == 0 && texts(s).len() == 1
    })
    .await;

    let (bo, bo_id, bo_dir) = teammate(temp, "bo").await;
    bo.enable(&bo_id, "origin").await.unwrap();
    let seen = until(&bo, &bo_id, "bo reading ana", |s| {
        texts(s) == ["hello from ana"]
    })
    .await;
    assert_eq!(seen.events[0].author.name, "ana");
    assert!(seen.presence.iter().any(|p| p.author.name == "ana"));

    // Both post at once: one push loses the race, rebuilds on the winner and retries.
    let (a, b) = tokio::join!(
        ana.post(
            &ana_id,
            Draft {
                kind: "comment".into(),
                body: json!({"text":"race a","diagram":"map","node":"api"})
            }
        ),
        bo.post(
            &bo_id,
            Draft {
                kind: "chat".into(),
                body: json!({"text":"race b"})
            }
        ),
    );
    a.unwrap();
    b.unwrap();
    for (hub, id) in [(&ana, &ana_id), (&bo, &bo_id)] {
        until(hub, id, "both race events", |s| {
            s.pending == 0 && texts(s).len() == 3
        })
        .await;
    }

    // The checkouts are untouched: same HEAD, clean status, no local sync branch.
    assert_eq!(git(&ana_dir, &["rev-parse", "HEAD"]), head);
    assert_eq!(git(&ana_dir, &["status", "--porcelain"]), "");
    assert_eq!(git(&bo_dir, &["status", "--porcelain"]), "");
    assert!(!git(&ana_dir, &["branch", "--list"]).contains("schematlas-sync"));
    let log = git(
        temp.join("remote.git").as_path(),
        &["log", "--format=%s", "schematlas-sync"],
    );
    assert!(log.lines().all(|l| l == COMMIT_MESSAGE), "{log}");
    // The email is never written, only a key derived from it.
    let grep = std::process::Command::new("git")
        .arg("-C")
        .arg(temp.join("remote.git"))
        .args(["grep", "-c", "example.com", "schematlas-sync", "--"])
        .output()
        .unwrap();
    assert!(grep.stdout.is_empty());

    bo.disable(&bo_id, false).await.unwrap();
    assert_eq!(bo.status(&bo_id).await.unwrap().status, "off");
    ana.shutdown().await;
}
