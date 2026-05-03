use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use audio_monitor_core::{Sink, SinkState};
use std::sync::Mutex;
use tokio::process::Command;

#[async_trait]
pub trait AudioSource: Send + Sync {
    fn server_version(&self) -> Option<String>;
    async fn sample(&self) -> Result<SampleData>;
    async fn set_default_sink(&self, name: &str) -> Result<()>;
}

#[derive(Debug, Clone)]
pub struct SampleData {
    pub default_sink: Option<String>,
    pub sinks: Vec<Sink>,
}

pub struct PactlSource {
    server_version: Option<String>,
}

impl PactlSource {
    pub async fn init() -> Result<Self> {
        let version = pactl(&["--version"]).await.ok().and_then(|out| {
            out.lines()
                .next()
                .and_then(|line| line.split_whitespace().nth(1))
                .map(|s| s.to_string())
        });
        Ok(Self {
            server_version: version,
        })
    }
}

#[async_trait]
impl AudioSource for PactlSource {
    fn server_version(&self) -> Option<String> {
        self.server_version.clone()
    }

    async fn sample(&self) -> Result<SampleData> {
        let default_sink = read_default_sink().await;
        let sinks_raw = pactl(&["list", "sinks"])
            .await
            .context("running `pactl list sinks`")?;
        let mut sinks = parse_sinks(&sinks_raw);
        if let Some(ref active) = default_sink {
            for s in &mut sinks {
                s.active = &s.name == active;
            }
        }
        Ok(SampleData {
            default_sink,
            sinks,
        })
    }

    async fn set_default_sink(&self, name: &str) -> Result<()> {
        let status = Command::new("pactl")
            .arg("set-default-sink")
            .arg(name)
            .env("LC_ALL", "C")
            .env("LANG", "C")
            .status()
            .await
            .context("spawning pactl set-default-sink")?;
        if !status.success() {
            return Err(anyhow!(
                "pactl set-default-sink {name} exited with {status}"
            ));
        }
        Ok(())
    }
}

async fn read_default_sink() -> Option<String> {
    // Newer pactl provides a dedicated subcommand; if it fails (older
    // versions, or we get an empty answer because no sink is active yet),
    // fall back to `pactl info` which always exposes "Default Sink:".
    if let Ok(out) = pactl(&["get-default-sink"]).await {
        let trimmed = out.trim();
        if !trimmed.is_empty() {
            return Some(trimmed.to_string());
        }
    }
    if let Ok(info) = pactl(&["info"]).await {
        for line in info.lines() {
            if let Some(rest) = line.strip_prefix("Default Sink:") {
                let name = rest.trim();
                if !name.is_empty() {
                    return Some(name.to_string());
                }
            }
        }
    }
    None
}

