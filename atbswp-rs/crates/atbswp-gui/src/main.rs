//! Slint front end.  All blocking work (recording, playback, dialogs) runs on
//! worker threads and reports back through `slint::invoke_from_event_loop`.

use atbswp_core::record;
use atbswp_macro::Macro;
use slint::{ComponentHandle, Weak};
use std::path::{Path, PathBuf};
use std::process::Child;
use std::sync::{Arc, Mutex};
use std::thread;

slint::include_modules!();

const HELP_URL: &str = "https://github.com/rmpr/atbswp";

#[derive(Default)]
struct State {
    macro_: Macro,
    /// Currently running player, so Stop can end it.
    child: Option<Child>,
    /// Set by Stop; checked by the play worker before and right after
    /// spawning, so a Stop clicked before the child exists still wins.
    cancel_play: bool,
    /// Where the current macro came from, used as default for Save.
    path: Option<PathBuf>,
}

type Shared = Arc<Mutex<State>>;

fn show_macro(ui: &MainWindow, m: &Macro) {
    ui.set_event_count(m.events.len() as i32);
    ui.set_duration_s(m.duration_us() as f32 / 1e6);
    ui.set_repeat(m.header.repeat.max(1) as i32);
    ui.set_infinite(m.header.repeat == 0);
    ui.set_speed(m.header.speed_percent.max(10) as i32);
}

fn header_from_ui(ui: &MainWindow, m: &mut Macro) {
    m.header.repeat = if ui.get_infinite() {
        0
    } else {
        ui.get_repeat().max(1) as u32
    };
    m.header.speed_percent = ui.get_speed().max(10) as u32;
}

/// Run `f` on the UI thread.
fn on_ui(weak: &Weak<MainWindow>, f: impl FnOnce(MainWindow) + Send + 'static) {
    let w = weak.clone();
    let _ = slint::invoke_from_event_loop(move || {
        if let Some(ui) = w.upgrade() {
            f(ui)
        }
    });
}

fn set_status(weak: &Weak<MainWindow>, msg: impl Into<String>) {
    let msg = msg.into();
    on_ui(weak, move |ui| ui.set_status(msg.into()));
}

fn file_filter(d: rfd::FileDialog) -> rfd::FileDialog {
    d.add_filter("Standalone macros", &["com", "exe"])
        .add_filter("Macro scripts (editable text)", &["txt"])
        .add_filter("Raw macro payloads (binary)", &["atbswp"])
        .add_filter("All files", &["*"])
}

