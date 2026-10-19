//! Startup opens a chosen/remembered library; it never recreates that file.
use anyhow::{Context, Result};
use oicha_core::{document::Manuscript, storage::Store};
use std::{
    io::ErrorKind,
    path::{Path, PathBuf},
};

fn open_existing(path: PathBuf) -> Result<(Store, Manuscript, bool)> {
    // Keep the OS error kind and path available to the startup UI, rather
    // than hiding NotFound inside the database backend's error wrapper.
    std::fs::metadata(&path)
        .with_context(|| format!("Cannot access library {}", path.display()))?;
    Store::open(path)
}

pub fn open_startup(
    explicit: Option<PathBuf>,
    directory: &Path,
) -> Result<(Store, Manuscript, bool)> {
    if let Some(path) = explicit {
        return open_existing(path);
    }
    match std::fs::read_to_string(directory.join("last-library")) {
        Ok(path) => {
            anyhow::ensure!(
                !path.is_empty(),
                "The remembered library path is empty. Choose a library to open."
            );
            open_existing(PathBuf::from(path))
        }
        Err(error) if error.kind() == ErrorKind::NotFound => {
            // Only a fresh profile without a remembered path bootstraps a
            // default library. Permission errors and deleted remembered files
            // must not be mistaken for first launch.
            let path = directory.join("Library.oicha");
            if path.try_exists()? {
                Store::open(path)
            } else {
                Store::create(path).map(|store| (store, Manuscript::new(), false))
            }
        }
        Err(error) => Err(error).context("Cannot read the remembered library path"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oicha_core::storage::atomic_write;

    #[test]
    fn deleted_remembered_library_is_not_recreated_or_replaced_by_default() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("Tieu-thuyet2.oicha");
        drop(Store::create(path.clone()).unwrap());
        atomic_write(
            &directory.path().join("last-library"),
            path.to_str().unwrap().as_bytes(),
        )
        .unwrap();
        std::fs::remove_file(&path).unwrap();
        for _ in 0..2 {
            let error = open_startup(None, directory.path()).err().unwrap();
            assert!(error.chain().any(|cause| {
                cause
                    .downcast_ref::<std::io::Error>()
                    .is_some_and(|e| e.kind() == ErrorKind::NotFound)
            }));
            assert!(!path.exists());
            assert!(!directory.path().join("Library.oicha").exists());
        }
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 1);
    }

    #[test]
    fn deleted_default_library_is_not_recreated_when_it_was_remembered() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("Library.oicha");
        atomic_write(
            &directory.path().join("last-library"),
            path.to_str().unwrap().as_bytes(),
        )
        .unwrap();
        assert!(open_startup(None, directory.path()).is_err());
        assert!(!path.exists());
    }

    #[test]
    fn missing_explicit_path_does_not_fall_back_to_another_library() {
        let directory = tempfile::tempdir().unwrap();
        let remembered = directory.path().join("existing.oicha");
        drop(Store::create(remembered.clone()).unwrap());
        atomic_write(
            &directory.path().join("last-library"),
            remembered.to_str().unwrap().as_bytes(),
        )
        .unwrap();
        let missing = directory
            .path()
            .join("deleted-folder")
            .join("deleted.oicha");
        assert!(open_startup(Some(missing.clone()), directory.path()).is_err());
        assert!(!missing.parent().unwrap().exists());
        assert!(open_startup(None, directory.path()).is_ok());
    }

    #[test]
    fn first_launch_creates_default_but_existing_default_is_preserved() {
        let directory = tempfile::tempdir().unwrap();
        let (mut store, _, _) = open_startup(None, directory.path()).unwrap();
        assert!(store.projects().unwrap().is_empty());
        let mut book = Manuscript::demo();
        book.title = "Preserved default".into();
        store.save(&book).unwrap();
        drop(store);
        let (_, reopened, _) = open_startup(None, directory.path()).unwrap();
        assert_eq!(reopened.title, book.title);
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 1);
    }

    #[test]
    fn explicit_path_takes_precedence_and_invalid_preference_creates_nothing() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("chosen.oicha");
        drop(Store::create(path.clone()).unwrap());
        std::fs::write(directory.path().join("last-library"), [0xff]).unwrap();
        assert!(open_startup(Some(path), directory.path()).is_ok());
        assert!(open_startup(None, directory.path()).is_err());
        std::fs::write(directory.path().join("last-library"), "").unwrap();
        assert!(open_startup(None, directory.path()).is_err());
        assert!(!directory.path().join("Library.oicha").exists());
    }
}
