//! The Code app's worker: hosts a coding agent tool on a project, keeps a copy of the
//! project as it was so every change can be reviewed and kept or undone, and runs dev
//! servers whose ports reach the person only through previews.
//!
//! The real tools (Claude's and OpenAI's) run unmodified as child processes. In the
//! prototype the child is `croncave-agent mock-coder`, a scripted stand-in that edits
//! files and asks for approval before running a command, the way the real tools do.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::time::Duration;

use croncave_proto::{
    AgentEvent, AgentTaskSetup, ChangeKind, CodeReviewRequest, DevServerSetup, FileChange, FileDiff, RunOutcome,
    RunResult, Summary,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::io::AsyncBufReadExt;
use tokio::process::Command;

use crate::Agent;
use crate::runs::{APPROVAL_PREFIX, RunCtx, supervise};
use crate::stream::AgentStream;

const SKIP: &[&str] = &["node_modules", ".git", "__pycache__", ".venv"];
const MAX_DIFF_TEXT: usize = 200 * 1024;

#[derive(Serialize, Deserialize)]
struct Review {
    project: String,
    files: Vec<ReviewFile>,
}

#[derive(Serialize, Deserialize)]
struct ReviewFile {
    path: String,
    kind: ChangeKind,
    decision: String,
}

fn review_dir(agent: &Agent, run_id: uuid::Uuid) -> PathBuf {
    agent.disk.system("apps/code").join(run_id.to_string())
}

pub async fn run_task(mut ctx: RunCtx) -> RunResult {
    let setup: AgentTaskSetup = match serde_json::from_value(ctx.spec.setup.clone()) {
        Ok(s) => s,
        Err(e) => return fail(&ctx, format!("The task's setup couldn't be read: {e}")),
    };
    let project = match ctx.agent.disk.resolve(&setup.project) {
        Ok(p) if p.is_dir() => p,
        _ => return fail(&ctx, format!("The project \"{}\" wasn't found.", setup.project)),
    };
    let dir = review_dir(&ctx.agent, ctx.spec.run_id);
    let base = dir.join("base");
    if let Err(e) = copy_tree(&project, &base) {
        return fail(&ctx, format!("Couldn't keep a copy of the project to review against: {e}"));
    }
    let exe = match std::env::current_exe() {
        Ok(e) => e,
        Err(e) => return fail(&ctx, e.to_string()),
    };
    let tool = match setup.provider.as_str() {
        "mock" => {
            let mut c = Command::new(exe);
            c.arg("mock-coder").arg("--project").arg(&project).arg("--prompt").arg(&setup.prompt);
            c
        }
        other => return fail(&ctx, format!("The {other} coding agent isn't set up on this computer yet.")),
    };
    ctx.system(format!("Starting the coding agent on {}", setup.project));
    let end = supervise(&mut ctx, tool, &project).await;

    let files = changed_files(&base, &project);
    let review = Review {
        project: setup.project.clone(),
        files: files
            .iter()
            .map(|(p, k)| ReviewFile { path: p.clone(), kind: *k, decision: "pending".into() })
            .collect(),
    };
    let _ = std::fs::write(dir.join("review.json"), serde_json::to_vec(&review).expect("review serializes"));
    let changes: Vec<FileChange> = files
        .iter()
        .map(|(p, k)| FileChange {
            path: format!("{}/{}", setup.project.trim_matches('/'), p),
            kind: *k,
            size: std::fs::metadata(project.join(p)).map(|m| m.len()).unwrap_or(0),
        })
        .collect();
    let mut r = ctx.result(end.outcome);
    r.exit_code = end.exit_code;
    r.output_tail = end.tail;
    r.summary = Some(Summary {
        headline: match files.len() {
            0 => "The agent finished without changing any files".into(),
            1 => "The agent changed 1 file. Review it before it lands.".into(),
            n => format!("The agent changed {n} files. Review them before they land."),
        },
        values: vec![],
    });
    r.data = json!({ "project": setup.project, "review": review.files });
    r.changes = changes;
    r
}

fn fail(ctx: &RunCtx, error: String) -> RunResult {
    ctx.system(&error);
    let mut r = ctx.result(RunOutcome::Failed);
    r.output_tail = error.clone();
    r.error = Some(error);
    r
}

fn copy_tree(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for e in std::fs::read_dir(from)?.flatten() {
        let name = e.file_name();
        if SKIP.contains(&name.to_string_lossy().as_ref()) {
            continue;
        }
        let ft = e.file_type()?;
        if ft.is_dir() {
            copy_tree(&e.path(), &to.join(&name))?;
        } else if ft.is_file() {
            std::fs::copy(e.path(), to.join(&name))?;
        }
    }
    Ok(())
}

fn files_in(dir: &Path, base: &Path, out: &mut BTreeSet<String>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        if SKIP.contains(&name.as_str()) {
            continue;
        }
        let p = e.path();
        if p.is_dir() {
            files_in(&p, base, out);
        } else {
            out.insert(crate::disk::rel_string(base, &p));
        }
    }
}

