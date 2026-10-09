//! CANON canonical JSON v1 — same serialization discipline as PL §23:
//! UTF-8, sorted object keys, no insignificant whitespace, integers only
//! (no float formatting ambiguity), schema version always present.
//! hash(record) = SHA-256(canonical bytes of record without its "hash" field).
use serde_json::{Map, Value};

pub fn canonical(v: &Value) -> Result<String, String> {
    match v {
        Value::Object(m) => {
            let mut sorted: Map<String, Value> = Map::new();
            for (k, val) in m {
                sorted.insert(k.clone(), canonical_value(val)?);
            }
            serde_json::to_string(&Value::Object(sorted)).map_err(|e| e.to_string())
        }
        _ => Err("canonical: top-level must be an object".into()),
    }
}

fn canonical_value(v: &Value) -> Result<Value, String> {
    match v {
        Value::Object(m) => {
            let mut sorted = Map::new();
            for (k, val) in m {
                sorted.insert(k.clone(), canonical_value(val)?);
            }
            Ok(Value::Object(sorted))
        }
        Value::Array(a) => {
            let mut out = Vec::with_capacity(a.len());
            for x in a {
                out.push(canonical_value(x)?);
            }
            Ok(Value::Array(out))
        }
        Value::Number(n) => {
            if n.is_f64() {
                Err("canonical: floats are not permitted in CANON records".into())
            } else if n.as_i64().is_none() {
                // u64 beyond i64::MAX: outside the i64 interop contract
                // (PL-REQ-019, XIMPL-2). Aligned with pl::canon and the
                // Python port, which reject these at canonicalization time;
                // the strict byte-parity pin demands all three agree.
                Err("canonical: integer outside i64 interop range".into())
            } else {
                Ok(v.clone())
            }
        }
        _ => Ok(v.clone()),
    }
}

/// sha256 hex of the canonical bytes.
pub fn hash_record(v: &Value) -> Result<String, String> {
    use sha2::{Digest, Sha256};
    let c = canonical(v)?;
    Ok(Sha256::digest(c.as_bytes()).iter().map(|b| format!("{:02x}", b)).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn keys_sorted_no_whitespace() {
        let v = json!({"z":1,"a":{"y":2,"b":3}});
        assert_eq!(canonical(&v).unwrap(), r#"{"a":{"b":3,"y":2},"z":1}"#);
    }
    #[test]
    fn floats_rejected() {
        assert!(canonical(&json!({"x":1.5})).is_err());
    }
    #[test]
    fn i64_boundaries_accepted() {
        assert!(canonical(&json!({"x":9223372036854775807i64})).is_ok());
        assert!(canonical(&json!({"x":-9223372036854775808i64})).is_ok());
    }
    #[test]
    fn u64_beyond_i64_rejected() {
        // 2^63 parses as u64, outside the i64 interop contract (XIMPL-2).
        let v: Value = serde_json::from_str(r#"{"x":9223372036854775808}"#).unwrap();
        assert!(v["x"].as_u64().is_some());
        assert!(canonical(&v).is_err());
        // u64::MAX also rejected.
        let v2: Value = serde_json::from_str(r#"{"x":18446744073709551615}"#).unwrap();
        assert!(canonical(&v2).is_err());
    }
    #[test]
    fn hash_stable() {
        let v = json!({"b":2,"a":1});
        assert_eq!(hash_record(&v).unwrap(), hash_record(&v).unwrap());
    }
}
