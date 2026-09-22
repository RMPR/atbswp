//! Slint front end.  All blocking work (recording, playback, dialogs) runs on
//! worker threads and reports back through `slint::invoke_from_event_loop`.

use atbswp_core::record;
use atbswp_macro::Macro;
use slint::{ComponentHandle, Weak};
use std::path::{Path, PathBuf};
use std::process::Child;
use std::sync::{Arc, Mutex};
use std::thread;

mod settings;

slint::include_modules!();

/// Same video tutorial the Python version opens from its Help button.
const HELP_URL: &str = "https://youtu.be/L0jjSgX5FYk";
const ABOUT_URL: &str = "https://github.com/rmpr/atbswp";

#[derive(Default)]
struct State {
    macro_: Macro,
    settings: settings::Settings,
    /// Currently running player, so Stop can end it.
    child: Option<Child>,
    /// Set by Stop; checked by the play worker before and right after
    /// spawning, so a Stop clicked before the child exists still wins.
    cancel_play: bool,
    /// Where the current macro came from, used as default for Save.
    path: Option<PathBuf>,
}

type Shared = Arc<Mutex<State>>;

/// How a playback ended; turned into a translated status on the UI thread.
enum Outcome {
    Done,
    Stopped,
    Exit(String),
    Quiet,
}

fn show_macro(ui: &MainWindow, m: &Macro) {
    ui.set_event_count(m.events.len() as i32);
    ui.set_duration_s(m.duration_us() as f32 / 1e6);
}

/// Push the persisted settings into the settings window.
fn show_settings(ui: &SettingsWindow, s: &settings::Settings) {
    ui.set_fast_play(s.fast_play);
    ui.set_infinite(s.infinite);
    ui.set_repeat(s.repeat as i32);
    ui.set_hotkey_index(settings::fkey_index(s.recording_hotkey));
    ui.set_stay_on_top(s.always_on_top);
    ui.set_recording_timer(s.recording_timer as i32);
    ui.set_mouse_speed(s.mouse_speed_ms as i32);
    ui.set_language_index(settings::language_index(&s.language));
}

/// Switch every @tr() string in both windows, live.
fn apply_language(setting: &str) {
    let lang = settings::effective_language(setting);
    if let Err(e) = slint::select_bundled_translation(lang) {
        eprintln!("atbswp: no bundled translation for {lang}: {e}");
    }
}

/// Read the settings window back.
fn settings_from_ui(ui: &SettingsWindow) -> settings::Settings {
    settings::Settings {
        fast_play: ui.get_fast_play(),
        infinite: ui.get_infinite(),
        repeat: ui.get_repeat().max(1) as u32,
        recording_hotkey: settings::FKEYS[ui.get_hotkey_index().clamp(0, 11) as usize],
        always_on_top: ui.get_stay_on_top(),
        recording_timer: ui.get_recording_timer().max(0) as u32,
        mouse_speed_ms: ui.get_mouse_speed().max(1) as u32,
        language: settings::LANGUAGES[ui.get_language_index().clamp(0, 8) as usize].to_string(),
    }
}

