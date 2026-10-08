//! Bake build provenance into the canon binary for deployment identity.
//! CANON_RUSTC_VERSION: the rustc version string.
//! CANON_SOURCE_DATE_EPOCH: "set" or "unset" (presence only, not the value).
fn main() {
    let rustc = std::env::var("RUSTC").unwrap_or_else(|_| "rustc".into());
    let version = std::process::Command::new(&rustc)
        .arg("--version")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".into());
    println!("cargo:rustc-env=CANON_RUSTC_VERSION={}", version);
    let sde = if std::env::var("SOURCE_DATE_EPOCH").is_ok() {
        "set"
    } else {
        "unset"
    };
    println!("cargo:rustc-env=CANON_SOURCE_DATE_EPOCH={}", sde);
    println!("cargo:rerun-if-env-changed=SOURCE_DATE_EPOCH");
}
