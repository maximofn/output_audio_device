use std::sync::Arc;
use std::time::Duration;

use audio_monitor_core::Snapshot;
use chrono::Utc;
use tokio::sync::watch;
use tokio::time::{interval, MissedTickBehavior};

use crate::pactl_source::{AudioSource, SampleData};

pub fn empty_snapshot(host: &str, server_version: Option<String>) -> Snapshot {
    Snapshot {
        timestamp: Utc::now().to_rfc3339(),
        host: host.to_string(),
        server_version,
        default_sink: None,
        sinks: Vec::new(),
    }
}

pub async fn build_snapshot(host: &str, source: &dyn AudioSource) -> Snapshot {
    let SampleData {
        default_sink,
        sinks,
    } = source.sample().await.unwrap_or_else(|err| {
        tracing::warn!(error = %err, "audio sample failed; emitting empty list");
        SampleData {
            default_sink: None,
            sinks: Vec::new(),
        }
    });
    Snapshot {
        timestamp: Utc::now().to_rfc3339(),
        host: host.to_string(),
        server_version: source.server_version(),
        default_sink,
        sinks,
    }
}

pub fn spawn(
    source: Arc<dyn AudioSource>,
    host: String,
    interval_ms: u64,
    tx: watch::Sender<Snapshot>,
) {
    tokio::spawn(async move {
        let period = Duration::from_millis(interval_ms.max(50));
        let mut ticker = interval(period);
        ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);

        loop {
            ticker.tick().await;
            let snapshot = build_snapshot(&host, source.as_ref()).await;
            if tx.send(snapshot).is_err() {
                tracing::info!("snapshot channel closed; sampler exiting");
                break;
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pactl_source::MockSource;

    #[tokio::test]
    async fn build_snapshot_uses_source_metadata() {
        let source = MockSource::new();
        let snap = build_snapshot("host-x", &source).await;
        assert_eq!(snap.host, "host-x");
        assert_eq!(snap.sinks.len(), 3);
        assert_eq!(snap.server_version.as_deref(), Some("mock-pactl"));
        assert_eq!(snap.default_sink.as_deref(), Some("mock_speakers"));
        assert!(!snap.timestamp.is_empty());
    }

    #[test]
    fn empty_snapshot_has_no_sinks() {
        let snap = empty_snapshot("h", None);
        assert!(snap.sinks.is_empty());
        assert!(snap.default_sink.is_none());
    }
}
