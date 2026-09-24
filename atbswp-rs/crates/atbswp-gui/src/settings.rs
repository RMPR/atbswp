//! Persistent GUI settings, the same set the Python version kept in
//! `atbswp.cfg`, stored as simple `key = value` lines.

use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    pub fast_play: bool,
    pub infinite: bool,
    pub repeat: u32,
    /// evdev code of the key that stops a recording (F2..F12)
    pub recording_hotkey: u16,
    pub always_on_top: bool,
    /// seconds to wait before a recording starts
    pub recording_timer: u32,
    /// motion coalescing interval in ms ("mouse speed" in the Python version)
    pub mouse_speed_ms: u32,
    /// UI language code ("" = follow the system locale)
    pub language: String,
}

/// Bundled UI languages, in the order of the settings combo box (index 0 =
/// system default).  The Python version shipped the same set.
pub const LANGUAGES: [&str; 9] = ["", "de", "en", "es", "fr", "it", "ja", "pl", "tr"];

pub fn language_index(code: &str) -> i32 {
    LANGUAGES
        .iter()
        .position(|l| *l == code)
        .map(|i| i as i32)
        .unwrap_or(0)
}

/// The language to use: the setting, else the system locale if we bundle it.
pub fn effective_language(setting: &str) -> &'static str {
    if let Some(l) = LANGUAGES.iter().find(|l| !l.is_empty() && **l == setting) {
        return l;
    }
    let locale = ["LC_ALL", "LC_MESSAGES", "LANG"]
        .iter()
        .find_map(|v| {
            std::env::var(v)
                .ok()
                .filter(|s| !s.is_empty() && s != "C" && s != "POSIX")
        })
        .unwrap_or_default();
    let code = locale
        .split(['_', '.', '@'])
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    LANGUAGES
        .iter()
        .find(|l| !l.is_empty() && **l == code)
        .copied()
        .unwrap_or("en")
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            fast_play: false,
            infinite: false,
            repeat: 1,
            recording_hotkey: 88, // KEY_F12
            always_on_top: true,
            recording_timer: 0,
            mouse_speed_ms: 21,
            language: String::new(),
        }
    }
}

/// evdev codes of F1..F12 (index 0 = F1).
pub const FKEYS: [u16; 12] = [59, 60, 61, 62, 63, 64, 65, 66, 67, 68, 87, 88];

pub fn fkey_index(code: u16) -> i32 {
    FKEYS
        .iter()
        .position(|c| *c == code)
        .map(|i| i as i32)
        .unwrap_or(11)
}

pub fn config_path() -> Option<PathBuf> {
    let base = if cfg!(target_os = "windows") {
        std::env::var_os("APPDATA").map(PathBuf::from)
    } else if cfg!(target_os = "macos") {
        std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Library/Application Support"))
    } else {
        std::env::var_os("XDG_CONFIG_HOME")
            .filter(|s| !s.is_empty())
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
    }?;
    Some(base.join("atbswp").join("gui.cfg"))
}

impl Settings {
    pub fn load() -> Settings {
        let mut s = Settings::default();
        let Some(path) = config_path() else { return s };
        let Ok(text) = std::fs::read_to_string(path) else {
            return s;
        };
        for line in text.lines() {
            let Some((k, v)) = line.split_once('=') else {
                continue;
            };
            let v = v.trim();
            match k.trim() {
                "fast_play" => s.fast_play = v == "true",
                "infinite" => s.infinite = v == "true",
                "repeat" => s.repeat = v.parse().unwrap_or(1).max(1),
                "recording_hotkey" => s.recording_hotkey = v.parse().unwrap_or(88),
                "always_on_top" => s.always_on_top = v == "true",
                "recording_timer" => s.recording_timer = v.parse().unwrap_or(0),
                "mouse_speed_ms" => s.mouse_speed_ms = v.parse().unwrap_or(21).max(1),
                "language" => s.language = v.to_string(),
                _ => {}
            }
        }
        s
    }

    pub fn save(&self) -> std::io::Result<()> {
        let Some(path) = config_path() else {
            return Ok(());
        };
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(
            path,
            format!(
                "fast_play = {}\ninfinite = {}\nrepeat = {}\nrecording_hotkey = {}\nalways_on_top = {}\nrecording_timer = {}\nmouse_speed_ms = {}\n",
                self.fast_play,
                self.infinite,
                self.repeat,
                self.recording_hotkey,
                self.always_on_top,
                self.recording_timer,
                self.mouse_speed_ms
            ),
        )
    }

    pub fn speed_percent(&self) -> u32 {
        if self.fast_play { 200 } else { 100 }
    }
}
