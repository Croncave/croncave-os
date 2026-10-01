//! Supervises runs: starts the app's worker for each one, streams its output, enforces the
//! longest run time, stops it on request, and reports the result.

use std::collections::{HashMap, VecDeque};
use std::path::Path;
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use croncave_proto::{AgentEvent, Health, OutputStream, RunOutcome, RunResult, RunSpec};
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;
use tokio::sync::{mpsc, watch};
use uuid::Uuid;

use crate::Agent;

const TAIL_LINES: usize = 40;

#[derive(Default)]
pub struct Supervisor {
    runs: Mutex<HashMap<Uuid, RunHandle>>,
}

struct RunHandle {
    stop: watch::Sender<bool>,
    approvals: mpsc::Sender<(String, bool)>,
    usage: Arc<Mutex<(f32, u64)>>,
}

impl Supervisor {
    pub fn running(&self) -> Vec<Uuid> {
        self.runs.lock().expect("runs").keys().copied().collect()
    }

    pub async fn stop(&self, run_id: Uuid) {
        if let Some(h) = self.runs.lock().expect("runs").get(&run_id) {
            let _ = h.stop.send(true);
        }
    }

    pub async fn stop_all(&self) {
        for h in self.runs.lock().expect("runs").values() {
            let _ = h.stop.send(true);
        }
        // Give workers a moment to end their process groups.
        for _ in 0..30 {
            if self.runs.lock().expect("runs").is_empty() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }

    pub async fn answer(&self, run_id: Uuid, request_id: &str, approved: bool) {
        let tx = self.runs.lock().expect("runs").get(&run_id).map(|h| h.approvals.clone());
        if let Some(tx) = tx {
            let _ = tx.send((request_id.to_string(), approved)).await;
        }
    }

    fn usage(&self) -> (f32, u64) {
        self.runs.lock().expect("runs").values().fold((0.0, 0), |(c, m), h| {
            let (hc, hm) = *h.usage.lock().expect("usage");
            (c + hc, m + hm)
        })
    }
}

/// What a worker gets for one run.
pub struct RunCtx {
    pub agent: Arc<Agent>,
    pub spec: RunSpec,
    pub stop: watch::Receiver<bool>,
    pub approvals: mpsc::Receiver<(String, bool)>,
    usage: Arc<Mutex<(f32, u64)>>,
}

impl RunCtx {
    pub fn output(&self, stream: OutputStream, text: impl Into<String>) {
        let text = mask(&text.into(), &self.spec.secrets);
        self.agent.emit(AgentEvent::RunOutput { run_id: self.spec.run_id, stream, text });
    }

    pub fn system(&self, text: impl Into<String>) {
        self.output(OutputStream::System, text)
    }

    pub fn stopped(&self) -> bool {
        *self.stop.borrow()
    }

