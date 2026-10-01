//! The Files app's worker, built into the agent: browsing, previews, Trash and uploads.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use bytes::Bytes;
use croncave_proto::{DiskUsage, FileEntry, FilePreview, FilesRequest, TrashEntry};
use serde_json::json;
use tokio::io::AsyncReadExt;

use crate::Agent;
use crate::disk::{Disk, clean, rel_string, size_of};
use crate::stream::AgentStream;

const MAX_PREVIEW_ROWS: usize = 50;
const MAX_TEXT_PREVIEW: usize = 64 * 1024;
const MAX_IMAGE_EDGE: u32 = 1024;
const MAX_WRITE: usize = 8 * 1024 * 1024;

pub async fn handle(agent: &Agent, req: FilesRequest, s: &mut AgentStream) {
    let disk = &agent.disk;
    let result: Result<(), String> = async {
        match req {
            FilesRequest::List { path } => {
                let dir = disk.resolve(&path)?;
                let entries = list(disk, &dir)?;
                s.send_json(&entries).await;
            }
            FilesRequest::Preview { path } => {
                let file = disk.resolve(&path)?;
                let (head, body) =
                    tokio::task::spawn_blocking(move || preview(&file)).await.map_err(|e| e.to_string())??;
                s.send_json(&head).await;
                if let Some(b) = body {
                    s.send(Bytes::from(b)).await;
                }
            }
            FilesRequest::Download { path } => {
                let target = disk.resolve(&path)?;
                let meta = std::fs::metadata(&target).map_err(|_| format!("\"{path}\" wasn't found"))?;
                let base = target
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "My computer".into());
                // A folder downloads as a zip, made in the system area and removed after.
                let (file, name, zipped) = if meta.is_dir() {
                    let tmp = disk.system("tmp").join(format!("{}.zip", uuid::Uuid::new_v4().simple()));
                    let (src, dst) = (target.clone(), tmp.clone());
                    tokio::task::spawn_blocking(move || zip_dir(&src, &dst)).await.map_err(|e| e.to_string())??;
                    (tmp, format!("{base}.zip"), true)
                } else {
                    (target.clone(), base, false)
                };
                let size = std::fs::metadata(&file).map(|m| m.len()).unwrap_or(0);
                s.send_json(&json!({ "size": size, "name": name, "zip": zipped })).await;
                let mut f = tokio::fs::File::open(&file).await.map_err(|e| e.to_string())?;
                let mut buf = vec![0u8; 256 * 1024];
                loop {
                    let n = f.read(&mut buf).await.map_err(|e| e.to_string())?;
                    if n == 0 || !s.send(Bytes::copy_from_slice(&buf[..n])).await {
                        break;
                    }
                }
                if zipped {
                    let _ = std::fs::remove_file(&file);
                }
            }
            FilesRequest::Recent { limit } => {
                let root = disk.root();
                let found = tokio::task::spawn_blocking(move || recent(&root, limit.min(200)))
                    .await
                    .map_err(|e| e.to_string())?;
                let entries: Vec<FileEntry> = found.iter().filter_map(|p| entry(disk, p).ok()).collect();
                s.send_json(&entries).await;
            }
            FilesRequest::Mkdir { path } => {
                let dir = disk.resolve(&path)?;
                std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
                s.send_json(&entry(disk, &dir)?).await;
            }
            FilesRequest::Move { from, to } => {
                let src = disk.resolve(&from)?;
                let dst = disk.resolve(&to)?;
                if !src.exists() {
                    return Err(format!("\"{from}\" wasn't found"));
                }
                if dst.exists() {
                    return Err(format!("\"{to}\" already exists"));
                }
                if let Some(p) = dst.parent() {
                    std::fs::create_dir_all(p).map_err(|e| e.to_string())?;
                }
                std::fs::rename(&src, &dst).map_err(|e| e.to_string())?;
                s.send_json(&entry(disk, &dst)?).await;
            }
            FilesRequest::Delete { path, by } => {
                let target = disk.resolve(&path)?;
                if target == disk.root() {
                    return Err("Your root folder can't be deleted.".into());
                }
                let t = to_trash(disk, &target, &by)?;
                s.send_json(&t).await;
            }
            FilesRequest::TrashList => {
                s.send_json(&trash_list(disk)).await;
            }
            FilesRequest::Restore { trash_id } => {
                let restored = restore(disk, &trash_id)?;
                s.send_json(&entry(disk, &restored)?).await;
            }
            FilesRequest::EmptyTrash => {
                let dir = disk.system("trash");
                let _ = std::fs::remove_dir_all(&dir);
                disk.system("trash");
                s.send_json(&json!({ "ok": true })).await;
            }
            FilesRequest::UploadChunk { upload_id, offset } => {
                let part = upload_part(disk, &upload_id)?;
                let have = std::fs::metadata(&part).map(|m| m.len()).unwrap_or(0);
                if have != offset {
                    // The client resumes from what we really have.
                    let _ = s.read_body(usize::MAX).await;
                    s.send_json(&json!({ "received": have, "mismatch": true })).await;
                    return Ok(());
                }
                let mut f =
                    std::fs::OpenOptions::new().create(true).append(true).open(&part).map_err(|e| e.to_string())?;
                let mut received = have;
                while let Some(chunk) = s.recv().await? {
                    f.write_all(&chunk).map_err(|e| e.to_string())?;
                    received += chunk.len() as u64;
                }
                f.sync_data().map_err(|e| e.to_string())?;
                s.send_json(&json!({ "received": received })).await;
            }
            FilesRequest::UploadStatus { upload_id } => {
                let part = upload_part(disk, &upload_id)?;
                let have = std::fs::metadata(&part).map(|m| m.len()).unwrap_or(0);
                s.send_json(&json!({ "received": have })).await;
            }
            FilesRequest::UploadFinish { upload_id, path, extract } => {
                let part = upload_part(disk, &upload_id)?;
                if !part.exists() {
                    std::fs::File::create(&part).map_err(|e| e.to_string())?;
                }
                let dst = disk.resolve(&path)?;
                if extract {
                    // A folder copied from another computer: unpack it in place of `path`.
                    let replaced = if dst.exists() { Some(to_trash(disk, &dst, "user")?) } else { None };
                    std::fs::create_dir_all(&dst).map_err(|e| e.to_string())?;
                    let (zip, into) = (part.clone(), dst.clone());
                    tokio::task::spawn_blocking(move || unzip(&zip, &into)).await.map_err(|e| e.to_string())??;
                    let _ = std::fs::remove_file(&part);
                    let mut e = serde_json::to_value(entry(disk, &dst)?).expect("entry serializes");
                    e["replaced"] = json!(replaced.is_some());
                    s.send_json(&e).await;
                    return Ok(());
                }
                let replaced = if dst.exists() { Some(to_trash(disk, &dst, "user")?) } else { None };
                if let Some(p) = dst.parent() {
                    std::fs::create_dir_all(p).map_err(|e| e.to_string())?;
                }
                std::fs::rename(&part, &dst).map_err(|e| e.to_string())?;
                let mut e = serde_json::to_value(entry(disk, &dst)?).expect("entry serializes");
                e["replaced"] = json!(replaced.is_some());
                s.send_json(&e).await;
            }
            FilesRequest::Write { path } => {
                let dst = disk.resolve(&path)?;
                let body = s.read_body(MAX_WRITE).await?;
                let existed = dst.exists();
                if let Some(p) = dst.parent() {
                    std::fs::create_dir_all(p).map_err(|e| e.to_string())?;
                }
                std::fs::write(&dst, body).map_err(|e| e.to_string())?;
                let mut e = serde_json::to_value(entry(disk, &dst)?).expect("entry serializes");
                e["created"] = json!(!existed);
                s.send_json(&e).await;
            }
            FilesRequest::Usage => {
                let usage =
                    DiskUsage { files_bytes: size_of(&disk.root()), trash_bytes: size_of(&disk.system("trash")) };
                s.send_json(&usage).await;
            }
        }
        Ok(())
    }
    .await;
    if let Err(e) = result {
        s.send_error(e).await;
    }
}