fn main() {
    let ui = MainWindow::new().expect("cannot create window");
    let state: Shared = Arc::default();

    if atbswp_core::EMBEDDED_PLAYER.is_empty() && std::env::var_os("ATBSWP_PLAYER").is_none() {
        ui.set_status(
            "No player embedded: run `make -C player` and rebuild, or set ATBSWP_PLAYER".into(),
        );
    }

    // Load ---------------------------------------------------------------
    {
        let (weak, state) = (ui.as_weak(), state.clone());
        ui.on_load_clicked(move || {
            let (weak, state) = (weak.clone(), state.clone());
            thread::spawn(move || {
                let Some(path) = file_filter(rfd::FileDialog::new())
                    .set_title("Load capture")
                    .pick_file()
                else {
                    return;
                };
                match atbswp_core::load(&path) {
                    Ok(m) => {
                        let mut st = state.lock().unwrap();
                        st.macro_ = m.clone();
                        st.path = Some(path.clone());
                        drop(st);
                        on_ui(&weak, move |ui| {
                            show_macro(&ui, &m);
                            ui.set_status(format!("Loaded {}", path.display()).into());
                        });
                    }
                    Err(e) => set_status(&weak, e),
                }
            });
        });
    }

    // Save / Compile ----------------------------------------------------------
    for compile in [false, true] {
        let (weak, state) = (ui.as_weak(), state.clone());
        let handler = move || {
            let (weak, state) = (weak.clone(), state.clone());
            let Some(ui) = weak.upgrade() else { return };
            let mut m = state.lock().unwrap().macro_.clone();
            header_from_ui(&ui, &mut m);
            let default_name = state
                .lock()
                .unwrap()
                .path
                .as_ref()
                .and_then(|p| p.file_stem().map(|s| s.to_string_lossy().into_owned()))
                .unwrap_or_else(|| "capture".into());
            thread::spawn(move || {
                let dialog = if compile {
                    rfd::FileDialog::new()
                        .set_title("Compile to standalone executable")
                        .set_file_name(format!("{default_name}.com"))
                        .add_filter("Standalone macro", &["com", "exe"])
                } else {
                    // the macro is an executable; scripts are the editable alternative
                    rfd::FileDialog::new()
                        .set_title("Save macro")
                        .set_file_name(format!("{default_name}.com"))
                        .add_filter("Standalone macro (runs anywhere)", &["com", "exe"])
                        .add_filter("Editable script", &["txt"])
                        .add_filter("Raw payload (binary)", &["atbswp"])
                };
                let Some(path) = dialog.save_file() else {
                    return;
                };
                let result = if compile {
                    atbswp_core::player_bytes(None)
                        .and_then(|p| atbswp_core::write_exe(&path, &p, &m))
                } else {
                    atbswp_core::save_as(&path, &m, None)
                };
                match result {
                    Ok(()) => {
                        if !compile {
                            state.lock().unwrap().path = Some(path.clone());
                        }
                        set_status(&weak, format!("Wrote {}", path.display()));
                    }
                    Err(e) => set_status(&weak, e),
                }
            });
        };
        if compile {
            ui.on_compile_clicked(handler);
        } else {
            ui.on_save_clicked(handler);
        }
    }

    // Record --------------------------------------------------------------
    {
        let (weak, state) = (ui.as_weak(), state.clone());
        ui.on_record_toggled(move || {
            let Some(ui) = weak.upgrade() else { return };
            if ui.get_recording() {
                record::request_stop();
                ui.set_status("Stopping…".into());
                return;
            }
            ui.set_recording(true);
            ui.set_status("Recording… press F12 or the record button to stop".into());
            let (weak, state) = (weak.clone(), state.clone());
            thread::spawn(move || {
                let opts = record::Options::default(); // F12 stops, pkexec on Wayland
                let result = record::record(&opts);
                on_ui(&weak, move |ui| {
                    ui.set_recording(false);
                    match result {
                        Ok(mut m) => {
                            header_from_ui(&ui, &mut m);
                            show_macro(&ui, &m);
                            ui.set_status(format!("Recorded {} events", m.events.len()).into());
                            let mut st = state.lock().unwrap();
                            st.macro_ = m;
                            st.path = None;
                        }
                        Err(e) => ui.set_status(e.into()),
                    }
                });
            });
        });
    }

    // Play ----------------------------------------------------------------
    {
        let (weak, state) = (ui.as_weak(), state.clone());
        ui.on_play_toggled(move || {
            let Some(ui) = weak.upgrade() else { return };
            if ui.get_playing() {
                let mut st = state.lock().unwrap();
                st.cancel_play = true;
                if let Some(child) = st.child.as_mut() {
                    // SIGTERM lets the player release held keys/buttons first
                    atbswp_core::stop_child(child, std::time::Duration::from_millis(1500));
                }
                ui.set_status("Stopped".into());
                return;
            }
            let mut m = state.lock().unwrap().macro_.clone();
            header_from_ui(&ui, &mut m);
            let player = match atbswp_core::player_bytes(None) {
                Ok(p) => p,
                Err(e) => {
                    ui.set_status(e.into());
                    return;
                }
            };
            ui.set_playing(true);
            ui.set_status("Playing…".into());
            let (weak, state) = (weak.clone(), state.clone());
            thread::spawn(move || {
                let outcome = (|| {
                    let tmp = atbswp_core::TempExe::new(&m, &player)?;
                    let child = atbswp_core::spawn_ape(&tmp.path, &[])
                        .map_err(|e| format!("running player: {e}"))?;
                    state.lock().unwrap().child = Some(child);
                    loop {
                        let mut st = state.lock().unwrap();
                        let Some(child) = st.child.as_mut() else {
                            break Ok(String::new());
                        };
                        match child.try_wait() {
                            Ok(Some(status)) => {
                                st.child = None;
                                break Ok(if status.success() {
                                    "Done".into()
                                } else {
                                    format!("Player exited with {status}")
                                });
                            }
                            Ok(None) => {}
                            Err(e) => {
                                st.child = None;
                                break Err(e.to_string());
                            }
                        }
                        drop(st);
                        thread::sleep(std::time::Duration::from_millis(50));
                    }
                })();
                on_ui(&weak, move |ui| {
                    ui.set_playing(false);
                    match outcome {
                        Ok(msg) if !msg.is_empty() => ui.set_status(msg.into()),
                        Ok(_) => {}
                        Err(e) => ui.set_status(e.into()),
                    }
                });
            });
        });
    }

    // Settings / Help -------------------------------------------------------
    {
        let (weak, state) = (ui.as_weak(), state.clone());
        ui.on_settings_changed(move || {
            if let Some(ui) = weak.upgrade() {
                header_from_ui(&ui, &mut state.lock().unwrap().macro_);
            }
        });
    }
    {
        let weak = ui.as_weak();
        ui.on_help_clicked(move || {
            let opened = open_url(HELP_URL);
            if let Some(ui) = weak.upgrade() {
                ui.set_status(
                    if opened {
                        format!("Opened {HELP_URL}")
                    } else {
                        HELP_URL.to_string()
                    }
                    .into(),
                );
            }
        });
    }

    ui.run().expect("event loop failed");
}

fn open_url(url: &str) -> bool {
    let cmd: (&str, &[&str]) = if cfg!(target_os = "windows") {
        ("cmd", &["/C", "start", "", url])
    } else if cfg!(target_os = "macos") {
        ("open", &[url])
    } else {
        ("xdg-open", &[url])
    };
    std::process::Command::new(cmd.0)
        .args(cmd.1)
        .spawn()
        .is_ok()
}

#[allow(dead_code)]
fn _assert_path(_: &Path) {}
