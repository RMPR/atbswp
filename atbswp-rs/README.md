# atbswp-rs

Rust rewrite of atbswp with one new idea at its core: a recorded macro is
exported as a **single standalone executable that runs on Linux, Windows and
macOS, on x86-64 and ARM64, without installing anything**. The recorder is a
normal native program; the exported macro does not depend on it.

```
 record (Rust, Linux)        export (Rust, any OS)           run (anywhere)
┌──────────────────┐   ┌────────────────────────────┐   ┌─────────────────────┐
│ /dev/input evdev │──▶│ player.com + payload + ftr │──▶│ my-macro.com        │
│ or a text script │   │ (APE, ~450 KiB, prebuilt)  │   │ Wayland: libei      │
└──────────────────┘   └────────────────────────────┘   │ X11: XTest          │
                                                        │ Windows: SendInput  │
                                                        │ macOS: CoreGraphics │
                                                        └─────────────────────┘
```

## Layout

| Path | What |
|------|------|
| `player/` | The macro player, in C, compiled **once** with [cosmocc] into an Actually Portable Executable (fat x86-64 + aarch64, all OSes). |
| `crates/atbswp-macro` | Dependency-free library: binary payload format, text script format, stored-zip embedding. Mirrors `player/src/macro_format.h`. |
| `crates/atbswp-cli` | `atbswp` command: `record`, `export`, `dump`, `play`, `player`. |
| `crates/atbswp-core` | Recording backends (XRecord, Windows hooks, macOS event tap, evdev), player embedding, export and launch helpers shared by CLI and GUI. |
| `crates/atbswp-gui` | Slint front end: the classic one-row toolbar (load, save, record, play, compile, settings, help). Built with `cargo build -p atbswp-gui`; not a default member because Slint takes a few minutes to compile. |
| `tests/e2e.sh` | Exports a macro and replays it through libei into a real EIS server. No compositor needed. |

## Build

```sh
make -C player                        # downloads cosmocc on first run, writes player/build/player.com
cargo build --release                 # CLI; embeds player/build/player.com (atbswp-core/build.rs)
cargo build --release -p atbswp-gui   # Slint GUI (winit + femtovg, no GTK at runtime)
```

Only Linux hosts can build the player today because cosmocc ships as Linux
binaries; the Rust crates build anywhere and can take a pre-built player via
`ATBSWP_PLAYER=/path/to/player.com` or `atbswp export --player`.

## Use

The macro **is** an executable. Recording writes one; you run it.

```sh
atbswp record -o demo.com     # press F12 to stop
./demo.com                    # Linux / macOS (or: sh demo.com)
demo.com                      # Windows (rename to .exe if you prefer)
./demo.com --repeat 0 --speed 200
```

The same `demo.com` runs unmodified on Linux (Wayland and X11), Windows, and
macOS, whichever platform it was recorded on: events are stored as evdev
codes and absolute positions are scaled from the recorded screen to the
target's. CI exports a macro on Ubuntu and runs it on Windows, Apple Silicon
and Intel macOS runners on every push.

Scripts are the editable alternative and convert both ways:

```sh
atbswp dump demo.com > demo.txt   # macro -> script
$EDITOR demo.txt
atbswp export demo.txt -o demo.com   # script -> macro
atbswp play demo.txt --dry-run       # try a script without exporting
cat > demo.txt <<'M'
screen 1920x1080
move 640 360
wait 50ms
click left
key a
scroll down 2
M
```

`record -o FILE.txt` writes the script form directly; `.atbswp` is the raw
binary payload.

## How the standalone executable works

An APE is also a valid zip archive, and cosmopolitan exposes its entries as
`/zip/…` at runtime. The exporter stores the macro as one uncompressed entry:

```
+--------------------------------------------+
| player.com  (APE, identical for all)       |
|   /zip/player-macos-x86_64  Intel helper   |  optional, added in CI
|   /zip/macro.bin                            |
|      header  32 bytes   version, count,    |
|                         screen, repeat,    |
|      events  16 bytes   speed              |
+--------------------------------------------+
```

`unzip -l my-macro.com` lists the contents. Exporting appends the entry and
rewrites the zip directory, no compression involved, so it is instant. On
start the player reads `/zip/macro.bin` straight into memory; nothing is
parsed beyond fixed-size structs, nothing is compiled, and there is no
runtime to unpack, so a macro is running a few milliseconds after launch.
The player keeps cosmopolitan's `-mtiny` runtime and pulls the zip
filesystem back in with a `__static_yoink`, which costs about 60 KiB.

All OS libraries are loaded at runtime with `cosmo_dlopen`, so the player has
zero link-time dependencies:

| Platform | Library | Notes |
|----------|---------|-------|
| Wayland | `libei.so.1` via the XDG RemoteDesktop portal (`libdbus-1.so.3`) | GNOME 45+, KDE Plasma 6. The portal asks once; the restore token is cached in `$XDG_STATE_HOME/atbswp-portal-token`. `LIBEI_SOCKET` bypasses the portal (used by the tests). |
| X11 | `libX11.so.6` + `libXtst.so.6` | Automatic fallback when no portal answers and `DISPLAY` is set. Force with `ATBSWP_BACKEND=xtest`. |
| Windows | `user32.dll` SendInput | Scan codes, extended keys, hi-res wheel. |
| macOS | CoreGraphics event taps | Apple Silicon via `cosmo_dlopen`; Intel via a bundled native x86-64 helper (see limitations). Needs Accessibility permission for the macro file. |

