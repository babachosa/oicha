//! A library is one transactional file. Projects and crash checkpoints live
//! inside it; redb locks the file itself and commits with immediate durability.
use crate::document::Manuscript;
use anyhow::{Context, Result, ensure};
use redb::{
    Database, ReadableDatabase, ReadableTable, ReadableTableMetadata, TableDefinition,
    WriteTransaction,
};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
use tempfile::NamedTempFile;

const SCHEMA: u64 = 2;
const MAX_PROJECT_BYTES: usize = 64 * 1024 * 1024;
const MAX_PROJECTS: u64 = 10_000;
const META: TableDefinition<&str, u64> = TableDefinition::new("oicha.library");
const BOOKS: TableDefinition<u64, &[u8]> = TableDefinition::new("projects");
const INDEX: TableDefinition<u64, &[u8]> = TableDefinition::new("project_index");
const CHECKPOINTS: TableDefinition<u64, &[u8]> = TableDefinition::new("checkpoints");

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
    fs::File::open(parent)?.sync_all()?;
    Ok(())
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProjectSummary {
    pub id: u64,
    pub title: String,
    pub author: String,
    pub chapters: usize,
    pub words: usize,
    pub updated: u64,
}

#[derive(Serialize, Deserialize)]
struct Checkpoint {
    base_revision: u64,
    manuscript: Manuscript,
}

pub struct Store {
    pub path: PathBuf,
    db: Database,
    selected: Option<u64>,
}

fn encode(book: &Manuscript) -> Result<Vec<u8>> {
    book.validate()?;
    let bytes = serde_json::to_vec(book)?;
    ensure!(
        bytes.len() <= MAX_PROJECT_BYTES,
        "Project exceeds 64 MiB safety limit"
    );
    Ok(bytes)
}

fn decode(bytes: &[u8]) -> Result<Manuscript> {
    ensure!(
        bytes.len() <= MAX_PROJECT_BYTES,
        "Project exceeds 64 MiB safety limit"
    );
    let book: Manuscript = serde_json::from_slice(bytes)?;
    book.validate()?;
    Ok(book)
}

fn put(tx: &WriteTransaction, id: u64, book: &Manuscript) -> Result<()> {
    let bytes = encode(book)?;
    let summary = ProjectSummary {
        id,
        title: book.title.clone(),
        author: book.author.clone(),
        chapters: book.chapters.len(),
        words: book.words(),
        updated: SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs(),
    };
    tx.open_table(BOOKS)?.insert(id, bytes.as_slice())?;
    tx.open_table(INDEX)?
        .insert(id, serde_json::to_vec(&summary)?.as_slice())?;
    tx.open_table(CHECKPOINTS)?.remove(id)?;
    tx.open_table(META)?.insert("active", id)?;
    Ok(())
}

