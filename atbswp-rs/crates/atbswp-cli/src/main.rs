//! `atbswp` command line: record macros, export standalone executables.
//!
//! Argument parsing is hand rolled to keep the binary dependency-free.

use atbswp_core::record;
use atbswp_macro::{Macro, text};
use std::fs;
use std::path::Path;
use std::process::ExitCode;

const USAGE: &str = "\
atbswp - record mouse & keyboard macros, export them as portable executables

usage:
  atbswp record  [-o MACRO.com] [--stop-key KEY] [--screen WxH] [--min-move-interval MS]
                 [--no-elevate]
  atbswp export  INPUT -o OUTPUT [--player PLAYER.COM] [--repeat N] [--speed PCT]
  atbswp dump    INPUT [--binary]
  atbswp play    INPUT [--repeat N] [--speed PCT] [--dry-run] [--player PLAYER.COM]
  atbswp player  -o OUTPUT                 write the embedded bare player

INPUT may be a text script (.txt), a binary payload (.atbswp) or an exported
executable; the format is detected automatically.

record captures the keyboard and mouse on X11 (XRecord), Windows (low-level
hooks) and macOS (event tap, asks for Input Monitoring once).  On Wayland it
reads /dev/input and, unless --no-elevate, asks for authorisation through
pkexec when those devices are not readable.  It stops on Ctrl-C or the stop
key (default KEY_F12, which is not recorded) and writes a standalone
executable (macro-<time>.com unless -o is given) that runs as-is on Linux,
Windows and macOS.  Use -o FILE.txt for an editable script or -o FILE.atbswp
for the raw payload; export turns either back into an executable.
";

struct Args {
    positional: Vec<String>,
    flags: Vec<(String, Option<String>)>,
}

impl Args {
    fn parse(argv: &[String], with_value: &[&str]) -> Result<Args, String> {
        let mut a = Args {
            positional: vec![],
            flags: vec![],
        };
        let mut it = argv.iter();
        while let Some(x) = it.next() {
            if let Some(name) = x
                .strip_prefix("--")
                .or_else(|| x.strip_prefix('-'))
                .filter(|_| x.len() > 1)
            {
                if let Some((n, v)) = name.split_once('=') {
                    a.flags.push((n.to_string(), Some(v.to_string())));
                } else if with_value.contains(&name) {
                    let v = it.next().ok_or_else(|| format!("--{name} needs a value"))?;
                    a.flags.push((name.to_string(), Some(v.clone())));
                } else {
                    a.flags.push((name.to_string(), None));
                }
            } else {
                a.positional.push(x.clone());
            }
        }
        Ok(a)
    }
    fn has(&self, name: &str) -> bool {
        self.flags.iter().any(|(n, _)| n == name)
    }
    fn value(&self, name: &str) -> Option<&str> {
        self.flags
            .iter()
            .rev()
            .find(|(n, _)| n == name)
            .and_then(|(_, v)| v.as_deref())
    }
    fn u32(&self, name: &str) -> Result<Option<u32>, String> {
        match self.value(name) {
            None => Ok(None),
            Some(v) => v
                .parse()
                .map(Some)
                .map_err(|_| format!("--{name}: expected a number, got `{v}`")),
        }
    }
}

fn load_macro(path: &str) -> Result<Macro, String> {
    atbswp_core::load(Path::new(path))
}

fn player_bytes(args: &Args) -> Result<Vec<u8>, String> {
    atbswp_core::player_bytes(args.value("player").map(Path::new))
}

fn apply_overrides(m: &mut Macro, args: &Args) -> Result<(), String> {
    if let Some(r) = args.u32("repeat")? {
        m.header.repeat = r;
    }
    if let Some(s) = args.u32("speed")? {
        m.header.speed_percent = s;
    }
    Ok(())
}

fn cmd_export(args: &Args) -> Result<(), String> {
    let input = args.positional.first().ok_or("export: missing INPUT")?;
    let output = args
        .value("o")
        .or(args.value("output"))
        .ok_or("export: missing -o OUTPUT")?;
    let mut m = load_macro(input)?;
    apply_overrides(&mut m, args)?;
    let player = player_bytes(args)?;
    atbswp_core::write_exe(Path::new(output), &player, &m)?;
    eprintln!(
        "wrote {output}: {} events, {:.2}s, {} KiB",
        m.events.len(),
        m.duration_us() as f64 / 1e6,
        (player.len() + m.events.len() * 16 + 48) / 1024
    );
    Ok(())
}