fn upload_part(disk: &Disk, id: &str) -> Result<PathBuf, String> {
    if id.is_empty() || !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
        return Err("bad upload id".into());
    }
    Ok(disk.system("uploads").join(format!("{id}.part")))
}

pub fn entry(disk: &Disk, path: &Path) -> Result<FileEntry, String> {
    let meta = std::fs::metadata(path).map_err(|_| format!("\"{}\" wasn't found", rel_string(&disk.root(), path)))?;
    let items = if meta.is_dir() { std::fs::read_dir(path).ok().map(|rd| rd.count() as u64) } else { None };
    Ok(FileEntry {
        items,
        name: path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
        path: rel_string(&disk.root(), path),
        is_dir: meta.is_dir(),
        size: if meta.is_dir() { 0 } else { meta.len() },
        modified_ms: meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0),
    })
}

/// Zip a folder (its contents, relative to it).
fn zip_dir(src: &Path, dst: &Path) -> Result<(), String> {
    use zip::write::SimpleFileOptions;
    let file = std::fs::File::create(dst).map_err(|e| e.to_string())?;
    let mut z = zip::ZipWriter::new(file);
    let opts = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    let mut stack = vec![src.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for e in std::fs::read_dir(&dir).map_err(|e| e.to_string())?.flatten() {
            let p = e.path();
            let rel = rel_string(src, &p);
            let ft = e.file_type().map_err(|e| e.to_string())?;
            if ft.is_dir() {
                z.add_directory(format!("{rel}/"), opts).map_err(|e| e.to_string())?;
                stack.push(p);
            } else if ft.is_file() {
                z.start_file(rel, opts).map_err(|e| e.to_string())?;
                let mut f = std::fs::File::open(&p).map_err(|e| e.to_string())?;
                std::io::copy(&mut f, &mut z).map_err(|e| e.to_string())?;
            }
        }
    }
    z.finish().map_err(|e| e.to_string())?;
    Ok(())
}

