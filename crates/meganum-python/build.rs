//! Embeds the rustc version at build time so `meganum.about()` can report it.

fn main() {
    let rustc = std::env::var("RUSTC").unwrap_or_else(|_| "rustc".to_string());
    let out = std::process::Command::new(rustc).arg("--version").output();
    if let Ok(out) = out {
        let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if !text.is_empty() {
            println!("cargo:rustc-env=MEGANUM_RUSTC_VERSION={text}");
        }
    }
    println!("cargo:rerun-if-env-changed=RUSTC");
}
