use clap::Parser;

#[derive(Debug, Clone, Parser)]
#[command(
    name = "audio-monitor-tray",
    about = "Linux system-tray frontend for audio-monitord",
    version
)]
pub struct Config {
    /// Base URL of the audio-monitord HTTP API.
    #[arg(
        long,
        env = "AUDIO_MONITOR_TRAY_URL",
        default_value = "http://127.0.0.1:9128"
    )]
    pub backend_url: String,

    /// tracing-subscriber EnvFilter directive.
    #[arg(long, env = "RUST_LOG", default_value = "info")]
    pub log_level: String,
}
