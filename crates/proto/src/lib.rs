//! The wire protocol between a computer's agent and the relay.
//!
//! The agent opens one WebSocket to the relay and everything travels inside it as
//! multiplexed streams. Every WebSocket message is one binary [`Frame`]:
//!
//! ```text
//! [kind: u8][stream: u32 big-endian][payload ...]
//! ```
//!
//! Stream 0 is the control stream and carries JSON [`AgentToRelay`] / [`RelayToAgent`]
//! messages. Other streams are opened by the relay with an [`StreamOpen`] payload and
//! then carry raw bytes in both directions, so a large file transfer never blocks a
//! control message.

pub mod frame;
pub mod messages;
pub mod watcher;

pub use frame::{Frame, FrameError, FrameKind};
pub use messages::*;

/// Version of the protocol; the relay refuses agents with a different major version.
pub const PROTOCOL_VERSION: u32 = 1;
