//! Atomic, conflict-aware persistence. A project lock prevents two Oicha writers.
use crate::document::Manuscript;
use anyhow::{Context, Result, bail, ensure};
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};
use tempfile::NamedTempFile;

const MAX_FILE_BYTES: u64 = 64 * 1024 * 1024;

pub fn sibling(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_os_string();
    name.push(suffix);
    PathBuf::from(name)
}

pub fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    fs::create_dir_all(parent).with_context(|| format!("Cannot create {}", parent.display()))?;
    let mut temp = NamedTempFile::new_in(parent).context("Cannot create temporary file")?;
    temp.write_all(bytes)
        .context("Cannot write file (disk full or permission denied)")?;
    temp.as_file()
        .sync_all()
        .context("Cannot flush file to disk")?;
    temp.persist(path)
        .map_err(|e| e.error)
        .with_context(|| format!("Cannot replace {}", path.display()))?;
    #[cfg(unix)]
    File::open(parent)?.sync_all()?;
    Ok(())
}

fn read_limited(path: &Path) -> Result<Vec<u8>> {
    let file = File::open(path).with_context(|| format!("Cannot open {}", path.display()))?;
    ensure!(
        file.metadata()?.len() <= MAX_FILE_BYTES,
        "Manuscript exceeds 64 MiB safety limit"
    );
    let mut bytes = Vec::new();
    file.take(MAX_FILE_BYTES + 1).read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() as u64 <= MAX_FILE_BYTES,
        "Manuscript exceeds 64 MiB safety limit"
    );
    Ok(bytes)
}

fn fingerprint(bytes: &[u8]) -> String {
    blake3::hash(bytes).to_hex().to_string()
}

#[derive(Serialize, Deserialize)]
struct Recovery {
    base: Option<String>,
    manuscript: Manuscript,
}

pub struct Store {
    pub path: PathBuf,
    _lock: File,
    fingerprint: Option<String>,
    recovery_warning: Option<String>,
    recovery_blocked: bool,
}

impl Store {
    pub fn create(path: PathBuf) -> Result<Self> {
        ensure!(
            !path.exists(),
            "Destination already exists. Choose another filename."
        );
        Self::lock(path, None)
    }

