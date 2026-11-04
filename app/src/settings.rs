//! The sound and the panel look, kept between runs in the family's data
//! folder: `~/.local/share/timsters-audio-works/poly-box` on Linux,
//! `%APPDATA%\Timster's Audio Works\Poly Box` on Windows,
//! `~/Library/Application Support/Timster's Audio Works/Poly Box` on a Mac.

use crate::controls::{defaults, Values, DEFS};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;
use taw_panel::theme::{KnobStyle, PanelKind};

#[derive(Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Saved {
    pub panel: PanelKind,
    pub knobs: KnobStyle,
    /// Control values by their ids; unknown ids are ignored, missing ones
    /// take their defaults, so old files keep loading.
    pub values: BTreeMap<String, f32>,
    /// The note the computer's A key plays.
    pub keys_base: Option<u8>,
}

impl Saved {
    pub fn values(&self) -> Values {
        let mut v = defaults();
        for (i, d) in DEFS.iter().enumerate() {
            if let Some(&x) = self.values.get(d.id) {
                if x.is_finite() {
                    v[i] = x.clamp(d.spec.min, d.spec.max);
                }
            }
        }
        v
    }

    pub fn set_values(&mut self, v: &Values) {
        self.values = DEFS
            .iter()
            .zip(v.iter())
            .map(|(d, &x)| (d.id.to_string(), x))
            .collect();
    }
}

pub fn folder() -> Option<PathBuf> {
    let env = |k: &str| std::env::var_os(k).map(PathBuf::from);
    if cfg!(windows) {
        env("APPDATA").map(|p| p.join("Timster's Audio Works").join("Poly Box"))
    } else if cfg!(target_os = "macos") {
        env("HOME").map(|p| {
            p.join("Library/Application Support")
                .join("Timster's Audio Works")
                .join("Poly Box")
        })
    } else {
        env("XDG_DATA_HOME")
            .or_else(|| env("HOME").map(|h| h.join(".local/share")))
            .map(|p| p.join("timsters-audio-works").join("poly-box"))
    }
}

fn file() -> Option<PathBuf> {
    folder().map(|f| f.join("settings.json"))
}

pub fn load() -> Saved {
    file()
        .and_then(|f| std::fs::read_to_string(f).ok())
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save(s: &Saved) {
    let Some(path) = file() else { return };
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Ok(json) = serde_json::to_string_pretty(s) {
        // Write beside it and rename, so a crash mid-write can't leave half a file.
        let tmp = path.with_extension("json.tmp");
        if std::fs::write(&tmp, json).is_ok() {
            let _ = std::fs::rename(&tmp, &path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::controls::Ctl;

    #[test]
    fn values_round_trip_and_bad_ones_are_tamed() {
        let mut v = defaults();
        v[Ctl::Cutoff as usize] = 440.0;
        let mut s = Saved::default();
        s.set_values(&v);
        s.values.insert("resonance".into(), 7.0);
        s.values.insert("no_such_control".into(), 1.0);
        let json = serde_json::to_string(&s).unwrap();
        let back: Saved = serde_json::from_str(&json).unwrap();
        let v2 = back.values();
        assert_eq!(v2[Ctl::Cutoff as usize], 440.0);
        assert_eq!(v2[Ctl::Resonance as usize], 1.0);
    }

    #[test]
    fn an_empty_file_gives_the_defaults() {
        let s: Saved = serde_json::from_str("{}").unwrap();
        assert_eq!(s.values(), defaults());
    }
}