fn cmd_dump(args: &Args) -> Result<(), String> {
    let input = args.positional.first().ok_or("dump: missing INPUT")?;
    let m = load_macro(input)?;
    if args.has("binary") {
        use std::io::Write;
        std::io::stdout()
            .write_all(&m.encode())
            .map_err(|e| e.to_string())?;
    } else {
        print!("{}", text::render(&m));
    }
    Ok(())
}

fn cmd_player(args: &Args) -> Result<(), String> {
    let output = args
        .value("o")
        .or(args.value("output"))
        .ok_or("player: missing -o OUTPUT")?;
    let player = player_bytes(args)?;
    atbswp_core::write_player(Path::new(output), &player)?;
    eprintln!("wrote {output} ({} KiB)", player.len() / 1024);
    Ok(())
}

fn cmd_play(args: &Args) -> Result<(), String> {
    let input = args.positional.first().ok_or("play: missing INPUT")?;
    let mut m = load_macro(input)?;
    apply_overrides(&mut m, args)?;
    let player = player_bytes(args)?;
    let mut extra: Vec<&str> = vec![];
    if args.has("dry-run") {
        extra.push("--dry-run");
    }
    if args.has("verbose") || args.has("v") {
        extra.push("--verbose");
    }
    let status = atbswp_core::play(&m, &player, &extra)?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("player exited with {status}"))
    }
}

fn cmd_record(args: &Args) -> Result<(), String> {
    let out = args.value("o").or(args.value("output"));
    let opts = record::Options {
        stop_key: match args.value("stop-key") {
            Some(k) => {
                Some(atbswp_macro::keys::key_code(k).ok_or_else(|| format!("unknown key `{k}`"))?)
            }
            None => Some(88), // KEY_F12
        },
        screen: match args.value("screen") {
            Some(s) => {
                let (w, h) = s.split_once('x').ok_or("--screen expects WxH")?;
                Some((
                    w.parse().map_err(|_| "bad width")?,
                    h.parse().map_err(|_| "bad height")?,
                ))
            }
            None => None,
        },
        min_move_interval_us: args.u32("min-move-interval")?.unwrap_or(10) * 1000,
        handle_signals: true,
        allow_elevate: !args.has("no-elevate"),
    };
    let mut m = record::record(&opts)?;
    if args.has("stdout-binary") {
        // used by the pkexec-elevated helper: payload on stdout, nothing else
        use std::io::Write;
        std::io::stdout()
            .write_all(&m.encode())
            .map_err(|e| e.to_string())?;
        return Ok(());
    }
    apply_overrides(&mut m, args)?;
    let summary = format!(
        "{} events, {:.2}s",
        m.events.len(),
        m.duration_us() as f64 / 1e6
    );
    // The macro *is* an executable: that is the default output.  A text
    // script (.txt) or raw payload (.atbswp) is available for editing.
    let mut path = out.map(str::to_string).unwrap_or_else(|| {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        format!("macro-{stamp}.com")
    });
    if Path::new(&path).extension().is_none() {
        path.push_str(".com");
    }
    let p = Path::new(&path);
    let ext = p
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match ext.as_str() {
        "atbswp" => fs::write(p, m.encode()).map_err(|e| format!("{path}: {e}"))?,
        "com" | "exe" => {
            let player = player_bytes(args)?;
            atbswp_core::write_exe(p, &player, &m)?;
        }
        _ => fs::write(p, text::render(&m)).map_err(|e| format!("{path}: {e}"))?,
    }
    eprintln!("wrote {path} ({summary})");
    if matches!(ext.as_str(), "com" | "exe") {
        eprintln!(
            "run it directly on Linux, Windows or macOS; `atbswp dump {path}` shows the script"
        );
    }
    Ok(())
}

fn run() -> Result<(), String> {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let Some(cmd) = argv.first() else {
        print!("{USAGE}");
        return Ok(());
    };
    let rest = &argv[1..];
    let valued = [
        "o",
        "output",
        "player",
        "repeat",
        "speed",
        "stop-key",
        "screen",
        "min-move-interval",
    ];
    let args = Args::parse(rest, &valued)?;
    if args.has("help") || args.has("h") {
        print!("{USAGE}");
        return Ok(());
    }
    match cmd.as_str() {
        "record" => cmd_record(&args),
        "export" | "compile" => cmd_export(&args),
        "dump" => cmd_dump(&args),
        "play" => cmd_play(&args),
        "player" => cmd_player(&args),
        "--version" | "version" => {
            println!("atbswp {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        "--help" | "-h" | "help" => {
            print!("{USAGE}");
            Ok(())
        }
        other => Err(format!("unknown command `{other}`\n\n{USAGE}")),
    }
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("atbswp: {e}");
            ExitCode::FAILURE
        }
    }
}
