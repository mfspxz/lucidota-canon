//! CANON stores: one hash-chained append-only JSONL per record kind.
//! Same discipline as PL: strict parsing (corruption is fatal, never skipped),
//! exclusive file lock on append, fsync before reporting success.
//! Kinds: constitution, contracts, decisions, supersessions.
use crate::cjson;
use crate::time;
use serde_json::{Map, Value};
use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;

pub const SCHEMA: i64 = 1;
pub const GENESIS: &str = "GENESIS";
pub const PROTOCOL: i64 = 1;

/// Record kinds and their files, in canonical order.
pub const KINDS: [(&str, &str); 4] = [
    ("constitution", "constitution.jsonl"),
    ("contracts", "contracts.jsonl"),
    ("decisions", "decisions.jsonl"),
    ("supersessions", "supersessions.jsonl"),
];

#[derive(Debug)]
pub enum StoreError {
    Io(String),
    Corrupt { file: String, line: usize, msg: String },
    Chain { file: String, line: usize, msg: String },
    Semantic(String),
}

impl std::fmt::Display for StoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StoreError::Io(m) => write!(f, "io: {}", m),
            StoreError::Corrupt { file, line, msg } => {
                write!(f, "AUDIT FAIL {} line {}: {}", file, line, msg)
            }
            StoreError::Chain { file, line, msg } => {
                write!(f, "AUDIT FAIL {} chain line {}: {}", file, line, msg)
            }
            StoreError::Semantic(m) => write!(f, "semantic: {}", m),
        }
    }
}

pub fn canon_dir() -> PathBuf {
    std::env::var("CANON_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/home/hatch/workspace/canon"))
}

fn file_for(kind: &str) -> Result<PathBuf, StoreError> {
    KINDS
        .iter()
        .find(|(k, _)| *k == kind)
        .map(|(_, f)| canon_dir().join(f))
        .ok_or_else(|| StoreError::Semantic(format!("unknown record kind {:?}", kind)))
}

/// Strict read: every line must be a JSON object with schema:1.
/// Missing file = empty store (not corruption). Malformed line = fatal.
pub fn read_strict(kind: &str) -> Result<Vec<Map<String, Value>>, StoreError> {
    let path = file_for(kind)?;
    let fname = path.file_name().unwrap().to_string_lossy().into_owned();
    let text = match fs::read_to_string(&path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]),
        Err(e) => return Err(StoreError::Io(e.to_string())),
    };
    let mut out = Vec::new();
    for (i, line) in text.lines().enumerate() {
        let ln = i + 1;
        let t = line.trim();
        if t.is_empty() {
            continue;
        }
        let v: Value = serde_json::from_str(t).map_err(|e| StoreError::Corrupt {
            file: fname.clone(),
            line: ln,
            msg: format!("invalid JSON: {}", e),
        })?;
        let m = v.as_object().ok_or_else(|| StoreError::Corrupt {
            file: fname.clone(),
            line: ln,
            msg: "top-level must be an object".into(),
        })?.clone();
        match m.get("schema").and_then(|v| v.as_i64()) {
            Some(1) => {}
            other => {
                return Err(StoreError::Corrupt {
                    file: fname.clone(),
                    line: ln,
                    msg: format!("unsupported schema: {:?}", other),
                })
            }
        }
        out.push(m);
    }
    Ok(out)
}

