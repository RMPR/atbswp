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
        return exe::player_of(&b).map_err(|e| format!("{}: {e}", p.display()));
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

/// A macro written to a temporary executable inside a freshly created
/// private directory (mode 0700, exclusive creation, unpredictable name), so
/// no other local user can pre-create or redirect the path.  Removed on drop.
pub struct TempExe {
    pub path: PathBuf,
    dir: PathBuf,
}

fn random_suffix() -> String {
    // Enough entropy for an unguessable name without pulling in a crate.
    let mut bytes = [0u8; 16];
    let ok = fs::File::open("/dev/urandom")
        .and_then(|mut f| std::io::Read::read_exact(&mut f, &mut bytes))
        .is_ok();
    if !ok {
        use std::hash::{BuildHasher, Hasher};
        let mut h = std::collections::hash_map::RandomState::new().build_hasher();
        h.write_u128(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0),
        );
        h.write_u32(std::process::id());
        bytes[..8].copy_from_slice(&h.finish().to_le_bytes());
        let mut h2 = std::collections::hash_map::RandomState::new().build_hasher();
        h2.write_u64(u64::from_le_bytes(bytes[..8].try_into().unwrap()));
        bytes[8..].copy_from_slice(&h2.finish().to_le_bytes());
    }
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn private_temp_dir() -> io::Result<PathBuf> {
    let base = std::env::temp_dir();
    for _ in 0..16 {
        let dir = base.join(format!("atbswp-{}", random_suffix()));
        let mut b = fs::DirBuilder::new();
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            b.mode(0o700);
        }
        match b.create(&dir) {
            Ok(()) => return Ok(dir),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e),
        }
    }
    Err(io::Error::other(
        "could not create a private temporary directory",
    ))
}

impl TempExe {
    pub fn new(m: &Macro, player: &[u8]) -> Result<Self, String> {
        let dir = private_temp_dir().map_err(|e| format!("temporary directory: {e}"))?;
        let path = dir.join("macro.com");
        let bytes = exe::build(player, m).map_err(|e| e.to_string())?;
        let mut opts = fs::OpenOptions::new();
        opts.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(0o700);
        }
        let mut f = opts
            .open(&path)
            .map_err(|e| format!("{}: {e}", path.display()))?;
        std::io::Write::write_all(&mut f, &bytes)
            .map_err(|e| format!("{}: {e}", path.display()))?;
        drop(f);
        make_executable(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        Ok(TempExe { path, dir })
    }
}

impl Drop for TempExe {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
        let _ = fs::remove_dir(&self.dir);
    }
}

/// Ask a running player to stop gracefully (it releases held keys and
/// buttons first), falling back to a hard kill after `grace`.
pub fn stop_child(child: &mut Child, grace: std::time::Duration) {
    #[cfg(unix)]
    unsafe {
        libc::kill(child.id() as libc::pid_t, libc::SIGTERM);
    }
    #[cfg(not(unix))]
    let _ = grace;
    let deadline = std::time::Instant::now() + grace;
    while std::time::Instant::now() < deadline {
        if let Ok(Some(_)) = child.try_wait() {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    let _ = child.kill();
}

/// Export to a temp file, run it to completion, clean up.
pub fn play(m: &Macro, player: &[u8], extra: &[&str]) -> Result<ExitStatus, String> {
    let tmp = TempExe::new(m, player)?;
    let mut child = spawn_ape(&tmp.path, extra).map_err(|e| format!("running player: {e}"))?;
    child.wait().map_err(|e| format!("running player: {e}"))
}
