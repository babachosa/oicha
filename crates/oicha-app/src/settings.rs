use oicha_core::storage::atomic_write;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    #[default]
    Vi,
    En,
}

impl Language {
    pub fn text(self, vietnamese: &'static str) -> &'static str {
        if self == Self::En {
            crate::translations::CATALOG
                .iter()
                .find_map(|(vi, en)| (*vi == vietnamese).then_some(*en))
                .unwrap_or(vietnamese)
        } else {
            vietnamese
        }
    }
    pub fn choose(self, vi: &'static str, en: &'static str) -> &'static str {
        if self == Self::En { en } else { vi }
    }
}

macro_rules! ui_format {
    ($language:expr, $vi:literal, $en:literal $(, $args:expr)* $(,)?) => {
        if $language == crate::settings::Language::En {
            format!($en $(, $args)*)
        } else {
            format!($vi $(, $args)*)
        }
    };
}
pub(crate) use ui_format;

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Preferences {
    pub language: Language,
}

pub fn data_dir() -> PathBuf {
    // QA sessions must never read or change the writer's real preferences.
    let args: Vec<_> = std::env::args_os().collect();
    if args.get(1).is_some_and(|s| s == "--smoke-test")
        && let Some(path) = args.get(2)
    {
        return PathBuf::from(path).with_extension("settings");
    }
    directories::ProjectDirs::from("dev", "Oicha", "Oicha")
        .map(|p| p.data_local_dir().to_owned())
        .unwrap_or_else(|| std::env::temp_dir().join("oicha"))
}

impl Preferences {
    pub fn load() -> Self {
        Self::read(&data_dir().join("preferences.json"))
    }
    fn read(path: &Path) -> Self {
        match std::fs::read(path) {
            Ok(bytes) if bytes.len() <= 16 * 1024 => {
                serde_json::from_slice(&bytes).unwrap_or_default()
            }
            _ => Self::default(),
        }
    }
    pub fn save(&self) -> anyhow::Result<()> {
        atomic_write(
            &data_dir().join("preferences.json"),
            &serde_json::to_vec_pretty(self)?,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preferences_survive_restart_and_ignore_unknown_fields() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("prefs.json");
        atomic_write(&path, br#"{"language":"en","future":true}"#).unwrap();
        assert_eq!(Preferences::read(&path).language, Language::En);
        atomic_write(&path, b"broken").unwrap();
        assert_eq!(Preferences::read(&path).language, Language::Vi);
    }
    #[test]
    fn catalog_has_unique_keys_and_nonempty_english() {
        let mut keys = std::collections::HashSet::new();
        for (vi, en) in crate::translations::CATALOG {
            assert!(keys.insert(vi), "Duplicate translation: {vi}");
            assert!(!en.is_empty());
            assert_eq!(Language::En.text(vi), *en);
            assert_eq!(Language::Vi.text(vi), *vi);
        }
    }
}