/// Append with exclusive lock, chain link, fsync. All-or-nothing.
/// Rejects duplicate ids within the kind (C2: no silent mutation).
pub fn append(kind: &str, mut e: Map<String, Value>) -> Result<(), StoreError> {
    use fs2::FileExt;
    let path = file_for(kind)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| StoreError::Io(e.to_string()))?;
    }
    // duplicate-id guard (read before lock; re-checked under lock below is
    // overkill for single-writer canon — lock serializes writers anyway)
    if let Some(id) = e.get("id").and_then(|v| v.as_str()) {
        let existing = read_strict(kind)?;
        if existing.iter().any(|r| r.get("id").and_then(|v| v.as_str()) == Some(id)) {
            return Err(StoreError::Semantic(format!(
                "{}: id {:?} already recorded — supersede it, do not mutate",
                kind, id
            )));
        }
    }
    let mut f = OpenOptions::new().create(true).read(true).append(true).open(&path)
        .map_err(|e| StoreError::Io(e.to_string()))?;
    f.lock_exclusive().map_err(|e| StoreError::Io(format!("lock: {}", e)))?;
    let result = (|| -> Result<(), StoreError> {
        let text = fs::read_to_string(&path).map_err(|e| StoreError::Io(e.to_string()))?;
        let mut prev = GENESIS.to_string();
        let mut count = 0;
        for line in text.lines() {
            let t = line.trim();
            if t.is_empty() {
                continue;
            }
            count += 1;
            let v: Value = serde_json::from_str(t).map_err(|e| StoreError::Corrupt {
                file: path.file_name().unwrap().to_string_lossy().into_owned(),
                line: count,
                msg: format!("invalid JSON: {}", e),
            })?;
            if let Some(h) = v.get("hash").and_then(|v| v.as_str()) {
                prev = h.to_string();
            }
        }
        e.insert("schema".into(), Value::Number(SCHEMA.into()));
        e.insert("prev_hash".into(), Value::String(prev));
        let at = time::now_iso();
        e.insert("created_at".into(), Value::String(at));
        if !e.contains_key("actor") {
            e.insert("actor".into(), Value::String("lucidota".into()));
        }
        let h = cjson::hash_record(&Value::Object(e.clone())).map_err(StoreError::Io)?;
        e.insert("hash".into(), Value::String(h));
        let line = cjson::canonical(&Value::Object(e)).map_err(StoreError::Io)?;
        f.write_all(line.as_bytes()).map_err(|e| StoreError::Io(e.to_string()))?;
        f.write_all(b"\n").map_err(|e| StoreError::Io(e.to_string()))?;
        f.flush().map_err(|e| StoreError::Io(e.to_string()))?;
        #[cfg(unix)]
        {
            use std::os::unix::io::AsRawFd;
            let r = unsafe { libc_fsync(f.as_raw_fd()) };
            if r != 0 {
                return Err(StoreError::Io("fsync failed".into()));
            }
        }
        Ok(())
    })();
    let _ = fs2::FileExt::unlock(&f);
    result
}

#[cfg(unix)]
unsafe extern "C" {
    fn fsync(fd: i32) -> i32;
}
#[cfg(unix)]
fn libc_fsync(fd: i32) -> i32 {
    unsafe { fsync(fd) }
}

/// Verify hash-chain integrity of one kind file.
pub fn verify_chain(kind: &str, entries: &[Map<String, Value>]) -> Result<usize, StoreError> {
    let fname = file_for(kind)?.file_name().unwrap().to_string_lossy().into_owned();
    let mut prev = GENESIS.to_string();
    for (i, e) in entries.iter().enumerate() {
        let ln = i + 1;
        let got_prev = e.get("prev_hash").and_then(|v| v.as_str()).ok_or_else(|| StoreError::Chain {
            file: fname.clone(),
            line: ln,
            msg: "missing prev_hash".into(),
        })?;
        if got_prev != prev {
            return Err(StoreError::Chain { file: fname.clone(), line: ln, msg: "prev_hash mismatch".into() });
        }
        let mut no_hash = e.clone();
        let h = no_hash.remove("hash").and_then(|v| v.as_str().map(|s| s.to_string()))
            .ok_or_else(|| StoreError::Chain { file: fname.clone(), line: ln, msg: "missing hash".into() })?;
        let recomputed = cjson::hash_record(&Value::Object(no_hash)).map_err(StoreError::Io)?;
        if recomputed != h {
            return Err(StoreError::Chain { file: fname.clone(), line: ln, msg: "hash mismatch".into() });
        }
        prev = h;
    }
    Ok(entries.len())
}

/// Effective status of a record id: SUPERSEDED if any supersession has from=id,
/// else the record's own status field. C12: superseded material never
/// silently resurrects — callers must surface this status.
pub fn effective_status(
    id: &str,
    supersessions: &[Map<String, Value>],
    own_status: Option<&str>,
) -> &'static str {
    let hit = supersessions.iter().any(|s| {
        s.get("from").and_then(|v| v.as_str()) == Some(id)
    });
    if hit {
        "SUPERSEDED"
    } else {
        match own_status {
            Some("active") => "ACTIVE",
            Some("proposed") => "PROPOSED",
            Some("rejected") => "REJECTED",
            Some("decided") => "DECIDED",
            _ => "UNKNOWN",
        }
    }
}

/// Index id -> record for a kind.
pub fn index(entries: &[Map<String, Value>]) -> BTreeMap<String, &Map<String, Value>> {
    let mut m = BTreeMap::new();
    for e in entries {
        if let Some(id) = e.get("id").and_then(|v| v.as_str()) {
            m.insert(id.to_string(), e);
        }
    }
    m
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn kinds_have_files() {
        assert_eq!(KINDS.len(), 4);
        for (k, f) in KINDS {
            assert!(f.ends_with(".jsonl"), "{} -> {}", k, f);
        }
    }
    #[test]
    fn superseded_wins() {
        let sup = vec![{
            let mut m = Map::new();
            m.insert("from".into(), Value::String("X".into()));
            m
        }];
        assert_eq!(effective_status("X", &sup, Some("active")), "SUPERSEDED");
        assert_eq!(effective_status("Y", &sup, Some("active")), "ACTIVE");
        assert_eq!(effective_status("Z", &sup, None), "UNKNOWN");
    }
}
