use std::collections::VecDeque;
use std::sync::{Mutex, OnceLock};

/// Thread-safe ring buffer for storing log lines in memory.
pub struct LogBuffer {
    inner: Mutex<VecDeque<String>>,
    max_lines: usize,
}

impl LogBuffer {
    pub fn new(max_lines: usize) -> Self {
        Self {
            inner: Mutex::new(VecDeque::with_capacity(max_lines)),
            max_lines,
        }
    }

    pub fn push(&self, line: String) {
        if let Ok(mut buf) = self.inner.lock() {
            buf.push_back(line);
            while buf.len() > self.max_lines {
                buf.pop_front();
            }
        }
    }

    pub fn get_recent(&self, count: usize) -> Vec<String> {
        match self.inner.lock() {
            Ok(buf) => {
                let skip = buf.len().saturating_sub(count);
                buf.iter().skip(skip).cloned().collect()
            }
            Err(_) => Vec::new(),
        }
    }

    pub fn len(&self) -> usize {
        self.inner.lock().map(|b| b.len()).unwrap_or(0)
    }
}

static GLOBAL_BUFFER: OnceLock<LogBuffer> = OnceLock::new();

pub fn global_buffer() -> &'static LogBuffer {
    GLOBAL_BUFFER.get_or_init(|| LogBuffer::new(500))
}

/// Read recent lines from syslog via `logread` (Entware/OpenWrt).
pub fn read_syslog(limit: usize) -> Vec<String> {
    let cmd = format!(
        "logread 2>/dev/null | grep -i trusttunnel | tail -n {}",
        limit
    );
    match std::process::Command::new("sh")
        .args(["-c", &cmd])
        .output()
    {
        Ok(output) => {
            let text = String::from_utf8_lossy(&output.stdout);
            text.lines()
                .filter(|l| !l.is_empty())
                .map(String::from)
                .collect()
        }
        Err(_) => Vec::new(),
    }
}

/// Collect recent logs: first from our in-memory buffer, then pad from syslog.
pub fn get_combined_logs(limit: usize) -> Vec<String> {
    let mut result = global_buffer().get_recent(limit);
    if result.len() < limit {
        let syslog_lines = read_syslog(limit - result.len());
        // Prepend syslog lines before buffer lines for chronological order
        let mut combined = syslog_lines;
        combined.append(&mut result);
        combined.truncate(limit);
        combined
    } else {
        result
    }
}
