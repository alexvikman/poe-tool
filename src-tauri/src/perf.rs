//! Where the app spends its time: a log of how long things take (opening a build,
//! loading data, Path of Building's answers, drawing the tree), one line each, in
//! perf.log in the app's data folder. It's kept small: past 1 MB it starts over.

use std::io::Write;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

const MAX_BYTES: u64 = 1024 * 1024;

static FILE: OnceLock<PathBuf> = OnceLock::new();
static WRITING: Mutex<()> = Mutex::new(());

/// Where the log goes; until it's set, lines are dropped.
pub fn init(dir: PathBuf) {
    let _ = FILE.set(dir.join("perf.log"));
}

/// Writes one line to the log.
pub fn note(line: &str) {
    let Some(path) = FILE.get() else { return };
    let _guard = WRITING.lock().unwrap();
    let start_over = std::fs::metadata(path).map(|m| m.len() > MAX_BYTES).unwrap_or(false);
    let file = std::fs::OpenOptions::new().create(true).append(!start_over).write(true).truncate(start_over).open(path);
    if let Ok(mut f) = file {
        let secs = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(0.0);
        let _ = writeln!(f, "{secs:.3} {line}");
    }
}

/// Times the parts of one thing: `mark` ends a part, and `done` writes them all on one
/// line, `label total=… part=… …` in milliseconds.
pub struct Span {
    label: String,
    started: Instant,
    last: Instant,
    parts: Vec<String>,
}

impl Span {
    pub fn new(label: impl Into<String>) -> Self {
        let now = Instant::now();
        Span { label: label.into(), started: now, last: now, parts: Vec::new() }
    }

    /// Ends the part since the last mark.
    pub fn mark(&mut self, part: &str) {
        let now = Instant::now();
        self.parts.push(format!("{part}={:.1}", (now - self.last).as_secs_f64() * 1000.0));
        self.last = now;
    }

    /// Adds something that isn't a time ("kb=300", "cached=true").
    pub fn note(&mut self, what: impl Into<String>) {
        self.parts.push(what.into());
    }

    pub fn done(self) {
        note(&format!("{} total={:.1} {}", self.label, self.started.elapsed().as_secs_f64() * 1000.0, self.parts.join(" ")));
    }
}