/// Compare file contents, since the copy's timestamps say nothing.
pub fn changed_files(base: &Path, project: &Path) -> Vec<(String, ChangeKind)> {
    let mut before = BTreeSet::new();
    let mut after = BTreeSet::new();
    files_in(base, base, &mut before);
    files_in(project, project, &mut after);
    let mut out = Vec::new();
    for p in before.union(&after) {
        let kind = match (before.contains(p), after.contains(p)) {
            (true, true) => {
                if std::fs::read(base.join(p)).ok() == std::fs::read(project.join(p)).ok() {
                    continue;
                }
                ChangeKind::Changed
            }
            (false, true) => ChangeKind::Created,
            _ => ChangeKind::Deleted,
        };
        out.push((p.clone(), kind));
    }
    out
}

fn read_text(p: &Path) -> Option<String> {
    let b = std::fs::read(p).ok()?;
    if b.len() > MAX_DIFF_TEXT || b.contains(&0) {
        return Some("(This file is too large or not text, so it isn't shown.)".into());
    }
    Some(String::from_utf8_lossy(&b).into_owned())
}

pub async fn review(agent: &Agent, req: CodeReviewRequest, s: &mut AgentStream) {
    let run_id = match &req {
        CodeReviewRequest::Diff { run_id }
        | CodeReviewRequest::Keep { run_id, .. }
        | CodeReviewRequest::Undo { run_id, .. } => *run_id,
    };
    let dir = review_dir(agent, run_id);
    let Some(mut review) =
        std::fs::read(dir.join("review.json")).ok().and_then(|b| serde_json::from_slice::<Review>(&b).ok())
    else {
        s.send_error("There's nothing to review for this task.").await;
        return;
    };
    let project = match agent.disk.resolve(&review.project) {
        Ok(p) => p,
        Err(e) => {
            s.send_error(e).await;
            return;
        }
    };
    let base = dir.join("base");
    let undo = matches!(req, CodeReviewRequest::Undo { .. });
    match req {
        CodeReviewRequest::Diff { .. } => {
            let diffs: Vec<FileDiff> = review
                .files
                .iter()
                .map(|f| FileDiff {
                    path: f.path.clone(),
                    kind: f.kind,
                    before: read_text(&base.join(&f.path)),
                    after: read_text(&project.join(&f.path)),
                    decision: f.decision.clone(),
                })
                .collect();
            s.send_json(&diffs).await;
        }
        CodeReviewRequest::Keep { path, .. } | CodeReviewRequest::Undo { path, .. } => {
            let Some(file) = review.files.iter_mut().find(|f| f.path == path) else {
                s.send_error("That file isn't part of this task's changes.").await;
                return;
            };
            if file.decision != "pending" {
                s.send_error("That change was already decided.").await;
                return;
            }
            if undo {
                let target = project.join(&file.path);
                let result = match file.kind {
                    ChangeKind::Created => std::fs::remove_file(&target),
                    _ => {
                        if let Some(p) = target.parent() {
                            let _ = std::fs::create_dir_all(p);
                        }
                        std::fs::copy(base.join(&file.path), &target).map(|_| ())
                    }
                };
                if let Err(e) = result {
                    s.send_error(format!("Couldn't undo the change: {e}")).await;
                    return;
                }
            }
            file.decision = if undo { "undone" } else { "kept" }.into();
            let _ = std::fs::write(dir.join("review.json"), serde_json::to_vec(&review).expect("review serializes"));
            s.send_json(
                &json!({ "ok": true, "pending": review.files.iter().filter(|f| f.decision == "pending").count() }),
            )
            .await;
        }
    }
}

