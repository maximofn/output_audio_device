use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Snapshot {
    pub timestamp: String,
    pub host: String,
    pub server_version: Option<String>,
    pub default_sink: Option<String>,
    pub sinks: Vec<Sink>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Sink {
    pub id: u32,
    pub name: String,
    pub description: String,
    pub state: SinkState,
    pub mute: Option<bool>,
    pub volume_percent: Option<u32>,
    pub active: bool,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SinkState {
    Running,
    Idle,
    Suspended,
    Unknown,
}

impl SinkState {
    pub fn from_pactl(raw: &str) -> Self {
        match raw.trim().to_ascii_uppercase().as_str() {
            "RUNNING" => SinkState::Running,
            "IDLE" => SinkState::Idle,
            "SUSPENDED" => SinkState::Suspended,
            _ => SinkState::Unknown,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_roundtrips_through_json() {
        let snapshot = Snapshot {
            timestamp: "2026-05-03T18:00:00Z".to_string(),
            host: "carbon".to_string(),
            server_version: Some("13.99.1".to_string()),
            default_sink: Some("alsa_output.pci-0000_0c_00.4.iec958-stereo".to_string()),
            sinks: vec![
                Sink {
                    id: 0,
                    name: "alsa_output.pci-0000_0a_00.1.hdmi-stereo-extra1".into(),
                    description: "HDA NVidia Digital Stereo (HDMI 2)".into(),
                    state: SinkState::Suspended,
                    mute: Some(false),
                    volume_percent: Some(100),
                    active: false,
                },
                Sink {
                    id: 1,
                    name: "alsa_output.pci-0000_0c_00.4.iec958-stereo".into(),
                    description: "Starship Digital".into(),
                    state: SinkState::Running,
                    mute: Some(false),
                    volume_percent: Some(80),
                    active: true,
                },
            ],
        };
        let json = serde_json::to_string(&snapshot).unwrap();
        let back: Snapshot = serde_json::from_str(&json).unwrap();
        assert_eq!(snapshot, back);
        assert!(json.contains("\"state\":\"running\""));
    }

    #[test]
    fn sink_state_from_pactl_handles_known_values() {
        assert_eq!(SinkState::from_pactl("RUNNING"), SinkState::Running);
        assert_eq!(SinkState::from_pactl("running"), SinkState::Running);
        assert_eq!(SinkState::from_pactl("  SUSPENDED  "), SinkState::Suspended);
        assert_eq!(SinkState::from_pactl("IDLE"), SinkState::Idle);
        assert_eq!(SinkState::from_pactl("???"), SinkState::Unknown);
    }
}
