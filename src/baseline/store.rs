//! Persistent store for named + rolling baselines.
//!
//! Named baselines: `<dir>/<name>.json` — single JSON object capturing the
//! signal id → value map at record time.
//! Rolling baselines: `<dir>/_rolling.jsonl` — append-only JSONL, one record
//! per line. Concurrent writes are serialised via `rustix::fs::flock` (SDD
//! §116). The file is pruned to `window_n` records on every append.

use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::signal::{Signal, SignalValue};

const ROLLING_FILENAME: &str = "_rolling.jsonl";

#[derive(Debug, Error)]
pub enum Error {
    #[error("baseline directory I/O at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("baseline JSON: {source}")]
    Json {
        #[from]
        source: serde_json::Error,
    },
    #[error("could not acquire exclusive lock on {path}: {source}")]
    Flock {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

/// One baseline record — captured signals and the time they were captured.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BaselineRecord {
    pub captured_at: DateTime<Local>,
    pub signals: HashMap<String, f64>,
}

impl BaselineRecord {
    pub fn from_signals(signals: &[Signal]) -> Self {
        let mut map = HashMap::new();
        for s in signals {
            if let Some(v) = signal_to_f64(&s.value) {
                map.insert(s.id.clone(), v);
            }
        }
        BaselineRecord {
            captured_at: Local::now(),
            signals: map,
        }
    }
}

fn signal_to_f64(v: &SignalValue) -> Option<f64> {
    match v {
        SignalValue::F64(x) => Some(*x),
        SignalValue::I64(x) => Some(*x as f64),
        _ => None,
    }
}

/// Persistent baseline store rooted at a directory. Use `at()` for tests
/// (explicit path) or `xdg()` for the default `${XDG_DATA_HOME}/usereport/baselines`.
#[derive(Debug, Clone)]
pub struct BaselineStore {
    dir: PathBuf,
}

impl BaselineStore {
    pub fn at(dir: PathBuf) -> Self {
        BaselineStore { dir }
    }

