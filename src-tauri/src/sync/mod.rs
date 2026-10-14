//! Team channel over a git branch. Each teammate's app appends events to its own files on the
//! `schematlas-sync` branch and polls for everyone else's, so presence, comments and change notes
//! travel through the git host the team already uses. Opt-in per project; nothing is pushed until
//! the user confirms the remote.
pub mod git;
pub mod log;

use crate::{
    domain::SyncSettings,
    error::{AppError, Result},
    service::Workspace,
};
use git::{Git, Push};
use log::{Author, Draft, Event, Presence, State};
use serde::Serialize;
use std::{
    collections::{BTreeMap, HashMap},
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::sync::{broadcast, Mutex, Notify};

const COMMIT_MESSAGE: &str = "schematlas sync [skip ci]";
const HEARTBEAT: Duration = Duration::from_secs(120);
const COMPACT_AFTER: u64 = 500;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncSnapshot {
    pub project_id: String,
    pub enabled: bool,
    pub remote: Option<String>,
    pub remote_url: Option<String>,
    pub me: Option<Author>,
    /// off | connecting | idle | syncing | error | confirm
    pub status: String,
    pub error: Option<String>,
    pub last_sync: Option<String>,
    pub events: Vec<Event>,
    pub presence: Vec<Presence>,
    pub pending: usize,
}
impl SyncSnapshot {
    fn off(project_id: &str) -> Self {
        Self {
            project_id: project_id.into(),
            enabled: false,
            remote: None,
            remote_url: None,
            me: None,
            status: "off".into(),
            error: None,
            last_sync: None,
            events: vec![],
            presence: vec![],
            pending: 0,
        }
    }
}

/// What enabling would do, shown before anything is pushed.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncPreview {
    pub remotes: Vec<String>,
    pub remote: Option<String>,
    pub remote_url: Option<String>,
    pub name: Option<String>,
    pub has_email: bool,
    pub branch: String,
    pub branch_exists: bool,
    pub error: Option<String>,
}

#[derive(Default)]
struct Inner {
    state: State,
    files: BTreeMap<String, String>,
    tip: Option<String>,
    read_once: bool,
    outbox: Vec<Event>,
    status: String,
    error: Option<String>,
    last_sync: Option<String>,
    last_heartbeat: Option<tokio::time::Instant>,
    focus: Option<(Option<String>, Option<String>)>,
    failures: u32,
}

struct Channel {
    project_id: String,
    remote: String,
    remote_url: String,
    git: Git,
    me: Author,
    inner: Mutex<Inner>,
    wake: Notify,
    stop: AtomicBool,
}

pub struct SyncHub {
    workspace: Arc<Workspace>,
    channels: Mutex<HashMap<String, Arc<Channel>>>,
    pub updates: broadcast::Sender<SyncSnapshot>,
    focused: Arc<AtomicBool>,
}

