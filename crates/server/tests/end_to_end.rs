//! The whole path in one process: sign-up, a computer that wakes over its outgoing
//! connection, Files with Trash and resumable uploads, Scripts with a summary and
//! changed files, file triggers, Watcher, sleep with the disk kept, and an agent task.

mod common;

use std::time::Duration;

use serde_json::{Value, json};

async fn computer(s: &common::Stack) -> String {
    let c = s.post("/computers", json!({ "name": "Test box", "size": "small" })).await;
    let id = c["id"].as_str().unwrap().to_string();
    s.wait_for("/me", "the computer to wake", |v| v["computers"][0]["state"] == "awake").await;
    id
}

#[tokio::test]
async fn files_trash_and_resumable_uploads() {
    let s = common::stack().await;
    s.sign_up("ana@example.com", "4155550111", "free", None, false).await;
    let c = computer(&s).await;

    // A two-chunk upload that "drops" after the first chunk and resumes from what arrived.
    let body: Vec<u8> = (0..1_500_000u32).map(|i| (i % 251) as u8).collect();
    let up = s.post(&format!("/computers/{c}/uploads"), json!({ "path": "Data/blob.bin", "size": body.len() })).await;
    let id = up["upload_id"].as_str().unwrap();
    s.put_bytes(&format!("/computers/{c}/uploads/{id}?offset=0"), body[..1_000_000].to_vec()).await;
    let wrong = s.put_bytes(&format!("/computers/{c}/uploads/{id}?offset=0"), body[..10].to_vec()).await;
    assert_eq!(wrong["mismatch"], true, "a resend from the wrong offset is refused");
    let status = s.get(&format!("/computers/{c}/uploads/{id}")).await;
    let have = status["received"].as_u64().unwrap() as usize;
    assert_eq!(have, 1_000_000);
    s.put_bytes(&format!("/computers/{c}/uploads/{id}?offset={have}"), body[have..].to_vec()).await;
    let done = s.post(&format!("/computers/{c}/uploads/{id}/finish"), json!({ "path": "Data/blob.bin" })).await;
    assert_eq!(done["size"], 1_500_000);
    let dl = s
        .client
        .get(format!("{}/api/computers/{c}/files/download?path=Data/blob.bin", s.base))
        .send()
        .await
        .unwrap()
        .bytes()
        .await
        .unwrap();
    assert_eq!(dl.to_vec(), body, "the download matches the upload byte for byte");

    // Preview a table, then delete it to Trash and restore it.
    s.put_bytes(&format!("/computers/{c}/files/write?path=Data/t.csv"), b"a,b\n1,2\n3,4\n".to_vec()).await;
    let p = s.get(&format!("/computers/{c}/files/preview?path=Data/t.csv")).await;
    assert_eq!(p["kind"], "table");
    assert_eq!(p["total_rows"], 2);
    s.post(&format!("/computers/{c}/files/delete"), json!({ "path": "Data/t.csv" })).await;
    let trash = s.get(&format!("/computers/{c}/files/trash")).await;
    let item = &trash["items"][0];
    assert_eq!(item["original_path"], "Data/t.csv");
    assert_eq!(item["deleted_by_label"], "Deleted by you");
    s.post(&format!("/computers/{c}/files/restore"), json!({ "trash_id": item["id"] })).await;
    let list = s.get(&format!("/computers/{c}/files?path=Data")).await;
    let names: Vec<&str> = list["entries"].as_array().unwrap().iter().map(|e| e["name"].as_str().unwrap()).collect();
    assert_eq!(names, vec!["blob.bin", "t.csv"]);
}

