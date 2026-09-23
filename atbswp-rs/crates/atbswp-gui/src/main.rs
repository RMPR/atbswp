//! Slint front end: the classic one-row toolbar plus a settings window.
//!
//! Every blocking action (dialogs, recording, playback) runs on a worker
//! thread and reports back through the Slint event loop.  All user-visible
//! text lives in `ui/app.slint` as `@tr()` strings; Rust only picks which
//! message to show.

mod settings;

use atbswp_core::record;
use atbswp_macro::Macro;
use settings::Settings;
use slint::{ComponentHandle, Weak};
use std::path::PathBuf;
use std::process::Child;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

slint::include_modules!();

/// Same video tutorial the Python version opens from its Help button.
const HELP_URL: &str = "https://youtu.be/L0jjSgX5FYk";
const ABOUT_URL: &str = "https://github.com/rmpr/atbswp";
/// How long a stopped player gets to release held keys before being killed.
const STOP_GRACE: Duration = Duration::from_millis(1500);

#[derive(Default)]
struct State {
    macro_: Macro,
    settings: Settings,
    /// Where the current macro came from, used as the default Save name.
    path: Option<PathBuf>,
    /// The running player, so Stop can end it.
    child: Option<Child>,
    /// Set by Stop; the play worker checks it before and right after
    /// spawning, so a Stop clicked before the child exists still wins.
    cancel_play: bool,
}

type Shared = Arc<Mutex<State>>;

/// Everything a handler needs: both windows and the shared state.
#[derive(Clone)]
struct App {
    ui: Weak<MainWindow>,
    settings_win: Weak<SettingsWindow>,
    state: Shared,
}

impl App {
    /// Run `f` on the UI thread with the main window (no-op after exit).
    fn on_ui(&self, f: impl FnOnce(&MainWindow) + Send + 'static) {
        let weak = self.ui.clone();
        let _ = slint::invoke_from_event_loop(move || {
            if let Some(ui) = weak.upgrade() {
                f(&ui);
            }
        });
    }

    fn set_status(&self, msg: impl Into<String>) {
        let msg = msg.into();
        self.on_ui(move |ui| ui.set_status(msg.into()));
    }

    fn macro_(&self) -> Macro {
        let st = self.state.lock().unwrap();
        let mut m = st.macro_.clone();
        apply_settings(&st.settings, &mut m);
        m
    }

    fn settings(&self) -> Settings {
        self.state.lock().unwrap().settings.clone()
    }

    fn default_name(&self) -> String {
        self.state
            .lock()
            .unwrap()
            .path
            .as_ref()
            .and_then(|p| p.file_stem().map(|s| s.to_string_lossy().into_owned()))
            .unwrap_or_else(|| "macro".into())
    }
}

/// Playback settings travel in the macro header.
fn apply_settings(s: &Settings, m: &mut Macro) {
    m.header.repeat = if s.infinite { 0 } else { s.repeat };
    m.header.speed_percent = s.speed_percent();
}

fn show_macro(ui: &MainWindow, m: &Macro) {
    ui.set_event_count(m.events.len() as i32);
    ui.set_duration_s(m.duration_us() as f32 / 1e6);
}

fn show_settings(w: &SettingsWindow, s: &Settings) {
    w.set_fast_play(s.fast_play);
    w.set_infinite(s.infinite);
    w.set_repeat(s.repeat as i32);
    w.set_hotkey_index(settings::fkey_index(s.recording_hotkey));
    w.set_stay_on_top(s.always_on_top);
    w.set_recording_timer(s.recording_timer as i32);
    w.set_mouse_speed(s.mouse_speed_ms as i32);
    w.set_language_index(settings::language_index(&s.language));
}

fn settings_from_ui(w: &SettingsWindow) -> Settings {
    Settings {
        fast_play: w.get_fast_play(),
        infinite: w.get_infinite(),
        repeat: w.get_repeat().max(1) as u32,
        recording_hotkey: settings::FKEYS[w.get_hotkey_index().clamp(0, 11) as usize],
        always_on_top: w.get_stay_on_top(),
        recording_timer: w.get_recording_timer().max(0) as u32,
        mouse_speed_ms: w.get_mouse_speed().max(1) as u32,
        language: settings::LANGUAGES[w.get_language_index().clamp(0, 8) as usize].to_string(),
    }
}

/// Switch every @tr() string in both windows, live.
fn apply_language(setting: &str) {
    let lang = settings::effective_language(setting);
    if let Err(e) = slint::select_bundled_translation(lang) {
        eprintln!("atbswp: no bundled translation for {lang}: {e}");
    }
}

fn macro_filters(d: rfd::FileDialog) -> rfd::FileDialog {
    d.add_filter("Standalone macros", &["com", "exe"])
        .add_filter("Macro scripts (editable text)", &["txt"])
        .add_filter("Raw macro payloads (binary)", &["atbswp"])
}

// ---- actions ---------------------------------------------------------------