impl Store {
    pub fn create(path: PathBuf) -> Result<Self> {
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            fs::create_dir_all(parent)?;
        }
        // create_new is essential: neither Save As nor a concurrent creator can
        // truncate a pre-existing library (including the unsupported old format).
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(&path)
            .context(
                "Cannot create library. Choose a new filename; existing files are preserved.",
            )?;
        let db = Database::builder()
            .set_cache_size(32 * 1024 * 1024)
            .create_file(file)?;
        let tx = db.begin_write()?;
        {
            let mut meta = tx.open_table(META)?;
            meta.insert("schema", SCHEMA)?;
            meta.insert("next_id", 1)?;
            tx.open_table(BOOKS)?;
            tx.open_table(INDEX)?;
            tx.open_table(CHECKPOINTS)?;
        }
        tx.commit()?;
        Ok(Self {
            path,
            db,
            selected: None,
        })
    }

    pub fn open(path: PathBuf) -> Result<(Self, Manuscript, bool)> {
        let db = Database::builder().set_cache_size(32 * 1024 * 1024).open(&path)
            .context("Cannot open library. It may be open in another Oicha window, damaged, or use the old format. The file is preserved.")?;
        let tx = db.begin_read()?;
        let meta = tx.open_table(META)?;
        ensure!(
            meta.get("schema")?.map(|v| v.value()) == Some(SCHEMA),
            "Unsupported Oicha library format; file preserved"
        );
        let books = tx.open_table(BOOKS)?;
        ensure!(
            books.len()? <= MAX_PROJECTS,
            "Too many projects in this library"
        );
        let selected = meta.get("active")?.map(|v| v.value());
        ensure!(
            selected.is_none_or(|id| books.get(id).is_ok_and(|v| v.is_some())),
            "Invalid active project"
        );
        ensure!(
            selected.is_some() || books.is_empty()?,
            "Missing active project"
        );
        drop(books);
        drop(meta);
        drop(tx);
        let mut store = Self {
            path,
            db,
            selected: None,
        };
        let (book, recovered) = if let Some(id) = selected {
            store.select_project(id)?
        } else {
            (Manuscript::new(), false)
        };
        Ok((store, book, recovered))
    }

    pub fn selected_project(&self) -> Option<u64> {
        self.selected
    }

    pub fn projects(&self) -> Result<Vec<ProjectSummary>> {
        let tx = self.db.begin_read()?;
        let index = tx.open_table(INDEX)?;
        ensure!(
            index.len()? <= MAX_PROJECTS,
            "Too many projects in this library"
        );
        let mut projects = Vec::new();
        for entry in index.iter()? {
            let (id, bytes) = entry?;
            let summary: ProjectSummary = serde_json::from_slice(bytes.value())?;
            ensure!(summary.id == id.value(), "Invalid project index");
            projects.push(summary);
        }
        projects.sort_by_key(|p| (std::cmp::Reverse(p.updated), std::cmp::Reverse(p.id)));
        Ok(projects)
    }

    pub fn select_project(&mut self, id: u64) -> Result<(Manuscript, bool)> {
        let tx = self.db.begin_read()?;
        let books = tx.open_table(BOOKS)?;
        let bytes = books.get(id)?.context("Project no longer exists")?;
        let mut book = decode(bytes.value())?;
        let mut recovered = false;
        if let Some(bytes) = tx.open_table(CHECKPOINTS)?.get(id)? {
            ensure!(
                bytes.value().len() <= MAX_PROJECT_BYTES + 1024,
                "Checkpoint exceeds safety limit"
            );
            let checkpoint: Checkpoint = serde_json::from_slice(bytes.value())?;
            checkpoint.manuscript.validate()?;
            ensure!(
                checkpoint.base_revision == book.revision,
                "Checkpoint conflicts with the saved project; preserved inside library"
            );
            if checkpoint.manuscript.revision > book.revision {
                book = checkpoint.manuscript;
                recovered = true;
            }
        }
        self.selected = Some(id);
        Ok((book, recovered))
    }

    pub fn new_project(&mut self, book: &Manuscript) -> Result<u64> {
        encode(book)?;
        let tx = self.db.begin_write()?;
        ensure!(
            tx.open_table(BOOKS)?.len()? < MAX_PROJECTS,
            "Library project limit reached"
        );
        let id = tx
            .open_table(META)?
            .get("next_id")?
            .context("Missing next project ID")?
            .value();
        let next = id.checked_add(1).context("Project ID limit reached")?;
        ensure!(
            tx.open_table(BOOKS)?.get(id)?.is_none(),
            "Invalid next project ID; existing projects preserved"
        );
        put(&tx, id, book)?;
        tx.open_table(META)?.insert("next_id", next)?;
        tx.commit()?;
        self.selected = Some(id);
        Ok(id)
    }

    pub fn save(&mut self, book: &Manuscript) -> Result<()> {
        let Some(id) = self.selected else {
            self.new_project(book)?;
            return Ok(());
        };
        let tx = self.db.begin_write()?;
        ensure!(
            tx.open_table(BOOKS)?.get(id)?.is_some(),
            "Project no longer exists"
        );
        put(&tx, id, book)?;
        tx.commit()?;
        Ok(())
    }

    /// Delete the project, index entry and checkpoint in one durable commit.
    /// Clearing an active project never implicitly creates a replacement.
    pub fn delete_project(&mut self, id: u64) -> Result<()> {
        let tx = self.db.begin_write()?;
        let fallback = {
            let mut books = tx.open_table(BOOKS)?;
            ensure!(books.remove(id)?.is_some(), "Project no longer exists");
            books
                .iter()?
                .next()
                .transpose()?
                .map(|(key, _)| key.value())
        };
        tx.open_table(INDEX)?.remove(id)?;
        tx.open_table(CHECKPOINTS)?.remove(id)?;
        {
            let mut meta = tx.open_table(META)?;
            let active = meta.get("active")?.map(|v| v.value());
            if active == Some(id) {
                if let Some(next) = self
                    .selected
                    .filter(|selected| *selected != id)
                    .or(fallback)
                {
                    meta.insert("active", next)?;
                } else {
                    meta.remove("active")?;
                }
            }
        }
        tx.commit()?;
        if self.selected == Some(id) {
            self.selected = None;
        }
        Ok(())
    }

    pub fn checkpoint(&self, book: &Manuscript) -> Result<()> {
        encode(book)?;
        let Some(id) = self.selected else {
            return Ok(());
        };
        let tx = self.db.begin_write()?;
        let base_revision = {
            let table = tx.open_table(BOOKS)?;
            decode(table.get(id)?.context("Project no longer exists")?.value())?.revision
        };
        let bytes = serde_json::to_vec(&Checkpoint {
            base_revision,
            manuscript: book.clone(),
        })?;
        tx.open_table(CHECKPOINTS)?.insert(id, bytes.as_slice())?;
        tx.commit()?;
        Ok(())
    }

    /// Don't Save removes only the selected project's in-file checkpoint.
    pub fn discard_checkpoint(&self) -> Result<()> {
        if let Some(id) = self.selected {
            // Normal editing autosaves directly in a transaction. If no crash
            // checkpoint exists, Don't Save needs no disk write (also when a
            // previous save failed because the disk was full).
            let read = self.db.begin_read()?;
            if read.open_table(CHECKPOINTS)?.get(id)?.is_none() {
                return Ok(());
            }
            drop(read);
            let tx = self.db.begin_write()?;
            tx.open_table(CHECKPOINTS)?.remove(id)?;
            tx.commit()?;
        }
        Ok(())
    }

    pub fn copy_to(&self, path: PathBuf, current: Option<&Manuscript>) -> Result<Self> {
        let mut copy = Self::create(path)?;
        let read = self.db.begin_read()?;
        let write = copy.db.begin_write()?;
        for definition in [BOOKS, INDEX, CHECKPOINTS] {
            let source = read.open_table(definition)?;
            let mut target = write.open_table(definition)?;
            for entry in source.iter()? {
                let (key, value) = entry?;
                target.insert(key.value(), value.value())?;
            }
        }
        {
            let source = read.open_table(META)?;
            let mut target = write.open_table(META)?;
            for entry in source.iter()? {
                let (key, value) = entry?;
                target.insert(key.value(), value.value())?;
            }
        }
        if let (Some(id), Some(book)) = (self.selected, current) {
            put(&write, id, book)?;
        }
        write.commit()?;
        copy.selected = self.selected;
        Ok(copy)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn invalid_next_id_cannot_overwrite_an_existing_project() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("library.oicha");
        let mut store = Store::create(path.clone()).unwrap();
        let original = Manuscript::demo();
        let id = store.new_project(&original).unwrap();
        let tx = store.db.begin_write().unwrap();
        tx.open_table(META).unwrap().insert("next_id", id).unwrap();
        tx.commit().unwrap();
        assert!(store.new_project(&Manuscript::new()).is_err());
        assert_eq!(store.projects().unwrap().len(), 1);
        assert_eq!(store.selected_project(), Some(id));
        drop(store);
        assert_eq!(
            Store::open(path).unwrap().1.chapters[0].body,
            original.chapters[0].body
        );
    }
    #[test]
    fn deleting_an_inactive_project_preserves_the_current_one_and_removes_checkpoint() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("novels.oicha");
        let mut store = Store::create(path.clone()).unwrap();
        let book = Manuscript::demo();
        let a = store.new_project(&book).unwrap();
        let b = store.new_project(&Manuscript::new()).unwrap();
        let mut recovery = Manuscript::new();
        recovery.revision += 1;
        recovery.title = "Deleted recovery".into();
        store.checkpoint(&recovery).unwrap();
        store.select_project(a).unwrap();
        store.delete_project(b).unwrap();
        assert_eq!(store.selected_project(), Some(a));
        assert_eq!(store.projects().unwrap().len(), 1);
        assert!(store.select_project(b).is_err());
        let tx = store.db.begin_read().unwrap();
        assert!(
            tx.open_table(CHECKPOINTS)
                .unwrap()
                .get(b)
                .unwrap()
                .is_none()
        );
        assert!(tx.open_table(INDEX).unwrap().get(b).unwrap().is_none());
        drop(tx);
        drop(store);
        let (store, reopened, recovered) = Store::open(path).unwrap();
        assert_eq!(store.selected_project(), Some(a));
        assert!(!recovered);
        assert_eq!(reopened.chapters[0].body, book.chapters[0].body);
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
    }

    #[test]
    fn deleting_current_and_final_project_reopens_empty_and_never_reuses_ids() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("novels.oicha");
        let mut store = Store::create(path.clone()).unwrap();
        let a = store.new_project(&Manuscript::demo()).unwrap();
        let b = store.new_project(&Manuscript::new()).unwrap();
        store.delete_project(b).unwrap();
        assert_eq!(store.selected_project(), None);
        drop(store);
        let (mut store, _, _) = Store::open(path.clone()).unwrap();
        assert_eq!(store.selected_project(), Some(a));
        assert!(store.delete_project(u64::MAX).is_err());
        assert_eq!(store.selected_project(), Some(a));
        assert_eq!(store.projects().unwrap().len(), 1);
        store.delete_project(a).unwrap();
        drop(store);
        let (mut store, _, recovered) = Store::open(path).unwrap();
        assert!(!recovered);
        assert_eq!(store.selected_project(), None);
        assert!(store.projects().unwrap().is_empty());
        assert!(store.new_project(&Manuscript::new()).unwrap() > b);
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
    }
    #[test]
    fn crash_worker() {
        let Some(path) = std::env::var_os("OICHA_CRASH_FIXTURE") else {
            return;
        };
        let (mut store, mut book, _) = Store::open(PathBuf::from(path)).unwrap();
        book.title = "Committed before crash".into();
        book.revision += 1;
        if std::env::var_os("OICHA_CRASH_COMMIT").is_some() {
            store.save(&book).unwrap();
        } else {
            let tx = store.db.begin_write().unwrap();
            put(&tx, store.selected.unwrap(), &book).unwrap();
            // No commit and no Drop, like terminating the process mid-edit.
            std::process::exit(0);
        }
        std::process::exit(0);
    }

    #[test]
    fn abrupt_process_exit_keeps_transaction_boundary_and_one_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("crash.oicha");
        let mut store = Store::create(path.clone()).unwrap();
        let original = Manuscript::demo();
        store.save(&original).unwrap();
        drop(store);
        for commit in [false, true] {
            let mut command = std::process::Command::new(std::env::current_exe().unwrap());
            command
                .args(["--exact", "storage::tests::crash_worker", "--nocapture"])
                .env("OICHA_CRASH_FIXTURE", &path)
                .env_remove("OICHA_CRASH_COMMIT");
            if commit {
                command.env("OICHA_CRASH_COMMIT", "1");
            }
            assert!(command.output().unwrap().status.success());
            let (_, book, _) = Store::open(path.clone()).unwrap();
            assert_eq!(
                book.title,
                if commit {
                    "Committed before crash"
                } else {
                    &original.title
                }
            );
            assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
        }
    }
    #[test]
    fn multiple_projects_remain_independent_in_exactly_one_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("library.oicha");
        let mut store = Store::create(path.clone()).unwrap();
        assert!(store.projects().unwrap().is_empty());
        let mut first = Manuscript::demo();
        first.title = "Tiểu thuyết thứ nhất 🌿".into();
        let a = store.new_project(&first).unwrap();
        let mut second = Manuscript::new();
        second.title = "Second novel".into();
        let b = store.new_project(&second).unwrap();
        assert_ne!(a, b);
        let (mut first, _) = store.select_project(a).unwrap();
        first.chapters[0].body = crate::document::RichText::plain("Niềm tin");
        first.revision += 1;
        store.checkpoint(&first).unwrap();
        store.save(&first).unwrap();
        assert_eq!(store.select_project(b).unwrap().0.title, second.title);
        drop(store);
        let (mut store, _, _) = Store::open(path).unwrap();
        assert_eq!(store.projects().unwrap().len(), 2);
        assert_eq!(
            store.select_project(a).unwrap().0.chapters[0].body.text,
            "Niềm tin"
        );
        assert_eq!(store.select_project(b).unwrap().0.title, "Second novel");
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
    }
    #[test]
    fn checkpoint_and_discard_are_isolated_inside_library() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("library.oicha");
        let mut store = Store::create(path.clone()).unwrap();
        let original = Manuscript::new();
        let a = store.new_project(&original).unwrap();
        let b = store.new_project(&original).unwrap();
        let mut changed = original.clone();
        changed.title = "Recovered".into();
        changed.revision += 1;
        store.select_project(a).unwrap();
        store.checkpoint(&changed).unwrap();
        store.select_project(b).unwrap();
        store.checkpoint(&changed).unwrap();
        store.discard_checkpoint().unwrap();
        drop(store);
        let (mut store, _, _) = Store::open(path).unwrap();
        assert!(!store.select_project(b).unwrap().1);
        let (recovered, flag) = store.select_project(a).unwrap();
        assert!(flag);
        assert_eq!(recovered.title, "Recovered");
        store.save(&recovered).unwrap();
        drop(store);
        assert!(!Store::open(dir.path().join("library.oicha")).unwrap().2);
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
    }
    #[test]
    fn copy_keeps_all_projects_and_current_unsaved_edit() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = Store::create(dir.path().join("source.oicha")).unwrap();
        let a = store.new_project(&Manuscript::demo()).unwrap();
        let b = store.new_project(&Manuscript::new()).unwrap();
        let mut edit = store.select_project(a).unwrap().0;
        edit.title = "Unsaved title".into();
        edit.revision += 1;
        let mut copy = store
            .copy_to(dir.path().join("copy.oicha"), Some(&edit))
            .unwrap();
        assert_eq!(copy.projects().unwrap().len(), 2);
        assert_eq!(copy.select_project(a).unwrap().0.title, edit.title);
        assert_eq!(
            copy.select_project(b).unwrap().0.title,
            Manuscript::new().title
        );
        assert_ne!(store.select_project(a).unwrap().0.title, edit.title);
    }
    #[test]
    fn locks_existing_files_and_rejects_old_format_without_overwriting() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("library.oicha");
        let store = Store::create(path.clone()).unwrap();
        assert!(Store::open(path.clone()).is_err());
        assert!(Store::create(path.clone()).is_err());
        drop(store);
        assert!(Store::open(path).is_ok());
        for bytes in [
            b"malformed".to_vec(),
            serde_json::to_vec(&Manuscript::demo()).unwrap(),
        ] {
            let old = dir.path().join("old.oicha");
            fs::write(&old, &bytes).unwrap();
            assert!(Store::open(old.clone()).is_err());
            assert!(Store::create(old.clone()).is_err());
            assert_eq!(fs::read(old).unwrap(), bytes);
        }
    }
    #[test]
    fn failed_or_aborted_transaction_keeps_saved_project() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("library.oicha");
        let mut store = Store::create(path.clone()).unwrap();
        let original = Manuscript::demo();
        store.save(&original).unwrap();
        let mut bad = original.clone();
        bad.chapters.clear();
        assert!(store.save(&bad).is_err());
        {
            let tx = store.db.begin_write().unwrap();
            tx.open_table(BOOKS)
                .unwrap()
                .insert(store.selected.unwrap(), b"partial".as_slice())
                .unwrap();
        }
        drop(store);
        assert_eq!(Store::open(path).unwrap().1.title, original.title);
    }
    #[test]
    fn atomic_replacement_never_leaves_trailing_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("output.txt");
        atomic_write(&path, b"a long document").unwrap();
        atomic_write(&path, b"short").unwrap();
        assert_eq!(fs::read(path).unwrap(), b"short");
    }
}
