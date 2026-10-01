//! The Scripts app's worker: runs Python, Node.js and shell scripts, with packages
//! installed per script, and reports a summary and the files the run touched.

use std::path::{Path, PathBuf};

use croncave_proto::{RunOutcome, RunResult, Runtime, ScriptSetup, Summary, SummaryValue};
use serde_json::Value;
use tokio::process::Command;

use crate::runs::{RunCtx, supervise};

pub async fn run(mut ctx: RunCtx) -> RunResult {
    let setup: ScriptSetup = match serde_json::from_value(ctx.spec.setup.clone()) {
        Ok(s) => s,
        Err(e) => return failed(&ctx, format!("The job's setup couldn't be read: {e}")),
    };
    let disk = ctx.agent.disk.clone();
    let script = match disk.resolve(&setup.path) {
        Ok(p) if p.is_file() => p,
        _ => return failed(&ctx, format!("The script \"{}\" wasn't found in your files.", setup.path)),
    };
    let Some(runtime) = setup.runtime.or_else(|| Runtime::for_path(&setup.path)) else {
        return failed(&ctx, "Choose whether this is a Python, Node.js or shell script.".into());
    };
    let workdir = script.parent().map(Path::to_path_buf).unwrap_or_else(|| disk.root());
    let run_dir = disk.system("runs").join(ctx.spec.run_id.to_string());
    let _ = std::fs::create_dir_all(&run_dir);
    let summary_path = run_dir.join("summary.json");

    let mut env: Vec<(String, String)> = Vec::new();
    match install_packages(&ctx, runtime, &workdir).await {
        Ok(Some((k, v))) => env.push((k, v)),
        Ok(None) => {}
        Err(e) => return failed(&ctx, e),
    }

    // Snapshot after installing packages, so the run's own changes are what we report.
    let snapshot =
        match crate::snapshot::take(&disk.root(), &disk.system("snapshots").join(ctx.spec.run_id.to_string())) {
            Ok(s) => s,
            Err(e) => return failed(&ctx, format!("Couldn't prepare the run: {e}")),
        };

    let mut cmd = match runtime {
        Runtime::Python => {
            let mut c = Command::new(std::env::var("CRONCAVE_PYTHON").unwrap_or_else(|_| "python3".into()));
            c.arg("-u").arg(&script);
            c
        }
        Runtime::Node => {
            let mut c = Command::new("node");
            c.arg(&script);
            c
        }
        Runtime::Shell => {
            let mut c = Command::new("bash");
            c.arg(&script);
            c
        }
    };
    cmd.args(&setup.args)
        .env("CRONCAVE_RUN_ID", ctx.spec.run_id.to_string())
        .env("CRONCAVE_JOB", &ctx.spec.job_name)
        .env("CRONCAVE_SUMMARY", &summary_path)
        .env("CRONCAVE_FILES", disk.root())
        .env("PYTHONUNBUFFERED", "1");
    for (k, v) in env {
        cmd.env(k, v);
    }
    ctx.system(format!("Running {} with {}", setup.path, runtime_name(runtime)));
    let end = supervise(&mut ctx, cmd, &workdir).await;
    let changes = snapshot.finish(&disk, &format!("run:{}", ctx.spec.run_id));
    let summary = read_summary(&summary_path);
    let _ = std::fs::remove_dir_all(&run_dir);

    let mut r = ctx.result(end.outcome);
    r.exit_code = end.exit_code;
    r.output_tail = end.tail;
    r.changes = changes;
    r.summary = summary;
    r
}

fn failed(ctx: &RunCtx, error: String) -> RunResult {
    ctx.system(&error);
    let mut r = ctx.result(RunOutcome::Failed);
    r.output_tail = error.clone();
    r.error = Some(error);
    r
}

fn runtime_name(r: Runtime) -> &'static str {
    match r {
        Runtime::Python => "Python",
        Runtime::Node => "Node.js",
        Runtime::Shell => "the shell",
    }
}

