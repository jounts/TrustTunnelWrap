use crate::config::LogSettings;
use log::{LevelFilter, Log, Metadata, Record};
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

pub fn init(settings: &LogSettings) -> Result<(), String> {
    let level = parse_level(&settings.level);
    let file_writer = if settings.file_enabled {
        Some(RotatingFile::new(
            PathBuf::from(settings.file_path.clone()),
            settings.rotate_size,
            settings.rotate_keep,
        )?)
    } else {
        None
    };

    let logger = TTLogger {
        level,
        file: Mutex::new(file_writer),
    };

    log::set_boxed_logger(Box::new(logger)).map_err(|e| format!("failed to set logger: {}", e))?;
    log::set_max_level(level);
    Ok(())
}

fn parse_level(value: &str) -> LevelFilter {
    match value.to_ascii_lowercase().as_str() {
        "off" => LevelFilter::Off,
        "error" => LevelFilter::Error,
        "warn" | "warning" => LevelFilter::Warn,
        "info" => LevelFilter::Info,
        "debug" => LevelFilter::Debug,
        "trace" => LevelFilter::Trace,
        _ => LevelFilter::Info,
    }
}

struct TTLogger {
    level: LevelFilter,
    file: Mutex<Option<RotatingFile>>,
}

impl TTLogger {
    fn format_line(record: &Record<'_>) -> String {
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        format!("[{}] {} {}", ts, record.level(), record.args())
    }
}

impl Log for TTLogger {
    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        metadata.level() <= self.level
    }

    fn log(&self, record: &Record<'_>) {
        if !self.enabled(record.metadata()) {
            return;
        }

        let line = Self::format_line(record);
        eprintln!("{}", line);

        if let Ok(mut guard) = self.file.lock() {
            if let Some(file) = guard.as_mut() {
                let _ = file.write_line(&line);
            }
        }
    }

    fn flush(&self) {}
}

struct RotatingFile {
    path: PathBuf,
    rotate_size: u64,
    rotate_keep: usize,
    file: File,
}

impl RotatingFile {
    fn new(path: PathBuf, rotate_size: u64, rotate_keep: usize) -> Result<Self, String> {
        let parent = path
            .parent()
            .ok_or_else(|| format!("invalid log file path: {}", path.display()))?;
        fs::create_dir_all(parent)
            .map_err(|e| format!("failed to create log dir {}: {}", parent.display(), e))?;
        let file = open_append(&path)?;
        Ok(Self {
            path,
            rotate_size: rotate_size.max(1024),
            rotate_keep,
            file,
        })
    }

    fn write_line(&mut self, line: &str) -> Result<(), String> {
        let line_bytes = line.as_bytes();
        let required = line_bytes.len() as u64 + 1;

        let current_size = self.file.metadata().map(|m| m.len()).unwrap_or(0);
        if self.rotate_keep > 0 && current_size.saturating_add(required) > self.rotate_size {
            self.rotate()?;
        }

        self.file
            .write_all(line_bytes)
            .and_then(|_| self.file.write_all(b"\n"))
            .and_then(|_| self.file.flush())
            .map_err(|e| format!("failed to write log file {}: {}", self.path.display(), e))
    }

    fn rotate(&mut self) -> Result<(), String> {
        if self.rotate_keep == 0 {
            self.file = open_truncate(&self.path)?;
            return Ok(());
        }

        // Delete the oldest archive if it exists.
        let oldest = format!("{}.{}", self.path.display(), self.rotate_keep);
        if Path::new(&oldest).exists() {
            let _ = fs::remove_file(&oldest);
        }

        // Shift N-1 ... 1 to N ... 2
        for idx in (1..self.rotate_keep).rev() {
            let src = format!("{}.{}", self.path.display(), idx);
            let dst = format!("{}.{}", self.path.display(), idx + 1);
            if Path::new(&src).exists() {
                let _ = fs::rename(&src, &dst);
            }
        }

        // Move current file to .1
        if self.path.exists() {
            let first = format!("{}.1", self.path.display());
            let _ = fs::rename(&self.path, first);
        }
        self.file = open_truncate(&self.path)?;
        Ok(())
    }
}

fn open_append(path: &Path) -> Result<File, String> {
    OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|e| format!("failed to open log file {}: {}", path.display(), e))
}

fn open_truncate(path: &Path) -> Result<File, String> {
    OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(path)
        .map_err(|e| format!("failed to truncate log file {}: {}", path.display(), e))
}
