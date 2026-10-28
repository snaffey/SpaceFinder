use crate::platform;
use crossbeam_channel::{Receiver, Sender, bounded, unbounded};
use serde::Serialize;
use std::{
    collections::HashMap,
    ffi::OsString,
    io::{self, Write},
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

#[derive(Debug)]
pub struct Node {
    pub name: OsString,
    pub parent: Option<usize>,
    pub children: Vec<usize>,
    pub directory: bool,
    pub bytes: u64,
    pub files: u64,
    pub folders: u64,
    pub details: platform::FileDetails,
    pub incomplete: bool,
}

#[derive(Debug, Default, Clone, Serialize)]
pub struct Progress {
    pub files: u64,
    pub folders: u64,
    pub bytes: u64,
    pub errors: u64,
    pub denied: u64,
    pub skipped: u64,
    pub seconds: f64,
    pub current: String,
}

#[derive(Debug)]
pub enum Event {
    Progress(Progress),
    Finished(Result<Scan, String>),
}

#[derive(Debug, Default, Serialize)]
pub struct Extension {
    pub bytes: u64,
    pub files: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum IssueKind {
    AccessDenied,
    NotFound,
    Other,
}

#[derive(Debug, Serialize)]
pub struct ScanIssue {
    pub path: PathBuf,
    pub message: String,
    pub kind: IssueKind,
}

impl ScanIssue {
    fn from_error(path: PathBuf, error: io::Error) -> Self {
        let kind = match error.kind() {
            io::ErrorKind::PermissionDenied => IssueKind::AccessDenied,
            io::ErrorKind::NotFound => IssueKind::NotFound,
            _ => IssueKind::Other,
        };
        Self {
            path,
            message: error.to_string(),
            kind,
        }
    }
}

#[derive(Debug)]
pub struct Scan {
    pub root: PathBuf,
    pub nodes: Vec<Node>,
    pub progress: Progress,
    pub cancelled: bool,
    pub issues: Vec<ScanIssue>,
    pub extensions: HashMap<String, Extension>,
    pub workers: usize,
}

impl Scan {
    pub fn path(&self, mut index: usize) -> PathBuf {
        let mut parts = Vec::new();
        while let Some(parent) = self.nodes[index].parent {
            parts.push(&self.nodes[index].name);
            index = parent;
        }
        let mut path = self.root.clone();
        for part in parts.into_iter().rev() {
            path.push(part);
        }
        path
    }

    pub fn sorted_children(&self, index: usize) -> Vec<usize> {
        let mut children = self.nodes[index].children.clone();
        children.sort_unstable_by(|a, b| {
            self.nodes[*b]
                .bytes
                .cmp(&self.nodes[*a].bytes)
                .then_with(|| self.nodes[*a].name.cmp(&self.nodes[*b].name))
        });
        children
    }

    pub fn write_csv(&self, writer: &mut impl Write) -> io::Result<()> {
        writer.write_all(b"\xEF\xBB\xBFpath,type,bytes,files,folders,created_unix,modified_unix,accessed_unix,attributes,incomplete\r\n")?;
        for (index, node) in self.nodes.iter().enumerate() {
            let path = platform::display_path(&self.path(index)).replace('"', "\"\"");
            writeln!(
                writer,
                "\"{path}\",{},{},{},{},{},{},{},{},{}\r",
                if node.directory { "directory" } else { "file" },
                node.bytes,
                node.files,
                node.folders,
                node.details
                    .created
                    .map(|t| t.to_string())
                    .unwrap_or_default(),
                node.details
                    .modified
                    .map(|t| t.to_string())
                    .unwrap_or_default(),
                node.details
                    .accessed
                    .map(|t| t.to_string())
                    .unwrap_or_default(),
                node.details.attributes,
                node.incomplete
            )?;
        }
        Ok(())
    }
}

pub fn default_workers() -> usize {
    std::thread::available_parallelism()
        .map_or(4, usize::from)
        .clamp(2, 16)
}

struct Job {
    parent: usize,
    path: PathBuf,
}
struct Batch {
    parent: usize,
    entries: Vec<platform::Entry>,
    error: Option<ScanIssue>,
    done: bool,
}

fn worker(jobs: Receiver<Job>, results: Sender<Batch>, cancel: &AtomicBool) {
    for job in jobs {
        if cancel.load(Ordering::Relaxed) {
            break;
        }
        let mut entries = Vec::with_capacity(1024);
        let mut disconnected = false;
        let result = platform::read_directory(&job.path, cancel, |entry| {
            if disconnected {
                return;
            }
            entries.push(entry);
            if entries.len() == 1024 {
                disconnected = results
                    .send(Batch {
                        parent: job.parent,
                        entries: std::mem::replace(&mut entries, Vec::with_capacity(1024)),
                        error: None,
                        done: false,
                    })
                    .is_err();
            }
        });
        let error = result
            .err()
            .map(|e| ScanIssue::from_error(job.path.clone(), e));
        if disconnected
            || results
                .send(Batch {
                    parent: job.parent,
                    entries,
                    error,
                    done: true,
                })
                .is_err()
        {
            break;
        }
    }
}

/// Enumerate metadata only; never read file contents. Child indexes always exceed
/// parent indexes, so totals can be aggregated in one reverse O(n) pass.
pub fn scan(
    root: &Path,
    workers: usize,
    cancel: &AtomicBool,
    mut progress: impl FnMut(Progress),
) -> Result<Scan, String> {
    let started = Instant::now();
    let root = root
        .canonicalize()
        .map_err(|e| format!("Não foi possível abrir a pasta: {e}"))?;
    let root_metadata = root.metadata().map_err(|e| e.to_string())?;
    if !root_metadata.is_dir() {
        return Err("Escolhe uma pasta ou uma unidade de disco.".into());
    }
    let workers = workers.clamp(1, 64);
    let mut scan = Scan {
        nodes: vec![Node {
            name: root.as_os_str().into(),
            parent: None,
            children: Vec::new(),
            directory: true,
            bytes: 0,
            files: 0,
            folders: 0,
            details: platform::metadata_details(&root_metadata),
            incomplete: true,
        }],
        root,
        progress: Progress::default(),
        cancelled: false,
        issues: Vec::new(),
        extensions: HashMap::new(),
        workers,
    };
    let (job_tx, job_rx) = unbounded();
    let (result_tx, result_rx) = bounded(workers * 2);
    job_tx
        .send(Job {
            parent: 0,
            path: scan.root.clone(),
        })
        .map_err(|e| e.to_string())?;
    std::thread::scope(|scope| {
        for _ in 0..workers {
            let (jobs, results) = (job_rx.clone(), result_tx.clone());
            scope.spawn(move || worker(jobs, results, cancel));
        }
        drop(job_rx);
        drop(result_tx);
        let mut pending = 1_u64;
        let mut last_update = Instant::now();
        while pending > 0 && !cancel.load(Ordering::Relaxed) {
            let batch = match result_rx.recv_timeout(Duration::from_millis(80)) {
                Ok(batch) => batch,
                Err(crossbeam_channel::RecvTimeoutError::Timeout) => continue,
                Err(_) => break,
            };
            if batch.done {
                pending -= 1;
                scan.nodes[batch.parent].incomplete = batch.error.is_some();
            }
            if let Some(error) = batch.error {
                scan.progress.errors += 1;
                if error.kind == IssueKind::AccessDenied {
                    scan.progress.denied += 1;
                }
                if scan.issues.len() < 100 {
                    scan.issues.push(error);
                }
            }
            let parent_path = scan.path(batch.parent);
            scan.progress.current = platform::display_path(&parent_path);
            for entry in batch.entries {
                if entry.reparse {
                    scan.progress.skipped += 1;
                    continue;
                }
                let index = scan.nodes.len();
                if entry.directory {
                    scan.progress.folders += 1;
                    let path = parent_path.join(&entry.name);
                    if job_tx
                        .send(Job {
                            parent: index,
                            path,
                        })
                        .is_ok()
                    {
                        pending += 1;
                    }
                } else {
                    scan.progress.files += 1;
                    scan.progress.bytes += entry.size;
                    let extension = Path::new(&entry.name)
                        .extension()
                        .map(|e| e.to_string_lossy().to_lowercase())
                        .unwrap_or_else(|| "sem extensão".into());
                    let summary = scan.extensions.entry(extension).or_default();
                    summary.bytes += entry.size;
                    summary.files += 1;
                }
                scan.nodes.push(Node {
                    name: entry.name,
                    parent: Some(batch.parent),
                    children: Vec::new(),
                    directory: entry.directory,
                    bytes: if entry.directory { 0 } else { entry.size },
                    files: u64::from(!entry.directory),
                    folders: 0,
                    details: entry.details,
                    incomplete: entry.directory,
                });
                scan.nodes[batch.parent].children.push(index);
            }
            if last_update.elapsed() >= Duration::from_millis(100) {
                scan.progress.seconds = started.elapsed().as_secs_f64();
                progress(scan.progress.clone());
                last_update = Instant::now();
            }
        }
        // Disconnect both channels before joining: a worker may be blocked sending
        // a full batch when cancellation arrives.
        drop(job_tx);
        drop(result_rx);
    });
    scan.cancelled = cancel.load(Ordering::Relaxed);
    for index in (1..scan.nodes.len()).rev() {
        let node = &scan.nodes[index];
        let (parent, bytes, files, folders, modified, accessed, incomplete) = (
            node.parent.unwrap(),
            node.bytes,
            node.files,
            node.folders + u64::from(node.directory),
            node.details.modified,
            node.details.accessed,
            node.incomplete,
        );
        let parent = &mut scan.nodes[parent];
        parent.bytes += bytes;
        parent.files += files;
        parent.folders += folders;
        parent.details.modified = parent.details.modified.max(modified);
        parent.details.accessed = parent.details.accessed.max(accessed);
        parent.incomplete |= incomplete;
    }
    scan.progress.seconds = started.elapsed().as_secs_f64();
    progress(scan.progress.clone());
    Ok(scan)
}

pub fn start(root: PathBuf, workers: usize) -> (Receiver<Event>, Arc<AtomicBool>) {
    let (tx, rx) = unbounded();
    let cancel = Arc::new(AtomicBool::new(false));
    let token = cancel.clone();
    std::thread::spawn(move || {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            scan(&root, workers, &token, |value| {
                let _ = tx.send(Event::Progress(value));
            })
        }))
        .unwrap_or_else(|_| {
            Err("O worker terminou inesperadamente. Tenta analisar novamente.".into())
        });
        let _ = tx.send(Event::Finished(result));
    });
    (rx, cancel)
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            let path = std::env::temp_dir().join(format!(
                "spacefinder-test-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn nested_totals_hidden_unicode_and_empty_directories() {
        let f = Fixture::new();
        std::fs::create_dir_all(f.0.join("pasta/empty")).unwrap();
        std::fs::write(f.0.join("pasta/ação.bin"), [0; 3073]).unwrap();
        std::fs::write(f.0.join(".hidden"), [0; 7]).unwrap();
        let s = scan(&f.0, 4, &AtomicBool::new(false), |_| {}).unwrap();
        assert_eq!(
            (s.nodes[0].bytes, s.nodes[0].files, s.nodes[0].folders),
            (3080, 2, 2)
        );
        assert_eq!(s.progress.errors, 0);
        assert!(s.nodes.iter().all(|n| !n.incomplete));
        assert_eq!(s.extensions["bin"].bytes, 3073);
        let pasta = s.nodes.iter().position(|n| n.name == "pasta").unwrap();
        assert_eq!(s.nodes[pasta].bytes, 3073);
        assert_eq!(s.path(pasta), f.0.canonicalize().unwrap().join("pasta"));
    }

    #[test]
    fn native_metadata_and_directory_dates_include_descendants() {
        let f = Fixture::new();
        let path = f.0.join("timestamp.bin");
        std::fs::write(&path, [0; 21]).unwrap();
        let future = std::time::SystemTime::now() + Duration::from_secs(3600);
        std::fs::OpenOptions::new()
            .write(true)
            .open(&path)
            .unwrap()
            .set_times(std::fs::FileTimes::new().set_modified(future))
            .unwrap();
        let expected = platform::metadata_details(&path.metadata().unwrap());
        let s = scan(&f.0, 4, &AtomicBool::new(false), |_| {}).unwrap();
        let file = s.nodes.iter().find(|n| !n.directory).unwrap();
        assert_eq!(file.details.modified, expected.modified);
        assert_eq!(file.details.created, expected.created);
        assert_eq!(file.details.attributes, expected.attributes);
        assert_eq!(s.nodes[0].details.modified, expected.modified);
    }

    #[test]
    fn empty_invalid_and_file_roots() {
        let f = Fixture::new();
        let s = scan(&f.0, 1, &AtomicBool::new(false), |_| {}).unwrap();
        assert_eq!(s.nodes.len(), 1);
        assert!(scan(&f.0.join("missing"), 1, &AtomicBool::new(false), |_| {}).is_err());
        std::fs::write(f.0.join("file"), []).unwrap();
        assert!(scan(&f.0.join("file"), 1, &AtomicBool::new(false), |_| {}).is_err());
    }

    #[test]
    fn cancellation_joins_workers_and_preserves_partial_totals() {
        let f = Fixture::new();
        for i in 0..256 {
            std::fs::create_dir(f.0.join(i.to_string())).unwrap();
        }
        let token = AtomicBool::new(true);
        let s = scan(&f.0, 8, &token, |_| {}).unwrap();
        assert!(s.cancelled);
        assert!(s.nodes[0].incomplete);
        assert_eq!(s.nodes[0].bytes, s.progress.bytes);
    }

    #[test]
    fn large_directory_batches_and_csv_quoting() {
        let f = Fixture::new();
        for i in 0..2200 {
            std::fs::write(f.0.join(format!("file,{i}.txt")), [0; 3]).unwrap();
        }
        let s = scan(&f.0, 4, &AtomicBool::new(false), |_| {}).unwrap();
        assert_eq!(s.nodes[0].bytes, 6600);
        assert_eq!(s.nodes[0].files, 2200);
        let mut csv = Vec::new();
        s.write_csv(&mut csv).unwrap();
        let csv = String::from_utf8(csv).unwrap();
        assert!(csv.contains("file,0.txt\",file,3,1,0"));
        assert_eq!(csv.lines().count(), 2202);
    }

    #[cfg(windows)]
    #[test]
    fn classifies_windows_permission_errors() {
        let issue = ScanIssue::from_error(
            PathBuf::from(r"C:\protected"),
            io::Error::from_raw_os_error(5),
        );
        assert_eq!(issue.kind, IssueKind::AccessDenied);
    }

    #[cfg(windows)]
    #[test]
    fn supports_extended_length_paths() {
        let f = Fixture::new();
        let mut deep = f.0.canonicalize().unwrap();
        for _ in 0..16 {
            deep.push("long-directory-name");
        }
        std::fs::create_dir_all(&deep).unwrap();
        std::fs::write(deep.join("ficheiro.txt"), [0; 17]).unwrap();
        let s = scan(&f.0, 4, &AtomicBool::new(false), |_| {}).unwrap();
        assert_eq!((s.nodes[0].bytes, s.nodes[0].files), (17, 1));
        assert_eq!(s.progress.errors, 0);
    }

    #[test]
    fn cancellation_unblocks_backpressured_worker() {
        let f = Fixture::new();
        for i in 0..2100 {
            std::fs::write(f.0.join(i.to_string()), []).unwrap();
        }
        let (job_tx, job_rx) = unbounded();
        let (result_tx, result_rx) = bounded(1);
        let cancel = AtomicBool::new(false);
        std::thread::scope(|scope| {
            job_tx
                .send(Job {
                    parent: 0,
                    path: f.0.clone(),
                })
                .unwrap();
            let handle = scope.spawn(|| worker(job_rx, result_tx, &cancel));
            // Once the first batch arrives, the worker will fill this bounded
            // channel again. Cancellation must also disconnect it before joining.
            let batch = result_rx.recv_timeout(Duration::from_secs(5)).unwrap();
            assert_eq!(batch.entries.len(), 1024);
            cancel.store(true, Ordering::Relaxed);
            drop(result_rx);
            drop(job_tx);
            handle.join().unwrap();
        });
    }

    #[cfg(windows)]
    #[test]
    fn skips_junction_cycles() {
        use std::os::windows::process::CommandExt;
        let f = Fixture::new();
        std::fs::write(f.0.join("data"), [0; 12]).unwrap();
        let status = std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(f.0.join("cycle"))
            .arg(&f.0)
            .creation_flags(0x08000000)
            .output()
            .unwrap();
        assert!(status.status.success());
        let s = scan(&f.0, 4, &AtomicBool::new(false), |_| {}).unwrap();
        assert_eq!(s.progress.skipped, 1);
        assert_eq!(s.nodes[0].bytes, 12);
        std::fs::remove_dir(f.0.join("cycle")).unwrap();
    }
}