/// Unpack a zip into a folder; entries that would land outside it are refused by the zip
/// reader.
fn unzip(zip_path: &Path, into: &Path) -> Result<(), String> {
    let f = std::fs::File::open(zip_path).map_err(|e| e.to_string())?;
    let mut archive = zip::ZipArchive::new(f).map_err(|_| "That isn't a zip file.".to_string())?;
    archive.extract(into).map_err(|e| e.to_string())
}

/// The `limit` most recently changed files under `root`, newest first. Stops looking after
/// a generous number of entries so a huge disk can't stall the agent.
fn recent(root: &Path, limit: usize) -> Vec<PathBuf> {
    let mut found: Vec<(std::time::SystemTime, PathBuf)> = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    let mut seen = 0usize;
    while let Some(dir) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&dir) else { continue };
        for e in rd.flatten() {
            seen += 1;
            if seen > 50_000 {
                break;
            }
            let Ok(ft) = e.file_type() else { continue };
            if ft.is_dir() {
                if !e.file_name().to_string_lossy().starts_with('.') {
                    stack.push(e.path());
                }
            } else if ft.is_file()
                && let Ok(m) = e.metadata()
                && let Ok(t) = m.modified()
            {
                found.push((t, e.path()));
            }
        }
    }
    found.sort_by_key(|f| std::cmp::Reverse(f.0));
    found.into_iter().take(limit).map(|(_, p)| p).collect()
}

fn list(disk: &Disk, dir: &Path) -> Result<Vec<FileEntry>, String> {
    let rd = std::fs::read_dir(dir).map_err(|_| "That folder wasn't found.".to_string())?;
    let mut out: Vec<FileEntry> = rd.flatten().filter_map(|e| entry(disk, &e.path()).ok()).collect();
    out.sort_by(|a, b| b.is_dir.cmp(&a.is_dir).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase())));
    Ok(out)
}

/// Move something into Trash, remembering where it came from and who deleted it.
pub fn to_trash(disk: &Disk, target: &Path, by: &str) -> Result<TrashEntry, String> {
    let original_path = rel_string(&disk.root(), target);
    let meta = std::fs::symlink_metadata(target).map_err(|_| format!("\"{original_path}\" wasn't found"))?;
    let id = uuid::Uuid::new_v4().simple().to_string();
    let dir = disk.system("trash").join(&id);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let entry = TrashEntry {
        id,
        original_path,
        is_dir: meta.is_dir(),
        size: size_of(target),
        deleted_ms: chrono::Utc::now().timestamp_millis(),
        deleted_by: by.to_string(),
    };
    std::fs::rename(target, dir.join("item")).map_err(|e| e.to_string())?;
    std::fs::write(dir.join("meta.json"), serde_json::to_vec(&entry).expect("trash entry serializes"))
        .map_err(|e| e.to_string())?;
    Ok(entry)
}

