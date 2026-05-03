use audio_monitor_core::{Sink, Snapshot};
use ksni::menu::StandardItem;
use ksni::{MenuItem, ToolTip, Tray};
use tokio::sync::mpsc;

const REPO_URL: &str = "https://github.com/maximofn/output_audio_device";
const COFFEE_URL: &str = "https://www.buymeacoffee.com/maximofn";

#[derive(Debug, Clone)]
pub enum State {
    Connecting,
    Connected(Snapshot),
    Disconnected(String),
}

pub struct AudioTray {
    backend_url: String,
    state: State,
    icon_name: String,
    icon_dir: String,
    /// Sender for sink-switch click events. We use an mpsc::Sender that
    /// can be cloned cheaply into each `activate` callback because those
    /// callbacks run on the ksni service thread (no tokio runtime).
    switch_tx: mpsc::Sender<String>,
}

impl AudioTray {
    pub fn new(
        backend_url: String,
        icon_name: String,
        icon_dir: String,
        switch_tx: mpsc::Sender<String>,
    ) -> Self {
        Self {
            backend_url,
            state: State::Connecting,
            icon_name,
            icon_dir,
            switch_tx,
        }
    }

    pub fn set_state(&mut self, state: State) {
        self.state = state;
    }
}

impl Tray for AudioTray {
    fn id(&self) -> String {
        "audio-monitor".to_string()
    }

    fn title(&self) -> String {
        "Output Audio Device".to_string()
    }

    fn icon_name(&self) -> String {
        self.icon_name.clone()
    }

    fn icon_theme_path(&self) -> String {
        self.icon_dir.clone()
    }

    fn tool_tip(&self) -> ToolTip {
        let title = "Output Audio Device".to_string();
        let description = match &self.state {
            State::Connecting => format!("Connecting to {}", self.backend_url),
            State::Connected(snap) => match snap.sinks.iter().find(|s| s.active) {
                Some(active) => format!("Active: {}", active.description),
                None => format!("{} sink(s); no default", snap.sinks.len()),
            },
            State::Disconnected(err) => format!("Backend offline: {err}"),
        };
        ToolTip {
            icon_name: String::new(),
            icon_pixmap: Vec::new(),
            title,
            description,
        }
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        let mut items: Vec<MenuItem<Self>> = Vec::new();

        items.push(disabled_item("Output devices".into()));

        match &self.state {
            State::Connecting => {
                items.push(disabled_item(format!(
                    "    Connecting to {}…",
                    self.backend_url
                )));
            }
            State::Disconnected(err) => {
                items.push(disabled_item(format!("    Backend offline: {err}")));
            }
            State::Connected(snap) if snap.sinks.is_empty() => {
                items.push(disabled_item("    (no output devices found)".into()));
            }
            State::Connected(snap) => {
                for sink in &snap.sinks {
                    items.push(sink_item(sink, self.switch_tx.clone()));
                }
            }
        }

        items.push(MenuItem::Separator);

        items.push(MenuItem::Standard(StandardItem {
            label: "Repository".into(),
            activate: Box::new(|_| open_url(REPO_URL)),
            ..Default::default()
        }));
        items.push(MenuItem::Standard(StandardItem {
            label: "Buy me a coffee".into(),
            activate: Box::new(|_| open_url(COFFEE_URL)),
            ..Default::default()
        }));
        items.push(MenuItem::Separator);
        items.push(MenuItem::Standard(StandardItem {
            label: "Quit".into(),
            activate: Box::new(|_| std::process::exit(0)),
            ..Default::default()
        }));

        items
    }
}

fn sink_item(sink: &Sink, switch_tx: mpsc::Sender<String>) -> MenuItem<AudioTray> {
    // ksni's StandardItem doesn't expose a radio/checkmark variant we
    // can rely on across panel implementations, so we fake it with a
    // bullet glyph at the start of the label. The active sink keeps a
    // filled bullet; inactive ones get an empty circle.
    let bullet = if sink.active { "● " } else { "○ " };
    let label = format!("{bullet}{}", sink.description);
    let target = sink.name.clone();
    MenuItem::Standard(StandardItem {
        label,
        enabled: true,
        activate: Box::new(move |_tray: &mut AudioTray| {
            // try_send drops the click on a full buffer (8); that's
            // fine — the user has just queued a switch and the next
            // sample will reflect reality.
            if let Err(err) = switch_tx.try_send(target.clone()) {
                tracing::warn!(error = %err, "could not enqueue sink switch");
            }
        }),
        ..Default::default()
    })
}

fn disabled_item(label: String) -> MenuItem<AudioTray> {
    MenuItem::Standard(StandardItem {
        label,
        enabled: false,
        ..Default::default()
    })
}

fn open_url(url: &str) {
    if let Err(err) = open::that(url) {
        tracing::warn!(%url, error = %err, "could not open url");
    }
}