/// Run a dev server and report when its port opens and closes. Its port is reached only
/// through a private preview.
pub async fn dev_server(mut ctx: RunCtx) -> RunResult {
    let setup: DevServerSetup = match serde_json::from_value(ctx.spec.setup.clone()) {
        Ok(s) => s,
        Err(e) => return fail(&ctx, format!("The dev server's setup couldn't be read: {e}")),
    };
    let project = match ctx.agent.disk.resolve(&setup.project) {
        Ok(p) if p.is_dir() => p,
        _ => return fail(&ctx, format!("The project \"{}\" wasn't found.", setup.project)),
    };
    let mut cmd = Command::new("sh");
    cmd.arg("-c").arg(&setup.command).env("PORT", setup.port.to_string()).env("HOST", "127.0.0.1");
    let (done_tx, mut done_rx) = tokio::sync::watch::channel(false);
    let probe = {
        let agent = ctx.agent.clone();
        let port = setup.port;
        let run_id = ctx.spec.run_id;
        tokio::spawn(async move {
            let mut open = false;
            loop {
                let now = tokio::net::TcpStream::connect(("127.0.0.1", port)).await.is_ok();
                if now != open {
                    open = now;
                    agent.emit(AgentEvent::PortChanged { port, open, run_id: Some(run_id) });
                }
                tokio::select! {
                    _ = tokio::time::sleep(Duration::from_millis(700)) => {}
                    _ = done_rx.changed() => break,
                }
            }
            if open {
                agent.emit(AgentEvent::PortChanged { port, open: false, run_id: Some(run_id) });
            }
        })
    };
    ctx.system(format!("Starting \"{}\" on port {}", setup.command, setup.port));
    let end = supervise(&mut ctx, cmd, &project).await;
    let _ = done_tx.send(true);
    let _ = probe.await;
    let mut r = ctx.result(if end.outcome == RunOutcome::Failed { RunOutcome::Failed } else { RunOutcome::Stopped });
    r.exit_code = end.exit_code;
    r.output_tail = end.tail;
    r.summary = Some(Summary { headline: "The dev server stopped".into(), values: vec![] });
    r
}

// ---------------------------------------------------------------------------------------
// The scripted stand-in for a provider's coding agent tool.
// ---------------------------------------------------------------------------------------