/// Playback settings travel in the macro header.
fn apply_settings(s: &settings::Settings, m: &mut Macro) {
    m.header.repeat = if s.infinite { 0 } else { s.repeat };
    m.header.speed_percent = s.speed_percent();
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
    let settings_win = SettingsWindow::new().expect("cannot create settings window");
    let state: Shared = Arc::default();
    {
        let loaded = settings::Settings::load();
        apply_language(&loaded.language);
        show_settings(&settings_win, &loaded);
        settings_win.set_version(env!("CARGO_PKG_VERSION").into());
        ui.set_stay_on_top(loaded.always_on_top);
        state.lock().unwrap().settings = loaded;
    }
    {
        let sw = settings_win.as_weak();
        ui.on_settings_clicked(move || {
            if let Some(w) = sw.upgrade() {
                let _ = w.show();
            }
        });
    }
    if std::env::var_os("ATBSWP_GUI_SHOW_SETTINGS").is_some() {
        let _ = settings_win.show(); // for screenshots/tests
    }

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
                            ui.set_status(ui.invoke_msg_loaded(path.display().to_string().into()));
                        });
                    }
                    Err(e) => set_status(&weak, e),
                }
            });
        });
    }

    // Save ------------------------------------------------------------------
    {
        let (weak, state) = (ui.as_weak(), state.clone());
        ui.on_save_clicked(move || {
            let (weak, state) = (weak.clone(), state.clone());
            if weak.upgrade().is_none() {
                return;
            }
            let mut m = state.lock().unwrap().macro_.clone();
            apply_settings(&state.lock().unwrap().settings, &mut m);
            let default_name = state
                .lock()
                .unwrap()
                .path
                .as_ref()
                .and_then(|p| p.file_stem().map(|s| s.to_string_lossy().into_owned()))
                .unwrap_or_else(|| "macro".into());
            thread::spawn(move || {
                // the macro is an executable; scripts are the editable alternative
                let dialog = rfd::FileDialog::new()
                    .set_title("Save macro")
                    .set_file_name(format!("{default_name}.com"))
                    .add_filter("Standalone macro (runs anywhere)", &["com", "exe"])
                    .add_filter("Editable script", &["txt"])
                    .add_filter("Raw payload (binary)", &["atbswp"]);
                let Some(path) = dialog.save_file() else {
                    return;
                };
                match atbswp_core::save_as(&path, &m, None) {
                    Ok(()) => {
                        state.lock().unwrap().path = Some(path.clone());
                        let p = path.display().to_string();
                        on_ui(&weak, move |ui| {
                            let m = ui.invoke_msg_wrote(p.into());
                            ui.set_status(m);
                        });
                    }
                    Err(e) => set_status(&weak, e),
                }
            });
        });
    }

    // Record --------------------------------------------------------------
    {
        let (weak, state) = (ui.as_weak(), state.clone());
        ui.on_record_toggled(move || {
            let Some(ui) = weak.upgrade() else { return };
            if ui.get_recording() {
                record::request_stop();
                ui.set_status(ui.invoke_msg_stopping());
                return;
            }
            let s = state.lock().unwrap().settings.clone();
            let hotkey = atbswp_macro::keys::key_name(s.recording_hotkey).unwrap_or("the hotkey");
            ui.set_recording(true);
            ui.set_status(
                if s.recording_timer > 0 {
                    format!(
                        "Recording starts in {}s… press {hotkey} or the record button to stop",
                        s.recording_timer
                    )
                } else {
                    format!("Recording… press {hotkey} or the record button to stop")
                }
                .into(),
            );
            let (weak, state) = (weak.clone(), state.clone());
            thread::spawn(move || {
                let opts = record::Options {
                    stop_key: Some(s.recording_hotkey),
                    min_move_interval_us: s.mouse_speed_ms * 1000,
                    ..record::Options::default()
                };
                // recording timer: give the user time to switch windows
                let started = std::time::Instant::now();
                while started.elapsed().as_secs() < s.recording_timer as u64 {
                    if weak.upgrade_in_event_loop(|_| {}).is_err() {
                        return;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(100));
                }
                set_status(
                    &weak,
                    format!("Recording… press {hotkey} or the record button to stop"),
                );
                let result = record::record(&opts);
                on_ui(&weak, move |ui| {
                    ui.set_recording(false);
                    match result {
                        Ok(mut m) => {
                            apply_settings(&state.lock().unwrap().settings, &mut m);
                            show_macro(&ui, &m);
                            ui.set_status(ui.invoke_msg_recorded(m.events.len() as i32));
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
                ui.set_status(ui.invoke_msg_stopped());
                return;
            }
            let mut m = state.lock().unwrap().macro_.clone();
            apply_settings(&state.lock().unwrap().settings, &mut m);
            let player = match atbswp_core::player_bytes(None) {
                Ok(p) => p,
                Err(e) => {
                    ui.set_status(e.into());
                    return;
                }
            };
            ui.set_playing(true);
            ui.set_status(ui.invoke_msg_playing());
            state.lock().unwrap().cancel_play = false;
            let (weak, state) = (weak.clone(), state.clone());
            thread::spawn(move || {
                let outcome = (|| {
                    let tmp = atbswp_core::TempExe::new(&m, &player)?;
                    if state.lock().unwrap().cancel_play {
                        return Ok(Outcome::Stopped);
                    }
                    let mut child = atbswp_core::spawn_ape(&tmp.path, &[])
                        .map_err(|e| format!("running player: {e}"))?;
                    let mut st = state.lock().unwrap();
                    if st.cancel_play {
                        // Stop was clicked while we were spawning
                        atbswp_core::stop_child(&mut child, std::time::Duration::from_millis(1500));
                        let _ = child.wait();
                        return Ok(Outcome::Stopped);
                    }
                    st.child = Some(child);
                    drop(st);
                    loop {
                        let mut st = state.lock().unwrap();
                        let Some(child) = st.child.as_mut() else {
                            break Ok(Outcome::Quiet);
                        };
                        match child.try_wait() {
                            Ok(Some(status)) => {
                                st.child = None;
                                break Ok(if status.success() {
                                    Outcome::Done
                                } else {
                                    Outcome::Exit(status.to_string())
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
                        Ok(Outcome::Done) => ui.set_status(ui.invoke_msg_done()),
                        Ok(Outcome::Stopped) => ui.set_status(ui.invoke_msg_stopped()),
                        Ok(Outcome::Exit(st)) => {
                            ui.set_status(ui.invoke_msg_player_exit(st.into()))
                        }
                        Ok(Outcome::Quiet) => {}
                        Err(e) => ui.set_status(e.into()),
                    }
                });
            });
        });
    }

    // Settings / Help / About / Exit -------------------------------------------
    {
        let (weak, sw, state) = (ui.as_weak(), settings_win.as_weak(), state.clone());
        settings_win.on_changed(move || {
            let (Some(ui), Some(w)) = (weak.upgrade(), sw.upgrade()) else {
                return;
            };
            let s = settings_from_ui(&w);
            ui.set_stay_on_top(s.always_on_top);
            let mut st = state.lock().unwrap();
            apply_settings(&s, &mut st.macro_);
            if let Err(e) = s.save() {
                ui.set_status(ui.invoke_msg_settings_error(e.to_string().into()));
            }
            st.settings = s;
        });
    }
    {
        let (sw, state) = (settings_win.as_weak(), state.clone());
        settings_win.on_language_changed(move || {
            let Some(w) = sw.upgrade() else { return };
            let s = settings_from_ui(&w);
            apply_language(&s.language);
            let _ = s.save();
            state.lock().unwrap().settings = s;
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
    {
        let weak = ui.as_weak();
        settings_win.on_about_clicked(move || {
            open_url(ABOUT_URL);
            if let Some(ui) = weak.upgrade() {
                ui.set_status(format!("atbswp {} — {ABOUT_URL}", env!("CARGO_PKG_VERSION")).into());
            }
        });
    }
    settings_win.on_exit_clicked(|| {
        let _ = slint::quit_event_loop();
    });

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
