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
        match retry_file_access(|| Ok(std::fs::read(path)?)) {
            Ok(bytes) if bytes.len() <= 16 * 1024 => {
                serde_json::from_slice(&bytes).unwrap_or_default()
            }
            _ => Self::default(),
        }
    }
    pub fn save(&self) -> anyhow::Result<()> {
        self.write(&data_dir().join("preferences.json"))
    }
    fn write(&self, path: &Path) -> anyhow::Result<()> {
        let bytes = serde_json::to_vec_pretty(self)?;
        retry_file_access(|| atomic_write(path, &bytes))
    }
}

fn retry_file_access<T>(mut operation: impl FnMut() -> anyhow::Result<T>) -> anyhow::Result<T> {
    // Windows readers (including sync/indexing tools) can briefly deny rename
    // or read access. Only retry native sharing/access errors, for at most
    // 235 ms of backoff. Atomic replacement keeps the previous file intact.
    #[cfg(windows)]
    for delay in [5, 10, 20, 40, 80, 80] {
        match operation() {
            Err(error)
                if error
                    .downcast_ref::<std::io::Error>()
                    .is_some_and(|error| matches!(error.raw_os_error(), Some(5 | 32 | 33))) =>
            {
                std::thread::sleep(std::time::Duration::from_millis(delay));
            }
            result => return result,
        }
    }
    operation()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(windows)]
    #[test]
    fn preference_save_recovers_after_a_temporary_windows_file_lock() {
        use std::{fs::OpenOptions, os::windows::fs::OpenOptionsExt, time::Duration};
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("preferences.json");
        Preferences {
            language: Language::En,
        }
        .write(&path)
        .unwrap();
        // A reader that does not grant FILE_SHARE_DELETE blocks atomic replace.
        let reader = OpenOptions::new()
            .read(true)
            .share_mode(1 | 2)
            .open(&path)
            .unwrap();
        let release = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(80));
            drop(reader);
        });
        let result = Preferences {
            language: Language::Vi,
        }
        .write(&path);
        release.join().unwrap();
        result.unwrap();
        assert_eq!(Preferences::read(&path).language, Language::Vi);
    }

    #[cfg(windows)]
    #[test]
    fn persistent_preference_lock_returns_error_and_preserves_the_old_file() {
        use std::{fs::OpenOptions, os::windows::fs::OpenOptionsExt};
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("preferences.json");
        Preferences {
            language: Language::En,
        }
        .write(&path)
        .unwrap();
        let original = std::fs::read(&path).unwrap();
        let reader = OpenOptions::new()
            .read(true)
            .share_mode(1 | 2)
            .open(&path)
            .unwrap();
        let result = Preferences {
            language: Language::Vi,
        }
        .write(&path);
        assert!(result.is_err());
        assert_eq!(std::fs::read(&path).unwrap(), original);
        drop(reader);
        assert_eq!(Preferences::read(&path).language, Language::En);
        assert_eq!(
            std::fs::read_dir(dir.path()).unwrap().count(),
            1,
            "retry left temporary files behind"
        );
    }

    #[cfg(windows)]
    #[test]
    fn preference_read_does_not_fall_back_to_default_during_a_temporary_lock() {
        use std::{fs::OpenOptions, os::windows::fs::OpenOptionsExt, time::Duration};
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("preferences.json");
        Preferences {
            language: Language::En,
        }
        .write(&path)
        .unwrap();
        let reader = OpenOptions::new()
            .read(true)
            .share_mode(0)
            .open(&path)
            .unwrap();
        let release = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(80));
            drop(reader);
        });
        let language = Preferences::read(&path).language;
        release.join().unwrap();
        assert_eq!(language, Language::En);
    }

    #[test]
    #[ignore = "explicit repeated-write diagnostic; can target an isolated OneDrive QA folder"]
    fn preference_write_stress_diagnostic() {
        let temporary = tempfile::tempdir().unwrap();
        let directory = std::env::var_os("OICHA_PREFERENCE_QA_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| temporary.path().to_owned());
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join("preferences.json");
        for iteration in 0..2_000 {
            let language = if iteration % 2 == 0 {
                Language::En
            } else {
                Language::Vi
            };
            let result = Preferences { language }.write(&path);
            if let Err(error) = result {
                panic!(
                    "preference write {iteration}: {error:#}; OS error {:?}",
                    error
                        .downcast_ref::<std::io::Error>()
                        .and_then(std::io::Error::raw_os_error)
                );
            }
            assert_eq!(
                Preferences::read(&path).language,
                language,
                "round-trip {iteration}"
            );
        }
    }
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