#[tokio::test]
async fn scripts_report_results_and_trigger_on_file_changes() {
    let s = common::stack().await;
    s.sign_up("bo@example.com", "4155550112", "free", None, false).await;
    let c = computer(&s).await;
    s.post(&format!("/computers/{c}/scripts/template"), json!({ "template": "csv-report" })).await;
    let made = s
        .post(
            &format!("/computers/{c}/scripts"),
            json!({ "name": "Report", "path": "Scripts/csv-report/report.py", "run_now": true }),
        )
        .await;
    let run = s.wait_run(made["run_id"].as_str().unwrap()).await;
    assert_eq!(run["status"], "succeeded", "{run}");
    assert_eq!(run["headline"], "10 sales summarized; East sold the most");
    assert_eq!(run["changes"][0]["path"], "Scripts/csv-report/summary.csv");
    let list = s.get(&format!("/computers/{c}/files?path=Scripts/csv-report")).await;
    let summary = list["entries"].as_array().unwrap().iter().find(|e| e["name"] == "summary.csv").unwrap().clone();
    assert_eq!(summary["source"]["label"], "Made by \"Report\"");

    // A script that deletes a file: Trash catches it, attributed to the run.
    s.put_bytes(
        &format!("/computers/{c}/files/write?path=Scripts/clean.sh"),
        b"rm -f old.txt\necho cleaned\n".to_vec(),
    )
    .await;
    s.put_bytes(&format!("/computers/{c}/files/write?path=Scripts/old.txt"), b"bye".to_vec()).await;
    let clean = s
        .post(
            &format!("/computers/{c}/scripts"),
            json!({ "name": "Clean", "path": "Scripts/clean.sh", "run_now": true }),
        )
        .await;
    let run = s.wait_run(clean["run_id"].as_str().unwrap()).await;
    assert_eq!(run["status"], "succeeded", "{run}");
    let trash = s.get(&format!("/computers/{c}/files/trash")).await;
    assert_eq!(trash["items"][0]["deleted_by_label"], "Deleted by \"Clean\"");

    // A failure explained in plain words, with the fix.
    s.put_bytes(&format!("/computers/{c}/files/write?path=Scripts/broken.py"), b"import notapackage\n".to_vec()).await;
    let broken = s
        .post(
            &format!("/computers/{c}/scripts"),
            json!({ "name": "Broken", "path": "Scripts/broken.py", "run_now": true }),
        )
        .await;
    let run = s.wait_run(broken["run_id"].as_str().unwrap()).await;
    assert_eq!(run["status"], "failed");
    assert!(run["error_plain"].as_str().unwrap().contains("notapackage"), "{run}");
    assert!(run["error_fix"].as_str().unwrap().contains("requirements.txt"));

    // "When files change": writing into Inbox runs the job.
    s.put_bytes(
        &format!("/computers/{c}/files/write?path=Scripts/count.sh"),
        b"ls \"$CRONCAVE_FILES/Inbox\" | wc -l\n".to_vec(),
    )
    .await;
    let job = s
        .post(
            &format!("/computers/{c}/scripts"),
            json!({ "name": "Count", "path": "Scripts/count.sh", "trigger": "files", "watch_path": "Inbox" }),
        )
        .await;
    let job_id = job["job"]["id"].as_str().unwrap().to_string();
    s.put_bytes(&format!("/computers/{c}/files/write?path=Inbox/a.txt"), b"x".to_vec()).await;
    let j = s
        .wait_for(&format!("/jobs/{job_id}"), "the file trigger", |v| {
            v["runs"].as_array().is_some_and(|r| !r.is_empty())
        })
        .await;
    assert_eq!(j["runs"][0]["trigger"], "files");
}

#[tokio::test]
async fn watches_find_planted_changes_and_tests_save_nothing() {
    let s = common::stack().await;
    s.sign_up("cy@example.com", "4155550113", "free", None, false).await;
    let c = computer(&s).await;
    let base = s.base.clone();
    let url = format!("{base}/demo/listings");
    let w = s
        .post(&format!("/computers/{c}/watches"), json!({ "type_id": "demo-listings", "name": "Flats", "inputs": { "url": url, "max_price": "2200", "min_beds": "" }, "status": "draft" }))
        .await;
    let job = w["job"]["id"].as_str().unwrap().to_string();
    let test = s.post(&format!("/jobs/{job}/test"), json!({})).await;
    let t = s.wait_run(test["run_id"].as_str().unwrap()).await;
    assert_eq!(t["status"], "succeeded", "{t}");
    assert_eq!(t["data"]["test"], true);
    // A blank input takes the type's default (at least 1 bedroom), so one listing matches.
    assert_eq!(t["headline"], "1 match on the first check");

    s.client.patch(format!("{base}/api/jobs/{job}")).json(&json!({ "status": "active" })).send().await.unwrap();
    let first = s.post(&format!("/jobs/{job}/run"), json!({})).await;
    let r = s.wait_run(first["run_id"].as_str().unwrap()).await;
    assert_eq!(r["headline"], "1 match on the first check", "the test didn't save state");
    // Plant listings until one is under $2,200.
    for _ in 0..4 {
        s.post("/dev/demo/listing", json!({})).await;
    }
    let again = s.post(&format!("/jobs/{job}/run"), json!({})).await;
    let r = s.wait_run(again["run_id"].as_str().unwrap()).await;
    assert!(r["headline"].as_str().unwrap().contains("new match"), "{r}");
    let events = s.get("/events").await;
    assert!(events["events"].as_array().unwrap().iter().any(|e| e["kind"] == "watch.found"));

    // A page that moved is "couldn't check", with a repair path.
    let gone = s
        .post(&format!("/computers/{c}/watches"), json!({ "type_id": "web-page", "name": "Gone", "inputs": { "url": format!("{base}/demo/nope") }, "check_now": true }))
        .await;
    let r = s.wait_run(gone["run_id"].as_str().unwrap()).await;
    assert_eq!(r["status"], "failed");
    assert!(r["error_plain"].as_str().unwrap().starts_with("Couldn't check"), "{r}");
}

