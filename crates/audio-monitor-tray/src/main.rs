mod client;
mod config;
mod tray;

use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Parser;
use config::Config;
use tokio::sync::mpsc;
use tracing_subscriber::EnvFilter;
use tray::{AudioTray, State};

const ICON_BASENAME: &str = "speaker";
const ASSET_ICON_REL: &str = "assets/speaker.png";

#[tokio::main]
async fn main() -> Result<()> {
    let cfg = Config::parse();
    init_tracing(&cfg.log_level);

    let icon_dir = prepare_icon_dir().context("could not locate or stage the speaker.png asset")?;
    tracing::info!(dir = %icon_dir.display(), "using icon directory");

    let (switch_tx, switch_rx) = mpsc::channel::<String>(8);
    client::spawn_switcher(cfg.backend_url.clone(), switch_rx);

    let tray_state = AudioTray::new(
        cfg.backend_url.clone(),
        ICON_BASENAME.to_string(),
        icon_dir.to_string_lossy().into_owned(),
        switch_tx,
    );
    let service = ksni::TrayService::new(tray_state);
    let handle = service.handle();
    service.spawn();

    let (tx, mut rx) = mpsc::channel(8);
    client::spawn(cfg.backend_url, tx);

    while let Some(update) = rx.recv().await {
        let state = match update {
            client::Update::Connected(snap) => State::Connected(snap),
            client::Update::Disconnected(err) => State::Disconnected(err),
        };
        handle.update(|tray| tray.set_state(state));
    }

    Ok(())
}

fn init_tracing(directive: &str) {
    let filter = EnvFilter::try_new(directive).unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(filter).init();
}

/// Find a speaker.png the panel can load and return the directory it
/// lives in. ksni publishes `IconThemePath` (the dir) + `IconName` (the
/// basename, no extension); GNOME and KDE then resolve `<dir>/<name>.png`.
///
/// The path **must** be absolute — GNOME-shell's icon resolver does not
/// walk relative paths from the tray's CWD. Symptom when this is wrong:
/// the panel shows three dots / a generic placeholder instead of the icon.
///
/// We prefer paths that survive a fresh build of the binary on a different
/// machine (XDG data dir, /usr/share). When developing from a checkout we
/// fall back to the workspace `assets/` folder; the baked-in
/// `CARGO_MANIFEST_DIR` lookup is the last resort.
fn prepare_icon_dir() -> Result<PathBuf> {
    if let Some(path) = std::env::var_os("AUDIO_MONITOR_TRAY_ICON") {
        let p = PathBuf::from(path);
        if p.exists() {
            if let Some(dir) = p.parent() {
                return canonical_dir(dir);
            }
        }
    }
    let candidates = locate_icon_candidates();
    if let Some(found) = candidates.iter().find(|p| p.exists()) {
        if let Some(dir) = found.parent() {
            return canonical_dir(dir);
        }
    }
    anyhow::bail!(
        "could not find {ICON_BASENAME}.png in any of: {:?}; install via `packaging/install.sh` or set AUDIO_MONITOR_TRAY_ICON",
        candidates
    )
}

fn canonical_dir(dir: &std::path::Path) -> Result<PathBuf> {
    dir.canonicalize()
        .with_context(|| format!("canonicalising {}", dir.display()))
}

fn locate_icon_candidates() -> Vec<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    let xdg_data = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")));
    if let Some(data) = xdg_data {
        candidates.push(
            data.join("audio-monitor")
                .join(format!("{ICON_BASENAME}.png")),
        );
    }
    candidates.push(PathBuf::from(format!(
        "/usr/share/audio-monitor/{ICON_BASENAME}.png"
    )));
    candidates.push(PathBuf::from(ASSET_ICON_REL));
    if let Some(workspace_root) = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
    {
        candidates.push(workspace_root.join(ASSET_ICON_REL));
    }
    candidates
}
