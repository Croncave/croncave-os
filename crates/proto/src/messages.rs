//! JSON messages carried on the control stream, and the typed payloads of other streams.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

// ---------------------------------------------------------------------------------------
// Control stream
// ---------------------------------------------------------------------------------------

/// Sent by the agent on stream 0.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum AgentToRelay {
    /// First message after connecting.
    Hello(Hello),
    /// Durable events, numbered by the agent. The relay acknowledges them once stored;
    /// until then the agent keeps them on disk and resends after a reconnect.
    Events {
        events: Vec<SeqEvent>,
    },
    /// A platform SDK call made by an app's worker (for example a market quote).
    SdkCall {
        id: u64,
        call: SdkCall,
    },
    /// Ask for a fresh credential before the current one expires.
    RenewCredential,
    Pong {
        nonce: u64,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Hello {
    pub computer_id: Uuid,
    pub agent_version: String,
    pub protocol: u32,
    /// Identifies this disk's event numbering; a reset disk starts a new epoch.
    pub outbox_epoch: Uuid,
    /// Runs this agent is still supervising (they survive a dropped connection).
    pub running_runs: Vec<Uuid>,
}

/// Sent by the relay on stream 0.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum RelayToAgent {
    /// Everything up to and including this sequence number is stored.
    Ack {
        upto: u64,
    },
    StartRun(RunSpec),
    StopRun {
        run_id: Uuid,
    },
    /// Answer to a [`AgentEvent::NeedsApproval`] request.
    Approval {
        run_id: Uuid,
        request_id: String,
        approved: bool,
    },
    SdkResult {
        id: u64,
        ok: bool,
        value: Value,
    },
    Credential {
        credential: String,
        expires_at: i64,
    },
    Ping {
        nonce: u64,
    },
    /// The computer is about to go to sleep; flush and exit.
    Sleep,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SeqEvent {
    pub seq: u64,
    /// Unix milliseconds on the computer.
    pub at_ms: i64,
    pub event: AgentEvent,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "e", rename_all = "snake_case")]