/// Put a file the run deleted (kept by its snapshot) into Trash.
pub fn adopt_into_trash(disk: &Disk, kept: &Path, original_path: &str, by: &str) -> Result<TrashEntry, String> {
    let id = uuid::Uuid::new_v4().simple().to_string();
    let dir = disk.system("trash").join(&id);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let meta = std::fs::symlink_metadata(kept).map_err(|e| e.to_string())?;
    let entry = TrashEntry {
        id,
        original_path: original_path.to_string(),
        is_dir: meta.is_dir(),
        size: size_of(kept),
        deleted_ms: chrono::Utc::now().timestamp_millis(),
        deleted_by: by.to_string(),
    };
    std::fs::rename(kept, dir.join("item")).map_err(|e| e.to_string())?;
    std::fs::write(dir.join("meta.json"), serde_json::to_vec(&entry).expect("trash entry serializes"))
        .map_err(|e| e.to_string())?;
    Ok(entry)
}

pub fn trash_list(disk: &Disk) -> Vec<TrashEntry> {
    let mut out: Vec<TrashEntry> = std::fs::read_dir(disk.system("trash"))
        .map(|rd| {
            rd.flatten()
                .filter_map(|e| std::fs::read(e.path().join("meta.json")).ok())
                .filter_map(|b| serde_json::from_slice(&b).ok())
                .collect()
        })
        .unwrap_or_default();
    out.sort_by_key(|e: &TrashEntry| std::cmp::Reverse(e.deleted_ms));
    out
}

fn restore(disk: &Disk, id: &str) -> Result<PathBuf, String> {
    if !id.chars().all(|c| c.is_ascii_alphanumeric()) {
        return Err("bad trash id".into());
    }
    let dir = disk.system("trash").join(id);
    let meta: TrashEntry = std::fs::read(dir.join("meta.json"))
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .ok_or("That item isn't in Trash any more.")?;
    let mut dst = disk.root().join(clean(&meta.original_path)?);
    if dst.exists() {
        // Never overwrite: restore beside it.
        let stem = dst.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        let ext = dst.extension().map(|e| format!(".{}", e.to_string_lossy())).unwrap_or_default();
        dst = dst.with_file_name(format!("{stem} (restored){ext}"));
    }
    if let Some(p) = dst.parent() {
        std::fs::create_dir_all(p).map_err(|e| e.to_string())?;
    }
    std::fs::rename(dir.join("item"), &dst).map_err(|e| e.to_string())?;
    let _ = std::fs::remove_dir_all(&dir);
    Ok(dst)
}

/// Release Trash items older than the retention.
pub fn clean_trash(disk: &Disk, days: u64) {
    let cutoff = chrono::Utc::now().timestamp_millis() - (days as i64) * 86_400_000;
    for item in trash_list(disk) {
        if item.deleted_ms < cutoff {
            let _ = std::fs::remove_dir_all(disk.system("trash").join(&item.id));
        }
    }
}

fn kind_of(path: &Path) -> &'static str {
    let ext = path.extension().map(|e| e.to_string_lossy().to_ascii_lowercase()).unwrap_or_default();
    match ext.as_str() {
        "csv" | "tsv" => "table",
        "png" | "jpg" | "jpeg" | "gif" | "webp" => "image",
        _ => "text",
    }
}