Key codes are stored as evdev codes and translated per platform in
`player/src/keymap.c`. Absolute mouse positions are scaled from the recorded
screen size to the target's.

## Recording

Recording is native and unprivileged wherever the platform allows it:

| Platform | Source | Privilege | Pointer |
|----------|--------|-----------|---------|
| X11 | XRecord extension | none | absolute |
| Windows | `WH_KEYBOARD_LL` / `WH_MOUSE_LL` hooks | none | absolute |
| macOS | listen-only `CGEventTap` | Input Monitoring, prompted once per app | absolute |
| Wayland | evdev (`/dev/input`) | polkit prompt via `pkexec` per recording, or root, or the `input` group | relative only |

Wayland is the odd one out because no compositor protocol lets a client
watch input passively. libei's receiver side, fed by the InputCapture portal,
is built for Input-Leap-style tools: it only delivers events after the
pointer crosses a barrier you define, and while it does the desktop stops
receiving them. So on Wayland the recorder reads evdev, and when `/dev/input`
is not readable it re-runs itself through `pkexec`, which shows the desktop's
authorisation dialog; only the small recorder runs privileged and the file is
written by the unprivileged parent. `--no-elevate` disables that. Because
evdev sees devices rather than the cursor, Wayland recordings contain
relative motion, which replays exactly only with the same pointer
acceleration (flat profile) on both ends. Clicks, keys and scrolling are
exact everywhere.

Key codes are stored as evdev codes; `player/src/keymap.c` is the single
table for Windows scancodes and macOS virtual keycodes, and
`player/tools/gen_keymap_rs.py` generates the recorder's copy from it.

## Text script reference

```
screen WxH            recording screen size (enables scaling)
repeat N              0 = forever
speed PCT             100 = real time
wait 50ms | 2s | 300us | 1500
move X Y              absolute
moverel DX DY
click left|right|middle|side|extra
buttondown / buttonup BUTTON
key KEY_A | a | enter | 30
keydown / keyup KEY
scroll DX DY          1/120 notch units, +y = down
scroll up|down|left|right N
```

## Testing

Every injection path is checked the same way: the exported macro plays for
real, an independent observer records what the OS delivered, and the test
diffs it against `tests/golden.txt`.

| Path | Observer | Where |
|------|----------|-------|
| libei protocol | `player/tests/eis_sink.c`, a real EIS server on `LIBEI_SOCKET` | `tests/e2e.sh`, local + CI |
| RemoteDesktop portal (libdbus, restore token) | `tests/fake_portal.py` on a private bus, handing the sink's socket to the player | `tests/e2e_portal.sh`, local + CI |
| XTest | Xvfb, `xinput test-xi2 --root`, `xdotool getmouselocation` | `tests/e2e_x11.sh`, CI |
| Windows SendInput | `tests/win/HookListener.cs`, low-level keyboard/mouse hooks | `tests/win/run.ps1`, CI |
| macOS CoreGraphics, arm64 dlopen and Intel helper | pointer position via `tests/mac/cursor.c` | CI (hosted runners honour mouse events; keys are not observed) |
| Recorders (XRecord, Windows hooks) | `atbswp record` itself, while the golden macro plays; `tests/check_recording.py` diffs the result | `tests/e2e_x11_record.sh`, `tests/win/record.ps1`, CI |

```sh
cargo test                 # format, text and footer round trips
make -C player test        # APE: dump, dry-run, timing (no display needed)
make -C player e2e         # libei -> EIS server
sh tests/e2e_portal.sh     # through the fake portal (python3-dbus, python3-gi)
sh tests/e2e_x11.sh macro.com   # needs xvfb-run, xinput, xdotool
```

CI (`.github/workflows/atbswp-rs.yml`) builds the APE on Ubuntu and the
Intel helper on macOS, bundles them, runs all of the above, then plays the
exported file on Windows, Apple Silicon and Intel macOS runners.

## Limitations, honestly

* **Wayland recording is relative-motion only** (see Recording). A
  compositor-side recorder would need a new portal; nothing standard exists.
* macOS: on Apple Silicon the APE plays natively through `cosmo_dlopen`.
  On Intel Macs cosmopolitan loads the binary itself, so Apple's dynamic
  linker is absent and `dlopen` is impossible (Rosetta does not change
  that). CI therefore builds the *same player sources* natively with clang
  as an x86-64 Mach-O and stores it as `/zip/player-macos-x86_64`
  (`player/tools/bundle.py`). The kernel can only exec a real file, so on an
  Intel Mac the APE copies it once to `$TMPDIR` and execs it with
  `--payload <itself>`; the helper reads `macro.bin` with its own tiny zip
  reader (`player/src/zipread.c`).
  A locally built player lacks the helper unless you run
  `make -C player bundle HELPER=...` with one from a macOS build. Either way
  the macro file needs Accessibility permission. Double-click detection
  relies on the OS click-state, which is not emulated yet.
* The GUI records with the same evdev backend and stops on F12 or the
  record button; file dialogs go through the XDG portal (`rfd`). It has not
  been exercised on a real display yet, only compiled.
* Windows: keys not in the keymap table (media keys, some international
  keys) are skipped, `--verbose` tells you which.
* Linux without the `ape` binfmt handler runs the file through `sh` first
  (about 5 ms); installing the `ape` loader system-wide removes that hop.

[cosmocc]: https://github.com/jart/cosmopolitan/blob/master/tool/cosmocc/README.md