fn load(app: &App) {
    let app = app.clone();
    thread::spawn(move || {
        let Some(path) = macro_filters(rfd::FileDialog::new())
            .set_title("Load macro")
            .pick_file()
        else {
            return;
        };
        match atbswp_core::load(&path) {
            Ok(m) => {
                let mut st = app.state.lock().unwrap();
                st.macro_ = m.clone();
                st.path = Some(path.clone());
                drop(st);
                app.on_ui(move |ui| {
                    show_macro(ui, &m);
                    ui.set_status(ui.invoke_msg_loaded(path.display().to_string().into()));
                });
            }
            Err(e) => app.set_status(e),
        }
    });
}

/// The macro is an executable; scripts are the editable alternative.
fn save(app: &App) {
    let app = app.clone();
    let m = app.macro_();
    let name = app.default_name();
    thread::spawn(move || {
        let dialog = rfd::FileDialog::new()
            .set_title("Save macro")
            .set_file_name(format!("{name}.com"))
            .add_filter("Standalone macro (runs anywhere)", &["com", "exe"])
            .add_filter("Editable script", &["txt"])
            .add_filter("Raw payload (binary)", &["atbswp"]);
        let Some(path) = dialog.save_file() else {
            return;
        };
        match atbswp_core::save_as(&path, &m, None) {
            Ok(()) => {
                app.state.lock().unwrap().path = Some(path.clone());
                app.on_ui(move |ui| {
                    ui.set_status(ui.invoke_msg_wrote(path.display().to_string().into()))
                });
            }
            Err(e) => app.set_status(e),
        }
    });
}

fn toggle_recording(app: &App, ui: &MainWindow) {
    if ui.get_recording() {
        record::request_stop();
        ui.set_status(ui.invoke_msg_stopping());
        return;
    }
    let s = app.settings();
    let hotkey = atbswp_macro::keys::key_name(s.recording_hotkey).unwrap_or("the hotkey");
    ui.set_recording(true);
    ui.set_status(if s.recording_timer > 0 {
        ui.invoke_msg_recording_in(s.recording_timer as i32, hotkey.into())
    } else {
        ui.invoke_msg_recording(hotkey.into())
    });

    let app = app.clone();
    thread::spawn(move || {
        // recording timer: time to switch to the window being automated
        let started = std::time::Instant::now();
        while started.elapsed().as_secs() < s.recording_timer as u64 {
            if app.ui.upgrade_in_event_loop(|_| {}).is_err() {
                return;
            }
            thread::sleep(Duration::from_millis(100));
        }
        if s.recording_timer > 0 {
            app.on_ui(move |ui| ui.set_status(ui.invoke_msg_recording(hotkey.into())));
        }
        let opts = record::Options {
            stop_key: Some(s.recording_hotkey),
            min_move_interval_us: s.mouse_speed_ms * 1000,
            ..record::Options::default()
        };
        let result = record::record(&opts);
        let state = app.state.clone();
        app.on_ui(move |ui| {
            ui.set_recording(false);
            match result {
                Ok(m) => {
                    show_macro(ui, &m);
                    ui.set_status(ui.invoke_msg_recorded(m.events.len() as i32));
                    let mut st = state.lock().unwrap();
                    st.macro_ = m;
                    st.path = None;
                }
                Err(e) => ui.set_status(e.into()),
            }
        });
    });
}

/// How a playback ended; turned into a translated status on the UI thread.
enum Outcome {
    Done,
    Stopped,
    Exit(String),
}

/// Export to a private temp file, run it, and watch it until it exits or
/// Stop is clicked.
fn run_player(app: &App, m: Macro, player: Vec<u8>) -> Result<Outcome, String> {
    let tmp = atbswp_core::TempExe::new(&m, &player)?;
    if app.state.lock().unwrap().cancel_play {
        return Ok(Outcome::Stopped);
    }
    let mut child =
        atbswp_core::spawn_ape(&tmp.path, &[]).map_err(|e| format!("running player: {e}"))?;
    {
        let mut st = app.state.lock().unwrap();
        if st.cancel_play {
            // Stop was clicked while we were spawning
            atbswp_core::stop_child(&mut child, STOP_GRACE);
            let _ = child.wait();
            return Ok(Outcome::Stopped);
        }
        st.child = Some(child);
    }
    loop {
        let mut st = app.state.lock().unwrap();
        let Some(child) = st.child.as_mut() else {
            return Ok(Outcome::Stopped);
        };
        match child.try_wait() {
            Ok(Some(status)) => {
                st.child = None;
                return Ok(if status.success() {
                    Outcome::Done
                } else {
                    Outcome::Exit(status.to_string())
                });
            }
            Ok(None) => {}
            Err(e) => {
                st.child = None;
                return Err(e.to_string());
            }
        }
        drop(st);
        thread::sleep(Duration::from_millis(50));
    }
}