pub async fn mock_main(args: Vec<String>) -> anyhow::Result<()> {
    let mut project = None;
    let mut prompt = String::new();
    let mut it = args.into_iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--project" => project = it.next().map(PathBuf::from),
            "--prompt" => prompt = it.next().unwrap_or_default(),
            _ => {}
        }
    }
    let project = project.ok_or_else(|| anyhow::anyhow!("--project is required"))?;
    let pause = Duration::from_millis(
        std::env::var("CRONCAVE_MOCK_CODER_PAUSE_MS").ok().and_then(|v| v.parse().ok()).unwrap_or(700),
    );
    let say = |s: &str| println!("{s}");

    say(&format!("Task: {prompt}"));
    say("Looking at the project…");
    tokio::time::sleep(pause).await;
    let mut files = BTreeSet::new();
    files_in(&project, &project, &mut files);
    for f in files.iter().take(12) {
        say(&format!("  {f}"));
    }
    say("Plan: 1) update the page heading  2) add a footer  3) note the change in CHANGELOG.md");
    tokio::time::sleep(pause).await;

    let heading = heading_from(&prompt);
    let index = project.join("index.html");
    let html = std::fs::read_to_string(&index).unwrap_or_else(|_| {
        "<!doctype html>\n<html>\n<head>\n<meta charset=\"utf-8\">\n<title>My site</title>\n<link rel=\"stylesheet\" href=\"style.css\">\n</head>\n<body>\n<h1>My site</h1>\n</body>\n</html>\n".into()
    });
    let re = regex::Regex::new(r"(?s)<h1[^>]*>.*?</h1>").expect("valid regex");
    let mut html = if re.is_match(&html) {
        re.replace(&html, format!("<h1>{heading}</h1>").as_str()).into_owned()
    } else {
        html.replacen("<body>", &format!("<body>\n<h1>{heading}</h1>"), 1)
    };
    if !html.contains("class=\"site-footer\"") {
        html = html.replacen("</body>", "<footer class=\"site-footer\">Made on Croncave</footer>\n</body>", 1);
    }
    std::fs::write(&index, html)?;
    say("Edited index.html: new heading and a footer");
    tokio::time::sleep(pause).await;

    let css = project.join("style.css");
    let mut style = std::fs::read_to_string(&css).unwrap_or_default();
    if !style.contains(".site-footer") {
        style.push_str("\n.site-footer {\n  margin-top: 3rem;\n  font-size: 0.85rem;\n  opacity: 0.7;\n}\n");
        std::fs::write(&css, style)?;
        say("Edited style.css: footer style");
    }
    tokio::time::sleep(pause).await;

    let log = project.join("CHANGELOG.md");
    let mut changelog = std::fs::read_to_string(&log).unwrap_or_else(|_| "# Changelog\n".into());
    changelog.push_str(&format!("\n- {}: {}\n", chrono::Utc::now().format("%Y-%m-%d"), prompt.trim()));
    std::fs::write(&log, changelog)?;
    say("Updated CHANGELOG.md");
    tokio::time::sleep(pause).await;

    // Ask before running anything, the way the real tools do.
    let request_id = uuid::Uuid::new_v4().simple().to_string();
    println!(
        "{APPROVAL_PREFIX}{}",
        json!({ "request_id": request_id, "command": "ls -la", "reason": "List the project's files to check nothing else needs updating." })
    );
    say("Waiting for approval to run: ls -la");
    let mut stdin = tokio::io::BufReader::new(tokio::io::stdin()).lines();
    let approved = loop {
        match stdin.next_line().await? {
            Some(line) if line.ends_with(&request_id) => break line.starts_with("approved"),
            Some(_) => continue,
            None => break false,
        }
    };
    if approved {
        say("Approved. Running: ls -la");
        let out = Command::new("ls").arg("-la").current_dir(&project).output().await?;
        print!("{}", String::from_utf8_lossy(&out.stdout));
    } else {
        say("Not approved, so I skipped that command.");
    }
    say("Done. Review the changes in the Code app; nothing lands until you keep it.");
    Ok(())
}

fn heading_from(prompt: &str) -> String {
    let re = regex::Regex::new(r#"(?i)\bsays?\s+["“']?([^"”']+)["”']?\s*$"#).expect("valid regex");
    let raw =
        re.captures(prompt.trim()).and_then(|c| c.get(1)).map(|m| m.as_str().trim().to_string()).unwrap_or_else(|| {
            let mut s: String = prompt.trim().chars().take(60).collect();
            if let Some(f) = s.get(..1) {
                s = f.to_uppercase() + &s[1..];
            }
            s
        });
    raw.replace('<', "&lt;").replace('>', "&gt;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heading_comes_from_the_prompt() {
        assert_eq!(heading_from("Make the heading say \"Fresh bread daily\""), "Fresh bread daily");
        assert_eq!(heading_from("add a <b>bold</b> intro"), "Add a &lt;b&gt;bold&lt;/b&gt; intro");
    }

    #[test]
    fn changed_files_compares_contents() {
        let d = tempfile::tempdir().unwrap();
        let (base, proj) = (d.path().join("base"), d.path().join("proj"));
        std::fs::create_dir_all(proj.join("node_modules")).unwrap();
        std::fs::write(proj.join("same.txt"), "x").unwrap();
        std::fs::write(proj.join("edit.txt"), "a").unwrap();
        std::fs::write(proj.join("gone.txt"), "g").unwrap();
        copy_tree(&proj, &base).unwrap();
        std::fs::write(proj.join("edit.txt"), "b").unwrap();
        std::fs::remove_file(proj.join("gone.txt")).unwrap();
        std::fs::write(proj.join("new.txt"), "n").unwrap();
        std::fs::write(proj.join("node_modules/dep.js"), "ignored").unwrap();
        assert_eq!(
            changed_files(&base, &proj),
            vec![
                ("edit.txt".to_string(), ChangeKind::Changed),
                ("gone.txt".to_string(), ChangeKind::Deleted),
                ("new.txt".to_string(), ChangeKind::Created),
            ]
        );
    }
}
