//! Why a run failed, in plain words, with the fix beside it.

use regex::Regex;

pub struct Explanation {
    pub plain: String,
    pub fix: String,
}

fn ex(plain: impl Into<String>, fix: impl Into<String>) -> Option<Explanation> {
    Some(Explanation { plain: plain.into(), fix: fix.into() })
}

pub fn explain(
    app: &str,
    outcome: &str,
    exit_code: Option<i32>,
    output: &str,
    error: Option<&str>,
    max_runtime_secs: i64,
) -> Option<Explanation> {
    if outcome == "succeeded" || outcome == "stopped" {
        return None;
    }
    if outcome == "timed_out" {
        let limit = crate::jobs::human_duration(max_runtime_secs);
        return ex(
            format!("It ran longer than its limit of {limit}, so it was stopped."),
            "Raise the longest run time in the job's setup, or make the work faster.",
        );
    }
    let text = format!("{}\n{}", error.unwrap_or_default(), output);
    if app == "watcher" {
        let why = error.unwrap_or("The check didn't finish.");
        return ex(
            format!("Couldn't check: {why}"),
            "Test the watch to see what it reads. If the page moved or changed, update its address or the part of the page it watches.",
        );
    }
    let re = |p: &str| Regex::new(p).expect("valid regex");
    if let Some(c) = re(r"ModuleNotFoundError: No module named '([^'.]+)").captures(&text) {
        let m = &c[1];
        return ex(
            format!("The script needs the Python package \"{m}\", which isn't installed."),
            format!(
                "Add {m} to a requirements.txt file next to the script, then retry. Packages install automatically."
            ),
        );
    }
    if let Some(c) = re(r"Cannot find module '([^']+)'").captures(&text) {
        let m = &c[1];
        return ex(
            format!("The script needs the package \"{m}\", which isn't installed."),
            format!("Add {m} to package.json next to the script, then retry."),
        );
    }
    if let Some(c) = re(r#"(?:FileNotFoundError|No such file or directory)[^\n]*?['"]([^'"\n]+)['"]"#).captures(&text) {
        return ex(
            format!("The script looked for \"{}\", which isn't there.", &c[1]),
            "Check the name and folder in Files. Paths are relative to the script's own folder.",
        );
    }
    if text.contains("No such file or directory") || text.contains("FileNotFoundError") {
        return ex(
            "The script looked for a file that isn't there.",
            "Check the name and folder in Files. Paths are relative to the script's own folder.",
        );
    }
    if let Some(c) = re(r"SyntaxError[^\n]*\n?[^\n]*line (\d+)")
        .captures(&text)
        .or_else(|| re(r#"line (\d+)[^\n]*\n(?:[^\n]*\n){0,3}?\s*SyntaxError"#).captures(&text))
    {
        return ex(
            format!("The script has a mistake in how it's written, on line {}.", &c[1]),
            "Open the script in Code, fix that line (the output shows where), then retry.",
        );
    }
    if text.contains("SyntaxError") {
        return ex(
            "The script has a mistake in how it's written.",
            "Open the script in Code and fix the line the output points to, then retry.",
        );
    }
    if exit_code == Some(127) || text.contains("command not found") {
        return ex(
            "A program the script calls isn't installed on this computer.",
            "Check the command's spelling, or install it as a package first.",
        );
    }
    if text.contains("Permission denied") {
        return ex(
            "The script tried to use a file or folder it isn't allowed to.",
            "Keep the script's files inside your own folders in Files.",
        );
    }
    if text.contains("Killed") || exit_code == Some(137) {
        return ex(
            "The computer ran out of memory while running this.",
            "Use a bigger computer size, or process the data in smaller pieces.",
        );
    }
    if text.contains("requirements.txt failed") || text.contains("package.json failed") {
        return ex(
            "Installing the script's packages failed.",
            "Check the package names and versions in requirements.txt or package.json.",
        );
    }
    if let Some(e) = error.filter(|e| !e.is_empty()) {
        return ex(e.to_string(), "Fix the setup and retry.");
    }
    let last = output.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("").trim();
    if !last.is_empty() {
        return ex(
            format!(
                "It stopped with an error{}: {}",
                exit_code.map(|c| format!(" (exit code {c})")).unwrap_or_default(),
                last.chars().take(200).collect::<String>()
            ),
            "Read the last lines of the output to see what went wrong, fix it, then retry.",
        );
    }
    ex(
        format!("It stopped with exit code {}.", exit_code.map(|c| c.to_string()).unwrap_or_else(|| "unknown".into())),
        "Look at the output to see why, then retry.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_python_package_names_the_fix() {
        let out = "Traceback (most recent call last):\n  File \"a.py\", line 1\nModuleNotFoundError: No module named 'requests'";
        let e = explain("scripts", "failed", Some(1), out, None, 60).unwrap();
        assert!(e.plain.contains("\"requests\""));
        assert!(e.fix.contains("requirements.txt"));
    }

    #[test]
    fn timeouts_and_missing_files_read_plainly() {
        let e = explain("scripts", "timed_out", None, "", None, 300).unwrap();
        assert!(e.plain.contains("5 minutes"));
        let e = explain(
            "scripts",
            "failed",
            Some(1),
            "FileNotFoundError: [Errno 2] No such file or directory: 'data/in.csv'",
            None,
            60,
        )
        .unwrap();
        assert!(e.plain.contains("data/in.csv"), "{}", e.plain);
    }

    #[test]
    fn successes_have_nothing_to_explain() {
        assert!(explain("scripts", "succeeded", Some(0), "ok", None, 60).is_none());
    }

    #[test]
    fn otherwise_the_last_line_is_shown() {
        let e = explain("scripts", "failed", Some(2), "working\nValueError: bad row 7\n", None, 60).unwrap();
        assert!(e.plain.contains("bad row 7"));
        assert!(e.plain.contains("exit code 2"));
    }
}