pub enum AgentEvent {
    RunStarted {
        run_id: Uuid,
    },
    RunOutput {
        run_id: Uuid,
        stream: OutputStream,
        text: String,
    },
    RunProgress {
        run_id: Uuid,
        cpu_percent: f32,
        memory_mb: u64,
        message: Option<String>,
    },
    /// The run (usually a coding agent) wants a person to approve something.
    NeedsApproval {
        run_id: Uuid,
        request_id: String,
        command: String,
        reason: String,
    },
    RunFinished(RunResult),
    /// A dev server started or stopped listening on the computer's localhost.
    PortChanged {
        port: u16,
        open: bool,
        run_id: Option<Uuid>,
    },
    Health(Health),
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OutputStream {
    Stdout,
    Stderr,
    /// Lines written by the platform about the run (installing packages, stopping...).
    System,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Health {
    pub cpu_percent: f32,
    pub memory_mb: u64,
    pub disk_bytes: u64,
    pub trash_bytes: u64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RunOutcome {
    Succeeded,
    Failed,
    Stopped,
    TimedOut,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RunResult {
    pub run_id: Uuid,
    pub outcome: RunOutcome,
    pub exit_code: Option<i32>,
    /// The optional summary the run wrote, or one the app's worker produced.
    pub summary: Option<Summary>,
    /// The last lines of output.
    pub output_tail: String,
    /// Files created, changed or deleted in the person's folder, from the run's snapshot.
    pub changes: Vec<FileChange>,
    /// App-specific result data (watch observations, a code review diff list...).
    pub data: Value,
    /// A short technical reason when the run failed before or outside the program.
    pub error: Option<String>,
}

/// What a run reports about itself: a one-line headline and a few labelled values.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Summary {
    pub headline: String,
    #[serde(default)]
    pub values: Vec<SummaryValue>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SummaryValue {
    pub label: String,
    pub value: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum ChangeKind {
    Created,
    Changed,
    Deleted,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FileChange {
    /// Path relative to the person's root, with `/` separators.
    pub path: String,
    pub kind: ChangeKind,
    pub size: u64,
}

/// A calls an app's worker makes to the platform through the agent's connection.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "call", rename_all = "snake_case")]
pub enum SdkCall {
    /// Latest price for a stock symbol, from the platform's market data provider.
    MarketQuote { symbol: String },
}

// ---------------------------------------------------------------------------------------
// Runs
// ---------------------------------------------------------------------------------------

/// Everything the agent needs to start a run.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RunSpec {
    pub run_id: Uuid,
    pub job_id: Uuid,
    pub job_name: String,
    pub app: String,
    pub kind: String,
    /// The job's setup, interpreted by the app's worker.
    pub setup: Value,
    /// Secrets as environment variables. Values are masked in output.
    pub secrets: Vec<(String, String)>,
    pub max_runtime_secs: u64,
    pub attempt: u32,
}

/// Setup of a Scripts job.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ScriptSetup {
    /// The script, relative to the person's root.
    pub path: String,
    #[serde(default)]
    pub runtime: Option<Runtime>,
    #[serde(default)]
    pub args: Vec<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Runtime {
    Python,
    Node,
    Shell,
}

impl Runtime {
    pub fn for_path(path: &str) -> Option<Self> {
        let ext = path.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase())?;
        match ext.as_str() {
            "py" => Some(Self::Python),
            "js" | "mjs" | "cjs" => Some(Self::Node),
            "sh" | "bash" => Some(Self::Shell),
            _ => None,
        }
    }
}

/// Setup of a Watcher check: the type's config travels with the job, so the engine runs
/// exactly the version the person approved.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CheckSetup {
    pub watch_type: crate::watcher::WatcherType,
    pub inputs: serde_json::Map<String, Value>,
}

/// Setup of a Code agent task.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentTaskSetup {
    /// Project folder, relative to the person's root.
    pub project: String,
    pub prompt: String,
    /// `mock` in the prototype; `claude` and `openai` later.
    pub provider: String,
}

/// Setup of a Code dev server.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DevServerSetup {
    pub project: String,
    pub command: String,
    pub port: u16,
}

// ---------------------------------------------------------------------------------------
// Streams other than control
// ---------------------------------------------------------------------------------------

/// Payload of an `Open` frame.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "s", rename_all = "snake_case")]
pub enum StreamOpen {
    /// A Files request. The relay may send a body (uploads); the agent answers with one
    /// JSON head frame and, for downloads and previews, body bytes.
    Files(FilesRequest),
    /// An HTTP request to a port on the computer's localhost. The relay sends the body,
    /// the agent answers with a [`HttpHead`] JSON frame followed by the body.
    PreviewHttp(HttpRequestHead),
    /// Review an agent task's changes, or keep/undo one file.
    CodeReview(CodeReviewRequest),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum FilesRequest {
    List {
        path: String,
    },
    /// A small render for display: table rows, a downscaled image or the start of a text.
    Preview {
        path: String,
    },
    Download {
        path: String,
    },
    Mkdir {
        path: String,
    },
    Move {
        from: String,
        to: String,
    },
    /// Moves into Trash, recording who deleted it.
    Delete {
        path: String,
        by: String,
    },
    TrashList,
    Restore {
        trash_id: String,
    },
    EmptyTrash,
    /// Append the stream's body at `offset` to an upload in progress.
    UploadChunk {
        upload_id: String,
        offset: u64,
    },
    UploadStatus {
        upload_id: String,
    },
    UploadFinish {
        upload_id: String,
        path: String,
    },
    /// Write a whole small file (the Code editor's save).
    Write {
        path: String,
    },
    Usage,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FileEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub size: u64,
    pub modified_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TrashEntry {
    pub id: String,
    pub original_path: String,
    pub is_dir: bool,
    pub size: u64,
    pub deleted_ms: i64,
    /// `user`, or `run:<uuid>` when a script or agent deleted it.
    pub deleted_by: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FilePreview {
    Table {
        columns: Vec<String>,
        rows: Vec<Vec<String>>,
        total_rows: u64,
    },
    /// The body that follows is a PNG.
    Image {
        width: u32,
        height: u32,
        original_width: u32,
        original_height: u32,
    },
    Text {
        text: String,
        truncated: bool,
    },
    Unsupported {
        reason: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DiskUsage {
    pub files_bytes: u64,
    pub trash_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HttpRequestHead {
    pub port: u16,
    pub method: String,
    /// Path and query.
    pub path: String,
    pub headers: Vec<(String, String)>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HttpHead {
    pub status: u16,
    pub headers: Vec<(String, String)>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum CodeReviewRequest {
    /// The diff of every file the task changed.
    Diff {
        run_id: Uuid,
    },
    Keep {
        run_id: Uuid,
        path: String,
    },
    Undo {
        run_id: Uuid,
        path: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FileDiff {
    pub path: String,
    pub kind: ChangeKind,
    pub before: Option<String>,
    pub after: Option<String>,
    /// `pending`, `kept` or `undone`.
    pub decision: String,
}

/// Generic error answer on a stream's head frame.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StreamError {
    pub error: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn control_messages_are_tagged_json() {
        let msg = RelayToAgent::StopRun { run_id: Uuid::nil() };
        let json = serde_json::to_value(&msg).unwrap();
        assert_eq!(json["t"], "stop_run");
        let back: RelayToAgent = serde_json::from_value(json).unwrap();
        assert_eq!(back, msg);
    }

    #[test]
    fn runtime_is_found_from_extension() {
        assert_eq!(Runtime::for_path("reports/daily.PY"), Some(Runtime::Python));
        assert_eq!(Runtime::for_path("a.mjs"), Some(Runtime::Node));
        assert_eq!(Runtime::for_path("backup.sh"), Some(Runtime::Shell));
        assert_eq!(Runtime::for_path("README"), None);
    }

    #[test]
    fn stream_open_round_trips() {
        let open = StreamOpen::Files(FilesRequest::Move { from: "a".into(), to: "b".into() });
        let back: StreamOpen = serde_json::from_slice(&serde_json::to_vec(&open).unwrap()).unwrap();
        assert_eq!(back, open);
    }
}
