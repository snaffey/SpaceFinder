use crate::{human_size, platform, recommendations, scanner};
use std::{path::PathBuf, sync::atomic::AtomicBool};

pub fn run(args: &[std::ffi::OsString]) -> Result<(), String> {
    let path = args
        .get(1)
        .ok_or("Missing folder. Example: spacefinder-cli --scan C:\\Users --json")?;
    let mut workers = scanner::default_workers();
    let mut json = false;
    let mut recommend = false;
    let mut index = 2;
    while index < args.len() {
        match args[index].to_str() {
            Some("--json") => json = true,
            Some("--recommendations") => recommend = true,
            Some("--threads") => {
                index += 1;
                workers = args
                    .get(index)
                    .and_then(|v| v.to_str())
                    .and_then(|v| v.parse().ok())
                    .filter(|v| (1..=64).contains(v))
                    .ok_or("--threads must be between 1 and 64")?;
            }
            _ => {
                return Err(format!("Unknown option: {}", args[index].to_string_lossy()));
            }
        }
        index += 1;
    }
    let result = scanner::scan(
        &PathBuf::from(path),
        workers,
        &AtomicBool::new(false),
        |_| {},
    )?;
    let report = recommend.then(|| {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |v| v.as_secs() as i64);
        recommendations::analyze(&result, now)
    });
    if json {
        let top: Vec<_> = result.sorted_children(0).into_iter().take(30).map(|i| {
            let n = &result.nodes[i];
            serde_json::json!({"path": platform::display_path(&result.path(i)), "bytes": n.bytes, "files": n.files, "folders": n.folders, "directory": n.directory, "details": n.details, "incomplete": n.incomplete})
        }).collect();
        let recommendations = report.as_ref().map(|report| {
            let items: Vec<_> = report.items.iter().map(|item| {
                let mut value = serde_json::to_value(item).expect("recommendation fields are serializable");
                value["path"] = serde_json::Value::String(platform::display_path(&result.path(item.node)));
                value
            }).collect();
            serde_json::json!({"items": items, "regenerable_bytes": report.regenerable_bytes, "review_bytes": report.review_bytes, "inspected_files": report.inspected_files, "incomplete": report.incomplete, "truncated": report.truncated})
        });
        let value = serde_json::json!({"root": platform::display_path(&result.root), "workers": workers, "progress": result.progress, "cancelled": result.cancelled, "issues": result.issues, "top": top, "extensions": result.extensions, "recommendations": recommendations});
        println!(
            "{}",
            serde_json::to_string_pretty(&value).map_err(|e| e.to_string())?
        );
    } else {
        println!(
            "{}\n{} | {} files | {} folders | {:.3}s | {} workers\n{} errors | {} links/reparse points skipped\n",
            platform::display_path(&result.root),
            human_size(result.progress.bytes),
            result.progress.files,
            result.progress.folders,
            result.progress.seconds,
            workers,
            result.progress.errors,
            result.progress.skipped
        );
        for index in result.sorted_children(0).into_iter().take(30) {
            let node = &result.nodes[index];
            println!(
                "{:>14}  {}{}",
                human_size(node.bytes),
                node.name.to_string_lossy(),
                if node.directory { "\\" } else { "" }
            );
        }
        if let Some(report) = &report {
            println!(
                "\nRecommendations: {} · logical sizes to review, no automatic cleanup",
                report.items.len()
            );
            for item in &report.items {
                println!(
                    "\n{}  {}\n{}\n{}\nBefore removing: {}",
                    human_size(item.bytes),
                    item.title,
                    platform::display_path(&result.path(item.node)),
                    item.reason,
                    item.caution
                );
            }
        }
    }
    Ok(())
}