impl SyncHub {
    pub fn new(workspace: Arc<Workspace>) -> Arc<Self> {
        Arc::new(Self {
            workspace,
            channels: Mutex::new(HashMap::new()),
            updates: broadcast::channel(64).0,
            focused: Arc::new(AtomicBool::new(true)),
        })
    }
    async fn repo_path(&self, project_id: &str) -> Result<PathBuf> {
        Ok(self.workspace.repo(project_id).await?.root().to_owned())
    }
    pub async fn preview(&self, project_id: &str, remote: Option<String>) -> Result<SyncPreview> {
        let git = Git::open(&self.repo_path(project_id).await?).await?;
        let remotes = git.remotes().await?;
        let remote = remote
            .or_else(|| remotes.iter().find(|r| *r == "origin").cloned())
            .or_else(|| remotes.first().cloned());
        let mut preview = SyncPreview {
            remotes,
            remote: remote.clone(),
            remote_url: None,
            name: git.config("user.name").await,
            has_email: git.config("user.email").await.is_some(),
            branch: git::BRANCH.into(),
            branch_exists: false,
            error: None,
        };
        let Some(remote) = remote else {
            preview.error = Some("This repository has no remote to share through.".into());
            return Ok(preview);
        };
        preview.remote_url = Some(git.remote_url(&remote).await?);
        match git.remote_tip(&remote).await {
            Ok(tip) => preview.branch_exists = tip.is_some(),
            Err(error) => preview.error = Some(auth_hint(error)),
        }
        Ok(preview)
    }
    /// Remember the confirmed remote and start syncing. The caller showed `preview` first.
    pub async fn enable(&self, project_id: &str, remote: &str) -> Result<SyncSnapshot> {
        let git = Git::open(&self.repo_path(project_id).await?).await?;
        if !git.remotes().await?.iter().any(|r| r == remote) {
            return Err(AppError::Validation(
                "Choose one of the repository's remotes.".into(),
            ));
        }
        if git.config("user.name").await.is_none() || git.config("user.email").await.is_none() {
            return Err(AppError::Validation(
                "Set git user.name and user.email first; teammates see the name.".into(),
            ));
        }
        let settings = SyncSettings {
            remote: remote.into(),
            remote_url: git.remote_url(remote).await?,
        };
        self.workspace
            .repository
            .mutate(project_id, |p| {
                p.sync = Some(settings);
                Ok(())
            })
            .await?;
        self.stop(project_id).await;
        self.status(project_id).await
    }
    pub async fn disable(&self, project_id: &str, delete_remote: bool) -> Result<SyncSnapshot> {
        let channel = self.channels.lock().await.get(project_id).cloned();
        self.stop(project_id).await;
        if delete_remote {
            if let Some(channel) = channel {
                if let Some(tip) = channel.git.remote_tip(&channel.remote).await? {
                    channel
                        .git
                        .delete_remote_branch(&channel.remote, &tip)
                        .await?;
                }
            }
        }
        self.workspace
            .repository
            .mutate(project_id, |p| {
                p.sync = None;
                Ok(())
            })
            .await?;
        let snapshot = SyncSnapshot::off(project_id);
        let _ = self.updates.send(snapshot.clone());
        Ok(snapshot)
    }
    async fn stop(&self, project_id: &str) {
        if let Some(channel) = self.channels.lock().await.remove(project_id) {
            channel.stop.store(true, Ordering::SeqCst);
            channel.wake.notify_one();
        }
    }
    pub async fn shutdown(&self) {
        for (_, channel) in self.channels.lock().await.drain() {
            channel.stop.store(true, Ordering::SeqCst);
            channel.wake.notify_one();
        }
    }
    /// The current snapshot, starting the project's channel if sync is enabled and not running.
    pub async fn status(&self, project_id: &str) -> Result<SyncSnapshot> {
        if let Some(channel) = self.channels.lock().await.get(project_id).cloned() {
            return Ok(channel.snapshot().await);
        }
        let project = self.workspace.repository.get(project_id).await?;
        let Some(settings) = project.sync else {
            return Ok(SyncSnapshot::off(project_id));
        };
        let mut snapshot = SyncSnapshot::off(project_id);
        snapshot.enabled = true;
        snapshot.remote = Some(settings.remote.clone());
        snapshot.remote_url = Some(settings.remote_url.clone());
        let git = match self.repo_path(project_id).await {
            Ok(path) => Git::open(&path).await,
            Err(e) => Err(e),
        };
        let git = match git {
            Ok(git) => git,
            Err(error) => {
                snapshot.status = "error".into();
                snapshot.error = Some(error.to_string());
                return Ok(snapshot);
            }
        };
        // A changed remote could point at a different audience: ask again before pushing there.
        let url = git.remote_url(&settings.remote).await.ok();
        if url.as_deref() != Some(settings.remote_url.as_str()) {
            snapshot.status = "confirm".into();
            snapshot.error = Some(format!(
                "Remote `{}` now points somewhere else. Confirm team sync again.",
                settings.remote
            ));
            return Ok(snapshot);
        }
        let (Some(name), Some(email)) = (
            git.config("user.name").await,
            git.config("user.email").await,
        ) else {
            snapshot.status = "error".into();
            snapshot.error = Some("Set git user.name and user.email to use team sync.".into());
            return Ok(snapshot);
        };
        let outbox = load_outbox(&git).await;
        let channel = Arc::new(Channel {
            project_id: project_id.into(),
            remote: settings.remote,
            remote_url: settings.remote_url,
            me: Author::new(&name, &email),
            git,
            inner: Mutex::new(Inner {
                status: "connecting".into(),
                outbox,
                ..Inner::default()
            }),
            wake: Notify::new(),
            stop: AtomicBool::new(false),
        });
        {
            let mut channels = self.channels.lock().await;
            if let Some(existing) = channels.get(project_id) {
                return Ok(existing.snapshot().await);
            }
            channels.insert(project_id.into(), channel.clone());
        }
        let updates = self.updates.clone();
        let focused = self.focused.clone();
        let task = channel.clone();
        tokio::spawn(async move { task.run(updates, focused).await });
        Ok(channel.snapshot().await)
    }
    pub async fn post(&self, project_id: &str, draft: Draft) -> Result<SyncSnapshot> {
        log::validate_body(&draft.kind, &draft.body)?;
        self.status(project_id).await?;
        let channel = self
            .channels
            .lock()
            .await
            .get(project_id)
            .cloned()
            .ok_or_else(|| AppError::Validation("Turn on team sync first.".into()))?;
        let snapshot = {
            let mut inner = channel.inner.lock().await;
            let lamport = inner
                .state
                .lamport
                .max(inner.outbox.iter().map(|e| e.lamport).max().unwrap_or(0))
                + 1;
            let event = Event {
                id: uuid::Uuid::new_v4().simple().to_string(),
                v: log::VERSION,
                author: channel.me.clone(),
                lamport,
                ts: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
                kind: draft.kind.clone(),
                body: draft.body.clone(),
            };
            event.line()?;
            match draft.kind.as_str() {
                "editing" => {
                    inner.focus = Some((
                        draft.body["diagram"].as_str().map(str::to_owned),
                        draft.body["note"].as_str().map(str::to_owned),
                    ))
                }
                "editing_done" => inner.focus = None,
                _ => {}
            }
            inner.outbox.push(event);
            save_outbox(&channel.git, &inner.outbox).await;
            channel.snapshot_of(&inner)
        };
        // Short debounce so a burst of edits travels as one commit.
        let wake = channel.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(2)).await;
            wake.wake.notify_one();
        });
        let _ = self.updates.send(snapshot.clone());
        Ok(snapshot)
    }
    pub async fn sync_now(&self, project_id: &str) -> Result<SyncSnapshot> {
        let snapshot = self.status(project_id).await?;
        if let Some(channel) = self.channels.lock().await.get(project_id) {
            channel.wake.notify_one();
        }
        Ok(snapshot)
    }
    pub fn set_focus(&self, focused: bool) {
        let was = self.focused.swap(focused, Ordering::SeqCst);
        if focused && !was {
            if let Ok(channels) = self.channels.try_lock() {
                for channel in channels.values() {
                    channel.wake.notify_one();
                }
            }
        }
    }
}