async fn pactl(args: &[&str]) -> Result<String> {
    // Force English output: avoids both the "Estado/Sink/Descripción"
    // localisation drift the Python original had to handle and changes in
    // wording across pactl versions.
    let output = Command::new("pactl")
        .args(args)
        .env("LC_ALL", "C")
        .env("LANG", "C")
        .output()
        .await
        .context("spawning pactl")?;
    if !output.status.success() {
        return Err(anyhow!(
            "pactl {:?} failed ({}): {}",
            args,
            output.status,
            String::from_utf8_lossy(&output.stderr).trim(),
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Parse the output of `pactl list sinks` (LC_ALL=C). The format is a
/// sequence of `Sink #N` records, each with key/value lines indented by
/// a single tab. Sub-blocks like `Properties:`, `Ports:`, `Formats:` are
/// indented with two tabs and are ignored — we only need top-level fields.
fn parse_sinks(raw: &str) -> Vec<Sink> {
    let mut out: Vec<Sink> = Vec::new();
    let mut current: Option<PartialSink> = None;
    let mut in_subblock = false;

    for line in raw.lines() {
        if let Some(id_str) = line.strip_prefix("Sink #") {
            if let Some(prev) = current.take() {
                if let Some(sink) = prev.into_sink() {
                    out.push(sink);
                }
            }
            in_subblock = false;
            if let Ok(id) = id_str.trim().parse::<u32>() {
                current = Some(PartialSink::new(id));
            }
            continue;
        }
        let Some(slot) = current.as_mut() else {
            continue;
        };

        // Top-level fields are indented by exactly one tab. Sub-block bodies
        // start with two tabs (`\t\t`) and we skip them entirely.
        if line.starts_with("\t\t") {
            continue;
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        // Section markers move us in/out of sub-blocks but the key:value
        // lines of those blocks aren't useful for us. The `Active Port:`
        // line is at top level and harmless.
        if trimmed.ends_with(':') {
            in_subblock = matches!(trimmed, "Properties:" | "Ports:" | "Formats:");
            continue;
        }
        if in_subblock {
            // Defensive: pactl uses two tabs for sub-block content but we
            // already filtered that. Anything reaching here is a top-level
            // line that ends a section implicitly.
            in_subblock = false;
        }
        let Some((key, value)) = trimmed.split_once(':') else {
            continue;
        };
        let key = key.trim();
        let value = value.trim();
        match key {
            "State" => slot.state = Some(SinkState::from_pactl(value)),
            "Name" => slot.name = Some(value.to_string()),
            "Description" => slot.description = Some(value.to_string()),
            "Mute" => slot.mute = Some(value.eq_ignore_ascii_case("yes")),
            "Volume" => slot.volume_percent = parse_volume_percent(value),
            _ => {}
        }
    }
    if let Some(prev) = current.take() {
        if let Some(sink) = prev.into_sink() {
            out.push(sink);
        }
    }
    out
}

fn parse_volume_percent(value: &str) -> Option<u32> {
    // Sample: `front-left: 65536 / 100% / 0.00 dB,   front-right: 65536 / 100% / 0.00 dB`
    // We grab the first `NN%` token; the per-channel values are usually equal.
    for tok in value.split_whitespace() {
        if let Some(num) = tok.strip_suffix('%') {
            if let Ok(v) = num.parse::<u32>() {
                return Some(v);
            }
        }
    }
    None
}

struct PartialSink {
    id: u32,
    name: Option<String>,
    description: Option<String>,
    state: Option<SinkState>,
    mute: Option<bool>,
    volume_percent: Option<u32>,
}

impl PartialSink {
    fn new(id: u32) -> Self {
        Self {
            id,
            name: None,
            description: None,
            state: None,
            mute: None,
            volume_percent: None,
        }
    }

    fn into_sink(self) -> Option<Sink> {
        let name = self.name?;
        let description = self.description.unwrap_or_else(|| name.clone());
        Some(Sink {
            id: self.id,
            name,
            description,
            state: self.state.unwrap_or(SinkState::Unknown),
            mute: self.mute,
            volume_percent: self.volume_percent,
            active: false,
        })
    }
}

pub struct MockSource {
    sinks: Mutex<Vec<Sink>>,
    default_sink: Mutex<Option<String>>,
}

impl MockSource {
    pub fn new() -> Self {
        let sinks = vec![
            Sink {
                id: 0,
                name: "mock_speakers".into(),
                description: "Mock Internal Speakers".into(),
                state: SinkState::Idle,
                mute: Some(false),
                volume_percent: Some(80),
                active: true,
            },
            Sink {
                id: 1,
                name: "mock_hdmi".into(),
                description: "Mock HDMI Output".into(),
                state: SinkState::Suspended,
                mute: Some(false),
                volume_percent: Some(100),
                active: false,
            },
            Sink {
                id: 2,
                name: "mock_usb_headset".into(),
                description: "Mock USB Headset".into(),
                state: SinkState::Suspended,
                mute: Some(false),
                volume_percent: Some(60),
                active: false,
            },
        ];
        Self {
            sinks: Mutex::new(sinks),
            default_sink: Mutex::new(Some("mock_speakers".into())),
        }
    }
}

#[async_trait]
impl AudioSource for MockSource {
    fn server_version(&self) -> Option<String> {
        Some("mock-pactl".to_string())
    }

    async fn sample(&self) -> Result<SampleData> {
        let default_sink = self.default_sink.lock().unwrap().clone();
        let mut sinks = self.sinks.lock().unwrap().clone();
        if let Some(ref active) = default_sink {
            for s in &mut sinks {
                s.active = &s.name == active;
            }
        }
        Ok(SampleData {
            default_sink,
            sinks,
        })
    }

    async fn set_default_sink(&self, name: &str) -> Result<()> {
        let sinks = self.sinks.lock().unwrap();
        if !sinks.iter().any(|s| s.name == name) {
            return Err(anyhow!("unknown sink: {name}"));
        }
        drop(sinks);
        *self.default_sink.lock().unwrap() = Some(name.to_string());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_OUTPUT: &str = "Sink #0\n\
\tState: SUSPENDED\n\
\tName: alsa_output.pci-0000_0a_00.1.hdmi-stereo-extra1\n\
\tDescription: HDA NVidia Digital Stereo (HDMI 2)\n\
\tDriver: module-alsa-card.c\n\
\tMute: no\n\
\tVolume: front-left: 65536 / 100% / 0.00 dB,   front-right: 65536 / 100% / 0.00 dB\n\
\t        balance 0.00\n\
\tBase Volume: 65536 / 100% / 0.00 dB\n\
\tFlags: HARDWARE DECIBEL_VOLUME LATENCY SET_FORMATS\n\
\tProperties:\n\
\t\talsa.resolution_bits = \"16\"\n\
\t\tdevice.api = \"alsa\"\n\
\tPorts:\n\
\t\thdmi-output-1: HDMI / DisplayPort 2 (priority: 5800, available)\n\
\tActive Port: hdmi-output-1\n\
\tFormats:\n\
\t\tpcm\n\
\n\
Sink #1\n\
\tState: RUNNING\n\
\tName: alsa_output.pci-0000_0c_00.4.iec958-stereo\n\
\tDescription: Starship Digital\n\
\tMute: yes\n\
\tVolume: front-left: 52428 / 80% / -4.94 dB\n\
\tProperties:\n\
\t\talsa.card = \"2\"\n";

    #[test]
    fn parser_extracts_two_sinks() {
        let sinks = parse_sinks(SAMPLE_OUTPUT);
        assert_eq!(sinks.len(), 2);

        assert_eq!(sinks[0].id, 0);
        assert_eq!(sinks[0].state, SinkState::Suspended);
        assert_eq!(
            sinks[0].name,
            "alsa_output.pci-0000_0a_00.1.hdmi-stereo-extra1"
        );
        assert_eq!(sinks[0].description, "HDA NVidia Digital Stereo (HDMI 2)");
        assert_eq!(sinks[0].mute, Some(false));
        assert_eq!(sinks[0].volume_percent, Some(100));
        assert!(!sinks[0].active);

        assert_eq!(sinks[1].id, 1);
        assert_eq!(sinks[1].state, SinkState::Running);
        assert_eq!(sinks[1].mute, Some(true));
        assert_eq!(sinks[1].volume_percent, Some(80));
    }

    #[test]
    fn parser_ignores_property_subblock_lines() {
        let sinks = parse_sinks(SAMPLE_OUTPUT);
        // "alsa.resolution_bits" or "device.api" must not show up as field
        // values; the only way they could is if the sub-block filter broke.
        assert!(sinks
            .iter()
            .all(|s| !s.description.contains("alsa.resolution_bits")));
    }

    #[test]
    fn volume_percent_picks_first_percent_token() {
        assert_eq!(
            parse_volume_percent("front-left: 65536 / 100% / 0.00 dB"),
            Some(100)
        );
        assert_eq!(
            parse_volume_percent("front-left: 0 / 0% / -inf dB"),
            Some(0)
        );
        assert_eq!(parse_volume_percent("balance 0.00"), None);
    }

    #[tokio::test]
    async fn mock_source_set_default_changes_active_flag() {
        let src = MockSource::new();
        src.set_default_sink("mock_hdmi").await.unwrap();
        let sample = src.sample().await.unwrap();
        let active: Vec<_> = sample.sinks.iter().filter(|s| s.active).collect();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].name, "mock_hdmi");
        assert_eq!(sample.default_sink.as_deref(), Some("mock_hdmi"));
    }

    #[tokio::test]
    async fn mock_source_rejects_unknown_sink() {
        let src = MockSource::new();
        let err = src.set_default_sink("nope").await.unwrap_err();
        assert!(err.to_string().contains("unknown sink"));
    }
}