fn toggle_playback(app: &App, ui: &MainWindow) {
    if ui.get_playing() {
        let mut st = app.state.lock().unwrap();
        st.cancel_play = true;
        if let Some(child) = st.child.as_mut() {
            // SIGTERM lets the player release held keys/buttons first
            atbswp_core::stop_child(child, STOP_GRACE);
        }
        ui.set_status(ui.invoke_msg_stopped());
        return;
    }
    let player = match atbswp_core::player_bytes(None) {
        Ok(p) => p,
        Err(e) => {
            ui.set_status(e.into());
            return;
        }
    };
    let m = app.macro_();
    app.state.lock().unwrap().cancel_play = false;
    ui.set_playing(true);
    ui.set_status(ui.invoke_msg_playing());

    let app = app.clone();
    thread::spawn(move || {
        let outcome = run_player(&app, m, player);
        app.on_ui(move |ui| {
            ui.set_playing(false);
            match outcome {
                Ok(Outcome::Done) => ui.set_status(ui.invoke_msg_done()),
                Ok(Outcome::Stopped) => ui.set_status(ui.invoke_msg_stopped()),
                Ok(Outcome::Exit(st)) => ui.set_status(ui.invoke_msg_player_exit(st.into())),
                Err(e) => ui.set_status(e.into()),
            }
        });
    });
}

fn settings_changed(app: &App, w: &SettingsWindow) {
    let s = settings_from_ui(w);
    if let Some(ui) = app.ui.upgrade() {
        ui.set_stay_on_top(s.always_on_top);
        if let Err(e) = s.save() {
            ui.set_status(ui.invoke_msg_settings_error(e.to_string().into()));
        }
    }
    app.state.lock().unwrap().settings = s;
}

fn open_url(url: &str) -> bool {
    let (cmd, args): (&str, &[&str]) = if cfg!(target_os = "windows") {
        ("cmd", &["/C", "start", "", url])
    } else if cfg!(target_os = "macos") {
        ("open", &[url])
    } else {
        ("xdg-open", &[url])
    };
    std::process::Command::new(cmd).args(args).spawn().is_ok()
}

// ---- wiring ------------------------------------------------------------------

fn main() {
    let ui = MainWindow::new().expect("cannot create window");
    let settings_win = SettingsWindow::new().expect("cannot create settings window");
    let app = App {
        ui: ui.as_weak(),
        settings_win: settings_win.as_weak(),
        state: Arc::default(),
    };

    let loaded = Settings::load();
    apply_language(&loaded.language);
    show_settings(&settings_win, &loaded);
    settings_win.set_version(env!("CARGO_PKG_VERSION").into());
    ui.set_stay_on_top(loaded.always_on_top);
    app.state.lock().unwrap().settings = loaded;
    if atbswp_core::EMBEDDED_PLAYER.is_empty() && std::env::var_os("ATBSWP_PLAYER").is_none() {
        ui.set_status(ui.invoke_msg_no_player());
    }
    if std::env::var_os("ATBSWP_GUI_SHOW_SETTINGS").is_some() {
        let _ = settings_win.show(); // for screenshots/tests
    }

    let a = app.clone();
    ui.on_load_clicked(move || load(&a));
    let a = app.clone();
    ui.on_save_clicked(move || save(&a));
    let a = app.clone();
    ui.on_record_toggled(move || {
        if let Some(ui) = a.ui.upgrade() {
            toggle_recording(&a, &ui);
        }
    });
    let a = app.clone();
    ui.on_play_toggled(move || {
        if let Some(ui) = a.ui.upgrade() {
            toggle_playback(&a, &ui);
        }
    });
    let a = app.clone();
    ui.on_settings_clicked(move || {
        if let Some(w) = a.settings_win.upgrade() {
            let _ = w.show();
        }
    });
    let a = app.clone();
    ui.on_help_clicked(move || {
        let opened = open_url(HELP_URL);
        if let Some(ui) = a.ui.upgrade() {
            ui.set_status(if opened {
                ui.invoke_msg_opened(HELP_URL.into())
            } else {
                HELP_URL.into()
            });
        }
    });

    let a = app.clone();
    settings_win.on_changed(move || {
        if let Some(w) = a.settings_win.upgrade() {
            settings_changed(&a, &w);
        }
    });
    let a = app.clone();
    settings_win.on_language_changed(move || {
        if let Some(w) = a.settings_win.upgrade() {
            apply_language(&settings_from_ui(&w).language);
            settings_changed(&a, &w);
        }
    });
    let a = app.clone();
    settings_win.on_about_clicked(move || {
        open_url(ABOUT_URL);
        if let Some(ui) = a.ui.upgrade() {
            ui.set_status(format!("atbswp {} — {ABOUT_URL}", env!("CARGO_PKG_VERSION")).into());
        }
    });
    settings_win.on_exit_clicked(|| {
        let _ = slint::quit_event_loop();
    });

    ui.run().expect("event loop failed");
}
