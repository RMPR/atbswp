//! Embeds the pre-compiled portable player (player/build/player.com) into the
//! CLI so `atbswp export` needs nothing else at runtime.
//!
//! Override the location with ATBSWP_PLAYER=/path/to/player.com.  When no
//! player is found the build still succeeds and `export` explains how to
//! build one (or pass `--player`).
use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    let out = PathBuf::from(env::var("OUT_DIR").unwrap()).join("player.com");
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let default = manifest.join("../../player/build/player.com");
    let candidate = env::var_os("ATBSWP_PLAYER")
        .map(PathBuf::from)
        .unwrap_or(default);

    println!("cargo:rerun-if-env-changed=ATBSWP_PLAYER");
    println!("cargo:rerun-if-changed={}", candidate.display());

    match fs::read(&candidate) {
        Ok(bytes) if !bytes.is_empty() => {
            fs::write(&out, bytes).unwrap();
            println!("cargo:rustc-cfg=embedded_player");
            println!(
                "cargo:warning=embedding player from {}",
                candidate.display()
            );
        }
        _ => {
            fs::write(&out, []).unwrap();
            println!(
                "cargo:warning=no player.com found at {} (run `make -C player`); export will need --player",
                candidate.display()
            );
        }
    }
    println!("cargo:rustc-check-cfg=cfg(embedded_player)");
}