    pub fn result(&self, outcome: RunOutcome) -> RunResult {
        RunResult {
            run_id: self.spec.run_id,
            outcome,
            exit_code: None,
            summary: None,
            output_tail: String::new(),
            changes: vec![],
            data: Value::Null,
            error: None,
        }
    }
}

pub async fn start(agent: Arc<Agent>, spec: RunSpec) {
    let (stop_tx, stop_rx) = watch::channel(false);
    let (ap_tx, ap_rx) = mpsc::channel(8);
    let usage = Arc::new(Mutex::new((0.0, 0)));
    {
        let mut runs = agent.runs.runs.lock().expect("runs");
        if runs.contains_key(&spec.run_id) {
            return; // Already running: a resent StartRun after a reconnect.
        }
        runs.insert(spec.run_id, RunHandle { stop: stop_tx, approvals: ap_tx, usage: usage.clone() });
    }
    let run_id = spec.run_id;
    tokio::spawn(async move {
        agent.emit(AgentEvent::RunStarted { run_id });
        let ctx = RunCtx { agent: agent.clone(), spec, stop: stop_rx, approvals: ap_rx, usage };
        let result = match (ctx.spec.app.as_str(), ctx.spec.kind.as_str()) {
            ("scripts", _) => crate::scripts::run(ctx).await,
            ("watcher", _) => crate::watcher::run(ctx).await,
            ("code", "agent_task") => crate::coder::run_task(ctx).await,
            ("code", "dev_server") => crate::coder::dev_server(ctx).await,
            (app, kind) => {
                let mut r = ctx.result(RunOutcome::Failed);
                r.error = Some(format!("This computer doesn't know how to run {app} {kind} jobs."));
                r
            }
        };
        agent.emit(AgentEvent::RunFinished(result));
        agent.runs.runs.lock().expect("runs").remove(&run_id);
    });
}

/// What happened to a supervised process.
pub struct ProcessEnd {
    pub outcome: RunOutcome,
    pub exit_code: Option<i32>,
    pub tail: String,
}

/// Lines starting with this ask for approval: `@@needs-approval {"request_id":..,"command":..,"reason":..}`.
/// The answer goes back on the process's stdin as `approved <id>` or `denied <id>`.
pub const APPROVAL_PREFIX: &str = "@@needs-approval ";

/// Run a command in its own process group, streaming its output, until it exits, is
/// stopped, or reaches the run's longest run time.
pub async fn supervise(ctx: &mut RunCtx, mut cmd: Command, workdir: &Path) -> ProcessEnd {
    cmd.current_dir(workdir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0)
        .kill_on_drop(true);
    for (k, v) in &ctx.spec.secrets {
        cmd.env(k, v);
    }
    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            let msg = if e.kind() == std::io::ErrorKind::NotFound {
                "The program to run this wasn't found on the computer (exit 127).".to_string()
            } else {
                e.to_string()
            };
            ctx.system(&msg);
            return ProcessEnd { outcome: RunOutcome::Failed, exit_code: Some(127), tail: msg };
        }
    };
    let pid = child.id();
    if let Some(pid) = pid {
        remember_group(&ctx.agent.disk, pid, true);
    }
    let mut stdin = child.stdin.take();
    let (line_tx, mut line_rx) = mpsc::channel::<(OutputStream, String)>(256);
    for (stream, reader) in [
        (
            OutputStream::Stdout,
            child.stdout.take().map(|r| Box::new(r) as Box<dyn tokio::io::AsyncRead + Unpin + Send>),
        ),
        (
            OutputStream::Stderr,
            child.stderr.take().map(|r| Box::new(r) as Box<dyn tokio::io::AsyncRead + Unpin + Send>),
        ),
    ] {
        let Some(reader) = reader else { continue };
        let tx = line_tx.clone();
        tokio::spawn(async move {
            let mut lines = BufReader::new(reader).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                if tx.send((stream, line)).await.is_err() {
                    break;
                }
            }
        });
    }
    drop(line_tx);

    let deadline = tokio::time::Instant::now() + Duration::from_secs(ctx.spec.max_runtime_secs.max(1));
    let mut tail: VecDeque<String> = VecDeque::new();
    let mut progress = tokio::time::interval(Duration::from_secs(3));
    let mut stop = ctx.stop.clone();
    let mut lines_open = true;
    let ended_by: Option<RunOutcome> = loop {
        tokio::select! {
            line = line_rx.recv(), if lines_open => match line {
                Some((stream, line)) => {
                    if let Some(json) = line.strip_prefix(APPROVAL_PREFIX) {
                        if let Ok(v) = serde_json::from_str::<Value>(json) {
                            let s = |k: &str| v[k].as_str().unwrap_or_default().to_string();
                            ctx.agent.emit(AgentEvent::NeedsApproval {
                                run_id: ctx.spec.run_id,
                                request_id: s("request_id"),
                                command: s("command"),
                                reason: s("reason"),
                            });
                        }
                        continue;
                    }
                    let masked = mask(&line, &ctx.spec.secrets);
                    tail.push_back(masked.clone());
                    if tail.len() > TAIL_LINES { tail.pop_front(); }
                    ctx.agent.emit(AgentEvent::RunOutput { run_id: ctx.spec.run_id, stream, text: masked });
                }
                None => lines_open = false,
            },
            answer = ctx.approvals.recv() => {
                if let (Some((id, ok)), Some(stdin)) = (answer, stdin.as_mut()) {
                    let word = if ok { "approved" } else { "denied" };
                    let _ = stdin.write_all(format!("{word} {id}\n").as_bytes()).await;
                    let _ = stdin.flush().await;
                }
            },
            status = child.wait(), if !lines_open => {
                if let Some(pid) = pid {
                    remember_group(&ctx.agent.disk, pid, false);
                }
                let code = status.ok().and_then(|s| s.code());
                let outcome = if code == Some(0) { RunOutcome::Succeeded } else { RunOutcome::Failed };
                return ProcessEnd { outcome, exit_code: code, tail: tail.into_iter().collect::<Vec<_>>().join("\n") };
            },
            _ = progress.tick() => {
                if let Some(pid) = pid {
                    let (cpu, mem) = group_usage(pid).await;
                    *ctx.usage.lock().expect("usage") = (cpu, mem);
                    ctx.agent.emit(AgentEvent::RunProgress { run_id: ctx.spec.run_id, cpu_percent: cpu, memory_mb: mem, message: None });
                }
            },
            _ = tokio::time::sleep_until(deadline) => break Some(RunOutcome::TimedOut),
            _ = stop.changed() => if *stop.borrow() { break Some(RunOutcome::Stopped) },
        }
    };

    let outcome = ended_by.unwrap_or(RunOutcome::Failed);
    if let Some(pid) = pid {
        kill_group(pid).await;
        remember_group(&ctx.agent.disk, pid, false);
    }
    let _ = child.kill().await;
    let code = child.wait().await.ok().and_then(|s| s.code());
    let why = match outcome {
        RunOutcome::TimedOut => {
            format!("Stopped after reaching its longest run time ({}).", human_secs(ctx.spec.max_runtime_secs))
        }
        _ => "Stopped.".to_string(),
    };
    ctx.system(&why);
    tail.push_back(why);
    ProcessEnd { outcome, exit_code: code, tail: tail.into_iter().collect::<Vec<_>>().join("\n") }
}