    /// Default location per SDD §114: `${XDG_DATA_HOME:-~/.local/share}/usereport/baselines`.
    pub fn xdg() -> Result<Self> {
        let base = std::env::var("XDG_DATA_HOME")
            .ok()
            .filter(|s| !s.is_empty())
            .map(PathBuf::from)
            .or_else(|| {
                std::env::var("HOME").ok().map(|home| {
                    let mut p = PathBuf::from(home);
                    p.push(".local/share");
                    p
                })
            });
        let mut dir = base.ok_or_else(|| Error::Io {
            path: PathBuf::new(),
            source: std::io::Error::new(std::io::ErrorKind::NotFound, "neither XDG_DATA_HOME nor HOME is set"),
        })?;
        dir.push("usereport/baselines");
        Ok(BaselineStore::at(dir))
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    fn ensure_dir(&self) -> Result<()> {
        std::fs::create_dir_all(&self.dir).map_err(|e| Error::Io {
            path: self.dir.clone(),
            source: e,
        })
    }

    fn named_path(&self, name: &str) -> PathBuf {
        self.dir.join(format!("{}.json", name))
    }

    pub fn record(&self, name: &str, signals: &[Signal]) -> Result<()> {
        self.ensure_dir()?;
        let record = BaselineRecord::from_signals(signals);
        let json = serde_json::to_vec_pretty(&record)?;
        let path = self.named_path(name);
        std::fs::write(&path, json).map_err(|e| Error::Io {
            path: path.clone(),
            source: e,
        })?;
        Ok(())
    }

    pub fn load(&self, name: &str) -> Result<Option<BaselineRecord>> {
        let path = self.named_path(name);
        match std::fs::read(&path) {
            Ok(bytes) => {
                let r: BaselineRecord = serde_json::from_slice(&bytes)?;
                Ok(Some(r))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(Error::Io { path, source: e }),
        }
    }

    pub fn list(&self) -> Result<Vec<String>> {
        let entries = match std::fs::read_dir(&self.dir) {
            Ok(e) => e,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => {
                return Err(Error::Io {
                    path: self.dir.clone(),
                    source: e,
                });
            }
        };
        let mut names: Vec<String> = entries
            .filter_map(|e| e.ok())
            .filter_map(|e| {
                let p = e.path();
                let file_name = p.file_name()?.to_str()?.to_string();
                if file_name == ROLLING_FILENAME {
                    return None;
                }
                p.file_stem()
                    .and_then(|s| s.to_str())
                    .filter(|_| p.extension().and_then(|x| x.to_str()) == Some("json"))
                    .map(|s| s.to_string())
            })
            .collect();
        names.sort();
        Ok(names)
    }

    pub fn delete(&self, name: &str) -> Result<()> {
        let path = self.named_path(name);
        match std::fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Err(Error::Io { path, source: e }),
            Err(e) => Err(Error::Io { path, source: e }),
        }
    }

    /// Append a new rolling record and prune to the most recent `window_n`
    /// records. Concurrent writers are serialised via `flock(LOCK_EX)`.
    pub fn append_rolling(&self, signals: &[Signal], window_n: usize) -> Result<()> {
        self.ensure_dir()?;
        let path = self.dir.join(ROLLING_FILENAME);
        let file = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .truncate(false)
            .open(&path)
            .map_err(|e| Error::Io {
                path: path.clone(),
                source: e,
            })?;
        let _guard = ExclusiveLock::acquire(&file, &path)?;

        // Read existing valid records using the same (locked) file descriptor.
        let mut existing: Vec<BaselineRecord> = Vec::new();
        let reader = BufReader::new(&file);
        for line in reader.lines() {
            match line {
                Ok(l) if l.trim().is_empty() => continue,
                Ok(l) => match serde_json::from_str::<BaselineRecord>(&l) {
                    Ok(r) => existing.push(r),
                    Err(_) => log::debug!("skipping malformed rolling JSONL line: {}", l),
                },
                Err(_) => break,
            }
        }
        existing.push(BaselineRecord::from_signals(signals));
        // Prune oldest if over window_n.
        let start = existing.len().saturating_sub(window_n);
        let kept = &existing[start..];

        // Rewrite using the same file descriptor: truncate then write from the
        // start. This keeps the exclusive lock held across the full read+write.
        file.set_len(0).map_err(|e| Error::Io {
            path: path.clone(),
            source: e,
        })?;
        (&file).seek(SeekFrom::Start(0)).map_err(|e| Error::Io {
            path: path.clone(),
            source: e,
        })?;
        let mut writer = std::io::BufWriter::new(&file);
        for r in kept {
            let line = serde_json::to_string(r)?;
            writeln!(writer, "{}", line).map_err(|e| Error::Io {
                path: path.clone(),
                source: e,
            })?;
        }
        Ok(())
    }

    /// Return all rolling records in chronological order (oldest first).
    pub fn load_rolling(&self) -> Result<Vec<BaselineRecord>> {
        let path = self.dir.join(ROLLING_FILENAME);
        let file = match File::open(&path) {
            Ok(f) => f,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(Error::Io { path, source: e }),
        };
        let reader = BufReader::new(file);
        let mut out = Vec::new();
        for line in reader.lines() {
            let line = line.map_err(|e| Error::Io {
                path: path.clone(),
                source: e,
            })?;
            if line.trim().is_empty() {
                continue;
            }
            match serde_json::from_str::<BaselineRecord>(&line) {
                Ok(r) => out.push(r),
                Err(_) => log::debug!("skipping malformed rolling JSONL line: {}", line),
            }
        }
        Ok(out)
    }
}

/// RAII handle around `flock(LOCK_EX)` — releases on drop. Implemented
/// manually because `rustix::fs::flock` operates on a borrowed file
/// descriptor and we need the lock to persist for the read+write sequence.
struct ExclusiveLock<'a> {
    file: &'a File,
}

impl<'a> ExclusiveLock<'a> {
    fn acquire(file: &'a File, path: &Path) -> Result<Self> {
        // Reset cursor before reading; the lock is held independently of cursor pos.
        let _ = (&*file).seek(SeekFrom::Start(0));
        rustix::fs::flock(file, rustix::fs::FlockOperation::LockExclusive).map_err(|e| Error::Flock {
            path: path.to_path_buf(),
            source: std::io::Error::from_raw_os_error(e.raw_os_error()),
        })?;
        Ok(ExclusiveLock { file })
    }
}

impl Drop for ExclusiveLock<'_> {
    fn drop(&mut self) {
        let _ = rustix::fs::flock(self.file, rustix::fs::FlockOperation::Unlock);
    }
}