#[tokio::test]
async fn computers_sleep_keep_their_disk_and_wake_for_work() {
    let s = common::stack().await;
    s.sign_up("di@example.com", "4155550114", "free", None, false).await;
    let c = computer(&s).await;
    s.put_bytes(&format!("/computers/{c}/files/write?path=keep.txt"), b"kept".to_vec()).await;
    sqlx::query("delete from presence").execute(&s.app.db).await.unwrap();
    s.post(&format!("/computers/{c}/action"), json!({ "action": "sleep" })).await;
    let me = s.get("/me").await;
    assert_eq!(me["computers"][0]["state"], "asleep");
    // Opening Files wakes it; the file is still there.
    let p = s.get(&format!("/computers/{c}/files/preview?path=keep.txt")).await;
    assert_eq!(p["text"], "kept");
    let wakes: Vec<(i32,)> = sqlx::query_as("select ms from wake_measurements").fetch_all(&s.app.db).await.unwrap();
    assert!(wakes.len() >= 2);
    assert!(wakes.iter().all(|(ms,)| *ms < 5000), "wake target: under 5 seconds ({wakes:?})");
}

#[tokio::test]
async fn an_agent_task_asks_before_running_and_its_changes_are_reviewed() {
    let s = common::stack().await;
    s.sign_up("ed@example.com", "4155550115", "free", None, false).await;
    let c = computer(&s).await;
    s.post(&format!("/computers/{c}/code/projects"), json!({ "name": "site" })).await;
    let t = s
        .post(
            &format!("/computers/{c}/code/tasks"),
            json!({ "project": "Projects/site", "prompt": "Make the heading say \"Fresh bread\"" }),
        )
        .await;
    let run = t["run_id"].as_str().unwrap().to_string();
    let r = s
        .wait_for(&format!("/runs/{run}"), "the approval request", |v| {
            v["approvals"].as_array().is_some_and(|a| !a.is_empty())
        })
        .await;
    let req = r["approvals"][0]["request_id"].as_str().unwrap();
    assert_eq!(r["approvals"][0]["command"], "ls -la");
    let home = s.get("/home").await;
    assert_eq!(home["needs_you"]["approvals"].as_array().unwrap().len(), 1);
    s.post(&format!("/runs/{run}/approvals/{req}"), json!({ "approved": true })).await;
    let done = s.wait_run(&run).await;
    assert_eq!(done["status"], "succeeded", "{done}");
    let review = s.get(&format!("/runs/{run}/review")).await;
    let files: Vec<&Value> = review["files"].as_array().unwrap().iter().collect();
    assert_eq!(files.len(), 3);
    let index = files.iter().find(|f| f["path"] == "index.html").unwrap();
    assert!(index["after"].as_str().unwrap().contains("<h1>Fresh bread</h1>"));
    s.post(&format!("/runs/{run}/review"), json!({ "path": "index.html", "decision": "keep" })).await;
    s.post(&format!("/runs/{run}/review"), json!({ "path": "CHANGELOG.md", "decision": "undo" })).await;
    let list = s.get(&format!("/computers/{c}/files?path=Projects/site")).await;
    assert!(
        !list["entries"].as_array().unwrap().iter().any(|e| e["name"] == "CHANGELOG.md"),
        "undoing a created file removes it"
    );
    tokio::time::sleep(Duration::from_millis(50)).await;
}
