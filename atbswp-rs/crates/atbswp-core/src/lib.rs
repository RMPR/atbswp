//! Shared plumbing for the atbswp front ends: the embedded portable player,
//! exporting standalone executables, launching them, and recording.

pub mod record;

use atbswp_macro::{Macro, exe};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus};

/// The portable player compiled by `make -C player`, embedded at build time
/// (empty when none was found; see build.rs).
pub static EMBEDDED_PLAYER: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/player.com"));

/// Resolve the player bytes: an explicit path, `$ATBSWP_PLAYER`, or the
/// embedded copy.  A path to an already exported macro is accepted too.
pub fn player_bytes(explicit: Option<&Path>) -> Result<Vec<u8>, String> {
    let from_env = std::env::var_os("ATBSWP_PLAYER").map(PathBuf::from);
    if let Some(p) = explicit.map(Path::to_path_buf).or(from_env) {
        let b = fs::read(&p).map_err(|e| format!("{}: {e}", p.display()))?;
        return Ok(exe::player_of(&b).to_vec());
    }
    if EMBEDDED_PLAYER.is_empty() {
        return Err(
            "this build has no embedded player; build one with `make -C player` \
                    and rebuild, or pass --player PLAYER.COM"
                .into(),
        );
    }
    Ok(EMBEDDED_PLAYER.to_vec())
}

#[cfg(unix)]
fn make_executable(p: &Path) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(p, fs::Permissions::from_mode(0o755))
}
#[cfg(not(unix))]
fn make_executable(_p: &Path) -> io::Result<()> {
    Ok(())
}

/// Write `player` + macro as an executable file.
pub fn write_exe(path: &Path, player: &[u8], m: &Macro) -> Result<(), String> {
    let bytes = exe::build(player, m).map_err(|e| e.to_string())?;
    fs::write(path, bytes).map_err(|e| format!("{}: {e}", path.display()))?;
    make_executable(path).map_err(|e| format!("{}: {e}", path.display()))
}

/// Write the bare player.
pub fn write_player(path: &Path, player: &[u8]) -> Result<(), String> {
    fs::write(path, player).map_err(|e| format!("{}: {e}", path.display()))?;
    make_executable(path).map_err(|e| e.to_string())
}

/// Save a macro in the representation implied by the file extension:
/// `.atbswp` binary payload, `.com`/`.exe` standalone executable, anything
/// else a text script.
pub fn save_as(path: &Path, m: &Macro, player: Option<&Path>) -> Result<(), String> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match ext.as_str() {
        "atbswp" => fs::write(path, m.encode()).map_err(|e| format!("{}: {e}", path.display())),
        "com" | "exe" => write_exe(path, &player_bytes(player)?, m),
        _ => fs::write(path, atbswp_macro::text::render(m))
            .map_err(|e| format!("{}: {e}", path.display())),
    }
}

/// Load a macro from any supported representation.
pub fn load(path: &Path) -> Result<Macro, String> {
    let bytes = fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    Macro::load(&bytes).map_err(|e| format!("{}: {e}", path.display()))
}

/// Spawn an APE.  Windows and a Linux with the `ape` binfmt registered run it
/// directly; elsewhere the kernel reports ENOEXEC and the file's shell-script
/// header has to be interpreted by `sh`, exactly as a double-click would.
pub fn spawn_ape(path: &Path, extra: &[&str]) -> io::Result<Child> {
    match Command::new(path).args(extra).spawn() {
        Ok(c) => Ok(c),
        Err(e) if cfg!(unix) && e.raw_os_error() == Some(8 /* ENOEXEC */) => {
            Command::new("/bin/sh").arg(path).args(extra).spawn()
        }
        Err(e) => Err(e),
    }
}

/// A macro written to a temporary executable, removed on drop.
pub struct TempExe {
    pub path: PathBuf,
}

impl TempExe {
    pub fn new(m: &Macro, player: &[u8]) -> Result<Self, String> {
        let path = std::env::temp_dir().join(format!(
            "atbswp-play-{}-{}.com",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis())
                .unwrap_or(0)
        ));
        write_exe(&path, player, m)?;
        Ok(TempExe { path })
    }
}

impl Drop for TempExe {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

/// Export to a temp file, run it to completion, clean up.
pub fn play(m: &Macro, player: &[u8], extra: &[&str]) -> Result<ExitStatus, String> {
    let tmp = TempExe::new(m, player)?;
    let mut child = spawn_ape(&tmp.path, extra).map_err(|e| format!("running player: {e}"))?;
    child.wait().map_err(|e| format!("running player: {e}"))
}