/// A small render made on the computer, so a 2 GB CSV never crosses the wire to show 20 rows.
pub fn preview(path: &Path) -> Result<(FilePreview, Option<Vec<u8>>), String> {
    let meta = std::fs::metadata(path).map_err(|_| "That file wasn't found.".to_string())?;
    if meta.is_dir() {
        return Ok((FilePreview::Unsupported { reason: "Folders don't have a preview.".into() }, None));
    }
    match kind_of(path) {
        "table" => {
            let delim = if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("tsv")) { b'\t' } else { b',' };
            let mut rdr =
                csv::ReaderBuilder::new().delimiter(delim).flexible(true).from_path(path).map_err(|e| e.to_string())?;
            let columns: Vec<String> = rdr.headers().map_err(|e| e.to_string())?.iter().map(String::from).collect();
            let mut rows = Vec::new();
            let mut total = 0u64;
            for rec in rdr.records() {
                let Ok(rec) = rec else { continue };
                if rows.len() < MAX_PREVIEW_ROWS {
                    rows.push(rec.iter().map(String::from).collect());
                }
                total += 1;
            }
            Ok((FilePreview::Table { columns, rows, total_rows: total }, None))
        }
        "image" => {
            let img = image::open(path).map_err(|_| "This image couldn't be read.".to_string())?;
            let (ow, oh) = (img.width(), img.height());
            let small = if ow > MAX_IMAGE_EDGE || oh > MAX_IMAGE_EDGE {
                img.thumbnail(MAX_IMAGE_EDGE, MAX_IMAGE_EDGE)
            } else {
                img
            };
            let mut png = Vec::new();
            small.write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png).map_err(|e| e.to_string())?;
            Ok((
                FilePreview::Image {
                    width: small.width(),
                    height: small.height(),
                    original_width: ow,
                    original_height: oh,
                },
                Some(png),
            ))
        }
        _ => {
            let mut f = std::fs::File::open(path).map_err(|e| e.to_string())?;
            let mut buf = vec![0u8; MAX_TEXT_PREVIEW + 1];
            let n = std::io::Read::read(&mut f, &mut buf).map_err(|e| e.to_string())?;
            buf.truncate(n);
            let sniff = &buf[..n.min(8192)];
            if sniff.contains(&0) || std::str::from_utf8(sniff).is_err() && n < 8192 {
                return Ok((
                    FilePreview::Unsupported { reason: "There's no preview for this kind of file yet.".into() },
                    None,
                ));
            }
            let truncated = n > MAX_TEXT_PREVIEW;
            buf.truncate(MAX_TEXT_PREVIEW);
            Ok((FilePreview::Text { text: String::from_utf8_lossy(&buf).into_owned(), truncated }, None))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delete_and_restore_round_trip_without_overwriting() {
        let dir = tempfile::tempdir().unwrap();
        let disk = Disk::open(dir.path()).unwrap();
        std::fs::create_dir_all(disk.root().join("a")).unwrap();
        std::fs::write(disk.root().join("a/x.txt"), "one").unwrap();
        let t = to_trash(&disk, &disk.root().join("a/x.txt"), "user").unwrap();
        assert_eq!(t.original_path, "a/x.txt");
        assert!(!disk.root().join("a/x.txt").exists());
        assert_eq!(trash_list(&disk).len(), 1);

        std::fs::write(disk.root().join("a/x.txt"), "two").unwrap();
        let back = restore(&disk, &t.id).unwrap();
        assert_eq!(back.file_name().unwrap(), "x (restored).txt");
        assert_eq!(std::fs::read_to_string(back).unwrap(), "one");
        assert!(trash_list(&disk).is_empty());
    }

    #[test]
    fn csv_preview_sends_only_the_first_rows() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("big.csv");
        let mut body = String::from("id,name\n");
        for i in 0..500 {
            body.push_str(&format!("{i},row {i}\n"));
        }
        std::fs::write(&p, body).unwrap();
        let (FilePreview::Table { columns, rows, total_rows }, None) = preview(&p).unwrap() else {
            panic!("not a table")
        };
        assert_eq!(columns, vec!["id", "name"]);
        assert_eq!(rows.len(), MAX_PREVIEW_ROWS);
        assert_eq!(total_rows, 500);
    }

    #[test]
    fn folders_zip_and_unzip_to_the_same_files() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("jobs");
        std::fs::create_dir_all(src.join("logs")).unwrap();
        std::fs::write(src.join("a.csv"), "x,y\n1,2\n").unwrap();
        std::fs::write(src.join("logs/run.txt"), "ok").unwrap();
        let zip = dir.path().join("jobs.zip");
        zip_dir(&src, &zip).unwrap();
        let out = dir.path().join("copy");
        std::fs::create_dir_all(&out).unwrap();
        unzip(&zip, &out).unwrap();
        assert_eq!(std::fs::read_to_string(out.join("a.csv")).unwrap(), "x,y\n1,2\n");
        assert_eq!(std::fs::read_to_string(out.join("logs/run.txt")).unwrap(), "ok");
    }

    #[test]
    fn recent_lists_newest_files_first_and_counts_folder_items() {
        let dir = tempfile::tempdir().unwrap();
        let disk = Disk::open(dir.path()).unwrap();
        std::fs::create_dir_all(disk.root().join("a/b")).unwrap();
        std::fs::write(disk.root().join("a/old.txt"), "1").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(20));
        std::fs::write(disk.root().join("a/b/new.txt"), "2").unwrap();
        let r = recent(&disk.root(), 10);
        assert_eq!(r[0].file_name().unwrap(), "new.txt");
        assert_eq!(r.len(), 2);
        assert_eq!(entry(&disk, &disk.root().join("a")).unwrap().items, Some(2));
    }

    #[test]
    fn binary_files_have_no_text_preview() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("blob.bin");
        std::fs::write(&p, [0u8, 159, 146, 150, 0, 1]).unwrap();
        assert!(matches!(preview(&p).unwrap().0, FilePreview::Unsupported { .. }));
    }
}