    fn lock(path: PathBuf, fingerprint: Option<String>) -> Result<Self> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let lock = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(sibling(&path, ".lock"))?;
        lock.try_lock_exclusive()
            .context("This manuscript is already open in another Oicha window")?;
        Ok(Self {
            path,
            _lock: lock,
            fingerprint,
            recovery_warning: None,
            recovery_blocked: false,
        })
    }

    pub fn open(path: PathBuf) -> Result<(Self, Manuscript, bool)> {
        let mut store = Self::lock(path, None)?;
        let bytes = read_limited(&store.path)?;
        let book: Manuscript = serde_json::from_slice(&bytes).context(
            "Invalid manuscript. Original file has been preserved; check the .bak file.",
        )?;
        book.validate()?;
        store.fingerprint = Some(fingerprint(&bytes));
        let recovery_path = sibling(&store.path, ".recovery");
        if recovery_path.exists() {
            let recovery = (|| -> Result<Recovery> {
                let recovery: Recovery = serde_json::from_slice(&read_limited(&recovery_path)?)?;
                recovery.manuscript.validate()?;
                Ok(recovery)
            })();
            let recovery = match recovery {
                Ok(recovery) => recovery,
                Err(error) => {
                    let archive = Self::archive_invalid_recovery(&recovery_path);
                    store.recovery_warning = Some(match archive {
                        Ok(path) => format!(
                            "File phục hồi không hợp lệ ({error:#}). Đã mở bản thảo chính; file phụ được giữ tại {}.",
                            path.display()
                        ),
                        Err(archive_error) => {
                            store.recovery_blocked = true;
                            format!(
                                "Đã mở bản thảo chính. File phục hồi không hợp lệ ({error:#}) và chưa thể lưu riêng ({archive_error:#}). Dùng Lưu thành… để giữ bản chỉnh sửa."
                            )
                        }
                    });
                    return Ok((store, book, false));
                }
            };
            if recovery.base == store.fingerprint && recovery.manuscript.revision > book.revision {
                return Ok((store, recovery.manuscript, true));
            }
            if recovery.base != store.fingerprint && recovery.manuscript.revision > book.revision {
                bail!(
                    "Recovery conflicts with the manuscript on disk. Both files are preserved. Open the .recovery file with the recovery command to save a separate copy."
                );
            }
        }
        Ok((store, book, false))
    }

    fn archive_invalid_recovery(path: &Path) -> Result<PathBuf> {
        let parent = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        // Reserve a unique path first; rename overwrites only our empty reservation.
        let archive = tempfile::Builder::new()
            .prefix("oicha-recovery-corrupt-")
            .suffix(".json")
            .tempfile_in(parent)?
            .into_temp_path()
            .keep()?;
        if let Err(error) = fs::rename(path, &archive) {
            let _ = fs::remove_file(&archive);
            return Err(error.into());
        }
        Ok(archive)
    }

    pub fn recovery_warning(&self) -> Option<&str> {
        self.recovery_warning.as_deref()
    }

    fn writable(&self) -> Result<()> {
        ensure!(
            !self.recovery_blocked,
            "Invalid recovery cannot be archived. Use Save As; the original files have been preserved."
        );
        Ok(())
    }

    pub fn recover_copy(path: &Path) -> Result<Manuscript> {
        let recovery: Recovery = serde_json::from_slice(&read_limited(path)?)?;
        recovery.manuscript.validate()?;
        Ok(recovery.manuscript)
    }

    fn check_conflict(&self) -> Result<Option<Vec<u8>>> {
        let bytes = if self.path.exists() {
            Some(read_limited(&self.path)?)
        } else {
            None
        };
        let current = bytes.as_deref().map(fingerprint);
        ensure!(
            current == self.fingerprint,
            "File changed outside Oicha. Use Save As to preserve your work in a separate file."
        );
        Ok(bytes)
    }

    pub fn save(&mut self, manuscript: &Manuscript) -> Result<()> {
        self.writable()?;
        manuscript.validate()?;
        let bytes = serde_json::to_vec_pretty(manuscript)?;
        ensure!(
            bytes.len() as u64 <= MAX_FILE_BYTES,
            "Manuscript exceeds 64 MiB safety limit"
        );
        if let Some(previous) = self.check_conflict()? {
            atomic_write(&sibling(&self.path, ".bak"), &previous)?;
        }
        self.check_conflict()?;
        atomic_write(&self.path, &bytes)?;
        self.fingerprint = Some(fingerprint(&bytes));
        // Stale recovery is safe to leave if removal is prevented by the OS.
        let _ = fs::remove_file(sibling(&self.path, ".recovery"));
        Ok(())
    }

    pub fn checkpoint(&self, manuscript: &Manuscript) -> Result<()> {
        self.writable()?;
        manuscript.validate()?;
        let recovery = Recovery {
            base: self.fingerprint.clone(),
            manuscript: manuscript.clone(),
        };
        let bytes = serde_json::to_vec(&recovery)?;
        ensure!(
            bytes.len() as u64 <= MAX_FILE_BYTES,
            "Recovery exceeds 64 MiB safety limit"
        );
        atomic_write(&sibling(&self.path, ".recovery"), &bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn save_backup_lock_and_conflict() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("novel.oicha");
        let mut store = Store::create(path.clone()).unwrap();
        let mut book = Manuscript::demo();
        store.save(&book).unwrap();
        assert!(Store::open(path.clone()).is_err());
        book.title = "Changed".into();
        book.revision += 1;
        store.save(&book).unwrap();
        let backup: Manuscript =
            serde_json::from_slice(&fs::read(sibling(&path, ".bak")).unwrap()).unwrap();
        assert_ne!(backup.title, book.title);
        fs::write(&path, "external edit").unwrap();
        assert!(store.save(&book).is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), "external edit");
    }
    #[test]
    fn checkpoint_recovers_unsaved_revision() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("novel.oicha");
        let mut store = Store::create(path.clone()).unwrap();
        let mut book = Manuscript::new();
        store.save(&book).unwrap();
        book.revision += 1;
        book.title = "Recovered".into();
        store.checkpoint(&book).unwrap();
        drop(store);
        let (mut store, restored, recovered) = Store::open(path).unwrap();
        assert!(recovered);
        assert_eq!(restored.title, "Recovered");
        store.save(&restored).unwrap();
        assert!(!sibling(&store.path, ".recovery").exists());
    }
    #[test]
    fn malformed_and_future_files_are_never_overwritten() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("bad.oicha");
        let mut book = Manuscript::new();
        book.version = 900;
        fs::write(&path, serde_json::to_vec(&book).unwrap()).unwrap();
        assert!(Store::open(path.clone()).is_err());
        assert!(path.exists());
    }
    #[test]
    fn atomic_replacement_never_leaves_trailing_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("output.txt");
        atomic_write(&path, b"a long document").unwrap();
        atomic_write(&path, b"short").unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"short");
    }

    #[test]
    fn invalid_recovery_is_archived_without_blocking_valid_primary() {
        for contents in [b"truncated".as_slice(), br#"{"base":null,"manuscript":{}}"#] {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("novel.oicha");
            let mut store = Store::create(path.clone()).unwrap();
            let mut book = Manuscript::demo();
            store.save(&book).unwrap();
            drop(store);
            fs::write(sibling(&path, ".recovery"), contents).unwrap();
            let (mut store, restored, recovered) = Store::open(path.clone()).unwrap();
            assert!(!recovered);
            assert_eq!(restored.title, book.title);
            assert!(store.recovery_warning().is_some());
            let archive = fs::read_dir(dir.path())
                .unwrap()
                .map(|entry| entry.unwrap().path())
                .find(|path| {
                    path.file_name()
                        .unwrap()
                        .to_string_lossy()
                        .starts_with("oicha-recovery-corrupt-")
                })
                .unwrap();
            assert_eq!(fs::read(&archive).unwrap(), contents);
            book.revision += 1;
            store.checkpoint(&book).unwrap();
            store.save(&book).unwrap();
            assert_eq!(fs::read(&archive).unwrap(), contents);
        }
    }

    #[test]
    fn oversized_recovery_is_preserved_and_unarchivable_recovery_prevents_overwrite() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("novel.oicha");
        let mut store = Store::create(path.clone()).unwrap();
        let book = Manuscript::new();
        store.save(&book).unwrap();
        drop(store);
        let recovery = sibling(&path, ".recovery");
        File::create(&recovery)
            .unwrap()
            .set_len(MAX_FILE_BYTES + 1)
            .unwrap();
        let (store, _, _) = Store::open(path.clone()).unwrap();
        assert!(store.recovery_warning().is_some());
        assert!(!recovery.exists());
        drop(store);
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            fs::write(&recovery, "corrupt recovery").unwrap();
            let locked = OpenOptions::new()
                .read(true)
                .write(true)
                .share_mode(0)
                .open(&recovery)
                .unwrap();
            let primary = fs::read(&path).unwrap();
            let (mut store, _, _) = Store::open(path.clone()).unwrap();
            assert!(store.recovery_warning().is_some());
            assert!(store.save(&book).is_err());
            assert!(store.checkpoint(&book).is_err());
            assert_eq!(fs::read(&path).unwrap(), primary);
            drop(locked);
            assert_eq!(fs::read_to_string(&recovery).unwrap(), "corrupt recovery");
        }
    }
}
