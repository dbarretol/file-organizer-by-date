use std::fs::{File, OpenOptions};
use std::io::{self, BufWriter, Write};
use std::path::Path;

use chrono::Local;

/// Optional record (`--log`) with the same lines as the console, each one
/// timestamped. Opened in append mode, so several runs can share a file.
pub struct EventLog(Option<BufWriter<File>>);

impl EventLog {
    pub fn open(path: Option<&Path>) -> io::Result<Self> {
        let Some(path) = path else {
            return Ok(EventLog(None));
        };
        let file = OpenOptions::new().create(true).append(true).open(path)?;
        Ok(EventLog(Some(BufWriter::new(file))))
    }

    /// Writes one line. The log is secondary: a write failure must not
    /// interrupt a half-done batch of moves, so it's ignored.
    pub fn line(&mut self, text: &str) {
        if let Some(out) = &mut self.0 {
            let now = Local::now().format("%Y-%m-%d %H:%M:%S");
            let _ = writeln!(out, "[{now}] {}", text.trim_start());
        }
    }
}

impl Drop for EventLog {
    fn drop(&mut self) {
        if let Some(out) = &mut self.0 {
            let _ = out.flush();
        }
    }
}