/// Install packages from requirements.txt or package.json beside the script. Python
/// packages go into a per-requirements folder in the app's own area, reused across runs.
async fn install_packages(ctx: &RunCtx, runtime: Runtime, workdir: &Path) -> Result<Option<(String, String)>, String> {
    match runtime {
        Runtime::Python => {
            let req = workdir.join("requirements.txt");
            let Ok(body) = std::fs::read(&req) else { return Ok(None) };
            let key = format!("{:016x}", fnv(&body));
            let target: PathBuf = ctx.agent.disk.system("apps/scripts/python-packages").join(key);
            if !target.join(".installed").exists() {
                ctx.system("Installing packages from requirements.txt…");
                let out = Command::new(std::env::var("CRONCAVE_PYTHON").unwrap_or_else(|_| "python3".into()))
                    .args(["-m", "pip", "install", "--quiet", "--disable-pip-version-check", "--target"])
                    .arg(&target)
                    .arg("-r")
                    .arg(&req)
                    .output()
                    .await
                    .map_err(|e| format!("Couldn't start pip: {e}"))?;
                if !out.status.success() {
                    let err = String::from_utf8_lossy(&out.stderr);
                    ctx.system(err.trim());
                    return Err(
                        "Installing the packages in requirements.txt failed. Check the package names and versions."
                            .into(),
                    );
                }
                let _ = std::fs::write(target.join(".installed"), "");
                ctx.system("Packages installed.");
            }
            Ok(Some(("PYTHONPATH".into(), target.to_string_lossy().into_owned())))
        }
        Runtime::Node => {
            if workdir.join("package.json").exists() && !workdir.join("node_modules").exists() {
                ctx.system("Installing packages from package.json…");
                let out = Command::new("npm")
                    .args(["install", "--no-audit", "--no-fund", "--loglevel=error"])
                    .current_dir(workdir)
                    .output()
                    .await
                    .map_err(|e| format!("Couldn't start npm: {e}"))?;
                if !out.status.success() {
                    ctx.system(String::from_utf8_lossy(&out.stderr).trim());
                    return Err("Installing the packages in package.json failed.".into());
                }
                ctx.system("Packages installed.");
            }
            Ok(None)
        }
        Runtime::Shell => Ok(None),
    }
}

fn fnv(data: &[u8]) -> u64 {
    data.iter().fold(0xcbf29ce484222325u64, |h, b| (h ^ *b as u64).wrapping_mul(0x100000001b3))
}

/// The summary a script may write to `$CRONCAVE_SUMMARY`:
/// `{"headline": "...", "values": {"Rows": 120}}` or `"values": [{"label": .., "value": ..}]`.
pub fn read_summary(path: &Path) -> Option<Summary> {
    let v: Value = serde_json::from_slice(&std::fs::read(path).ok()?).ok()?;
    let headline = v.get("headline").and_then(Value::as_str).unwrap_or_default().chars().take(200).collect::<String>();
    let show = |v: &Value| match v {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    };
    let values = match v.get("values") {
        Some(Value::Object(m)) => m.iter().map(|(k, v)| SummaryValue { label: k.clone(), value: show(v) }).collect(),
        Some(Value::Array(a)) => a
            .iter()
            .filter_map(|e| {
                Some(SummaryValue { label: e.get("label")?.as_str()?.to_string(), value: show(e.get("value")?) })
            })
            .collect(),
        _ => vec![],
    };
    if headline.is_empty() && values.is_empty() {
        return None;
    }
    Some(Summary { headline, values: values.into_iter().take(12).collect() })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summary_accepts_both_value_shapes() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("s.json");
        std::fs::write(&p, r#"{"headline":"3 new rows","values":{"Rows":3,"Source":"api"}}"#).unwrap();
        let s = read_summary(&p).unwrap();
        assert_eq!(s.headline, "3 new rows");
        assert_eq!(s.values.len(), 2);
        std::fs::write(&p, r#"{"values":[{"label":"Total","value":"$4"}]}"#).unwrap();
        assert_eq!(read_summary(&p).unwrap().values[0].value, "$4");
        std::fs::write(&p, "not json").unwrap();
        assert!(read_summary(&p).is_none());
    }
}