fn auth_hint(error: AppError) -> String {
    let text = error.to_string();
    if [
        "Authentication",
        "Permission denied",
        "could not read Username",
        "403",
        "401",
    ]
    .iter()
    .any(|m| text.contains(m))
    {
        format!(
            "{text}\nAuthenticate with git in a terminal (for example `git fetch`), then retry."
        )
    } else {
        text
    }
}
async fn load_outbox(git: &Git) -> Vec<Event> {
    tokio::fs::read_to_string(git.state_dir().join("outbox.jsonl"))
        .await
        .map(|text| {
            text.lines()
                .filter_map(|l| serde_json::from_str(l).ok())
                .collect()
        })
        .unwrap_or_default()
}
/// The outbox survives a crash or quit until its events reach the remote.
async fn save_outbox(git: &Git, outbox: &[Event]) {
    let text: String = outbox
        .iter()
        .filter_map(|e| e.line().ok())
        .map(|l| l + "\n")
        .collect();
    let _ = tokio::fs::write(git.state_dir().join("outbox.jsonl"), text).await;
}

impl Channel {
    async fn snapshot(&self) -> SyncSnapshot {
        let inner = self.inner.lock().await;
        self.snapshot_of(&inner)
    }
    fn snapshot_of(&self, inner: &Inner) -> SyncSnapshot {
        let mut state = inner.state.clone();
        for event in &inner.outbox {
            state.add(event.clone());
        }
        SyncSnapshot {
            project_id: self.project_id.clone(),
            enabled: true,
            remote: Some(self.remote.clone()),
            remote_url: Some(self.remote_url.clone()),
            me: Some(self.me.clone()),
            status: inner.status.clone(),
            error: inner.error.clone(),
            last_sync: inner.last_sync.clone(),
            events: state.feed(),
            presence: state.active(&chrono::Utc::now()),
            pending: inner.outbox.len(),
        }
    }
    async fn run(
        self: Arc<Self>,
        updates: broadcast::Sender<SyncSnapshot>,
        focused: Arc<AtomicBool>,
    ) {
        let mut first = true;
        while !self.stop.load(Ordering::SeqCst) {
            if !first {
                let failures = self.inner.lock().await.failures;
                let base = if focused.load(Ordering::SeqCst) {
                    15
                } else {
                    60
                };
                let delay = (base << failures.min(5)).min(300);
                tokio::select! {
                    _ = tokio::time::sleep(Duration::from_secs(delay)) => {}
                    _ = self.wake.notified() => {}
                }
                if self.stop.load(Ordering::SeqCst) {
                    break;
                }
            }
            first = false;
            {
                let mut inner = self.inner.lock().await;
                inner.status = "syncing".into();
                let _ = updates.send(self.snapshot_of(&inner));
            }
            let result = self.sync_once(focused.load(Ordering::SeqCst)).await;
            let mut inner = self.inner.lock().await;
            match result {
                Ok(()) => {
                    inner.status = "idle".into();
                    inner.error = None;
                    inner.failures = 0;
                    inner.last_sync = Some(chrono::Utc::now().to_rfc3339());
                }
                Err(error) => {
                    inner.status = "error".into();
                    inner.error = Some(auth_hint(error));
                    inner.failures += 1;
                }
            }
            let _ = updates.send(self.snapshot_of(&inner));
        }
    }
    async fn refresh(&self, tip: Option<String>) -> Result<()> {
        let files = match &tip {
            Some(tip) => {
                self.git.fetch(&self.remote).await?;
                self.git.read_tree(tip).await?
            }
            None => vec![],
        };
        let mut inner = self.inner.lock().await;
        inner.state = State::from_files(&files);
        inner.files = files.into_iter().collect();
        inner.tip = tip;
        inner.read_once = true;
        Ok(())
    }
    async fn sync_once(&self, focused: bool) -> Result<()> {
        let mut tip = self.git.remote_tip(&self.remote).await?;
        let known = {
            let inner = self.inner.lock().await;
            inner.read_once && inner.tip == tip
        };
        if !known {
            self.refresh(tip.clone()).await?;
        }
        for _ in 0..5 {
            let (writes, removes, sent) = {
                let inner = self.inner.lock().await;
                let heartbeat_due = focused
                    && inner
                        .last_heartbeat
                        .is_none_or(|t| t.elapsed() >= HEARTBEAT);
                if inner.outbox.is_empty() && tip.is_some() && !heartbeat_due {
                    return Ok(());
                }
                self.plan(&inner, tip.is_none())
            };
            let compact = match &tip {
                Some(t) => self.git.commit_count(t).await? >= COMPACT_AFTER,
                None => false,
            };
            let parent = if compact { None } else { tip.as_deref() };
            let commit = self
                .git
                .commit(parent, &writes, &removes, COMMIT_MESSAGE)
                .await?;
            match self
                .git
                .push(&self.remote, &commit, tip.as_deref(), compact)
                .await?
            {
                Push::Pushed => {
                    let mut inner = self.inner.lock().await;
                    for (path, content) in writes {
                        inner.files.insert(path, content);
                    }
                    for path in &removes {
                        inner.files.remove(path);
                    }
                    let files: Vec<(String, String)> = inner
                        .files
                        .iter()
                        .map(|(p, c)| (p.clone(), c.clone()))
                        .collect();
                    inner.state = State::from_files(&files);
                    inner.tip = Some(commit);
                    inner.last_heartbeat = Some(tokio::time::Instant::now());
                    inner.outbox.retain(|e| !sent.contains(&e.id));
                    save_outbox(&self.git, &inner.outbox).await;
                    return Ok(());
                }
                Push::Stale => {
                    tip = self.git.remote_tip(&self.remote).await?;
                    self.refresh(tip.clone()).await?;
                }
            }
        }
        Err(AppError::Validation(
            "The team branch kept moving; will retry shortly.".into(),
        ))
    }
    /// Files to write and remove for one commit: my day files with the outbox appended, my
    /// presence, and anything past retention.
    fn plan(
        &self,
        inner: &Inner,
        create: bool,
    ) -> (Vec<(String, String)>, Vec<String>, Vec<String>) {
        let now = chrono::Utc::now();
        let mut writes: BTreeMap<String, String> = BTreeMap::new();
        if create {
            writes.insert(
                "meta.json".into(),
                serde_json::json!({"version": log::VERSION, "retentionDays": log::RETENTION_DAYS, "about": "Schematlas team channel. Managed by the app; safe to delete."})
                    .to_string()
                    + "\n",
            );
        }
        let mut sent = vec![];
        for event in &inner.outbox {
            let ts = chrono::DateTime::parse_from_rfc3339(&event.ts)
                .map(|t| t.with_timezone(&chrono::Utc))
                .unwrap_or(now);
            let path = log::event_path(&self.me, &ts);
            let Ok(line) = event.line() else { continue };
            let content = writes
                .entry(path.clone())
                .or_insert_with(|| inner.files.get(&path).cloned().unwrap_or_default());
            if !content.is_empty() && !content.ends_with('\n') {
                content.push('\n');
            }
            content.push_str(&line);
            content.push('\n');
            sent.push(event.id.clone());
        }
        let (diagram, note) = inner.focus.clone().unwrap_or_default();
        let presence = Presence {
            author: self.me.clone(),
            ts: now.to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
            diagram,
            note,
        };
        writes.insert(
            log::presence_path(&self.me),
            serde_json::to_string(&presence).unwrap_or_default() + "\n",
        );
        let removes = inner
            .files
            .iter()
            .filter(|(path, content)| {
                let ts = serde_json::from_str::<Presence>(content).ok().map(|p| p.ts);
                !writes.contains_key(*path) && log::expired(path, &now, ts.as_deref())
            })
            .map(|(path, _)| path.clone())
            .collect();
        (writes.into_iter().collect(), removes, sent)
    }
}

#[cfg(test)]
mod tests;
