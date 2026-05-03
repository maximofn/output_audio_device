pub mod model;

pub use model::{Sink, SinkState, Snapshot};

pub const DEFAULT_PORT: u16 = 9128;
pub const DEFAULT_BIND: &str = "127.0.0.1";
pub const API_VERSION: &str = "v1";