/// When a process started, as `ps` reports it (works on Linux and macOS). Used to tell a
/// remembered process group from an unrelated one that reused its id.
fn started(pid: u32) -> Option<String> {
    let out = std::process::Command::new("ps").args(["-o", "lstart=", "-p", &pid.to_string()]).output().ok()?;
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    (!s.is_empty()).then_some(s)
}

/// Keep the process groups of running work on disk, so a restarted agent can end them.
fn remember_group(disk: &crate::disk::Disk, pgid: u32, running: bool) {
    let path = disk.system("agent").join("run-groups");
    let mut groups: Vec<String> = std::fs::read_to_string(&path)
        .unwrap_or_default()
        .lines()
        .filter(|l| l.split('\t').next() != Some(&pgid.to_string()))
        .map(String::from)
        .collect();
    if running && let Some(at) = started(pgid) {
        groups.push(format!("{pgid}\t{at}"));
    }
    let body: String = groups.iter().map(|g| format!("{g}\n")).collect();
    let _ = std::fs::write(path, body);
}

/// End the process groups a previous boot left running (only if they are still ours).
pub async fn kill_leftovers(disk: &crate::disk::Disk) {
    let path = disk.system("agent").join("run-groups");
    for line in std::fs::read_to_string(&path).unwrap_or_default().lines() {
        let Some((pgid, at)) = line.split_once('\t') else { continue };
        let Ok(pgid) = pgid.parse::<u32>() else { continue };
        if started(pgid).as_deref() == Some(at) {
            kill_group(pgid).await;
        }
    }
    let _ = std::fs::remove_file(path);
}

/// End every process the run started, not just the first.
pub async fn kill_group(pid: u32) {
    let _ = Command::new("kill").args(["-s", "KILL", "--", &format!("-{pid}")]).status().await;
}

/// CPU percent and memory (MB) of a process group, via `ps` so it works on Linux and macOS.
async fn group_usage(pgid: u32) -> (f32, u64) {
    let Ok(out) = Command::new("ps").args(["-A", "-o", "pgid=,%cpu=,rss="]).output().await else { return (0.0, 0) };
    let mut cpu = 0.0;
    let mut rss_kb = 0u64;
    for line in String::from_utf8_lossy(&out.stdout).lines() {
        let mut it = line.split_whitespace();
        if it.next().and_then(|p| p.parse::<u32>().ok()) == Some(pgid) {
            cpu += it.next().and_then(|c| c.parse::<f32>().ok()).unwrap_or(0.0);
            rss_kb += it.next().and_then(|r| r.parse::<u64>().ok()).unwrap_or(0);
        }
    }
    (cpu, rss_kb / 1024)
}

pub fn mask(text: &str, secrets: &[(String, String)]) -> String {
    let mut out = text.to_string();
    for (_, v) in secrets {
        if v.len() >= 4 {
            out = out.replace(v.as_str(), "••••••");
        }
    }
    out
}

pub fn human_secs(s: u64) -> String {
    match s {
        s if s < 60 => format!("{s} seconds"),
        s if s < 3600 && s % 60 == 0 => format!("{} minutes", s / 60),
        s if s % 3600 == 0 => format!("{} hours", s / 3600),
        s => format!("{} minutes", s / 60),
    }
}

/// Report the computer's health every 30 seconds.
pub async fn report_health(agent: Arc<Agent>) {
    loop {
        let (cpu, mem) = agent.runs.usage();
        let disk = agent.disk.clone();
        let (disk_bytes, trash_bytes) = tokio::task::spawn_blocking(move || {
            (crate::disk::size_of(&disk.root()), crate::disk::size_of(&disk.system("trash")))
        })
        .await
        .unwrap_or((0, 0));
        agent.emit(AgentEvent::Health(Health { cpu_percent: cpu, memory_mb: mem, disk_bytes, trash_bytes }));
        tokio::time::sleep(Duration::from_secs(30)).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secrets_are_masked() {
        let s = vec![("API_KEY".into(), "sk-12345".into()), ("SHORT".into(), "ab".into())];
        assert_eq!(mask("key=sk-12345 ab", &s), "key=•••••• ab");
    }

    #[test]
    fn durations_read_plainly() {
        assert_eq!(human_secs(30), "30 seconds");
        assert_eq!(human_secs(300), "5 minutes");
        assert_eq!(human_secs(7200), "2 hours");
    }
}
