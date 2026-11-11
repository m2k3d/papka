//! Файловый журнал. Вызывать только из фонового потока.
//! Изменение файлов пользователя и выполнение отмены сюда не входят.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};

pub const HISTORY_LIMIT: usize = 100;
pub const HISTORY_AGE_SECONDS: u64 = 30 * 24 * 60 * 60;
const MAX_FILE_BYTES: u64 = 16 * 1024 * 1024;
const MAX_CHECKPOINT_BYTES: usize = 64 * 1024;
const MAX_PENDING: usize = 64;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Action { Copy, Move, Rename, Trash, CreateFile, CreateDirectory }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ResultKind { Success, Partial, Failed, Interrupted }

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Finished {
    pub at: u64,
    pub result: ResultKind,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Record {
    pub id: u64,
    pub action: Action,
    pub started_at: u64,
    pub finished: Option<Finished>,
    /// Версионированные данные конкретной операции, включая данные восстановления.
    /// Байты позволяют сохранять пути ОС без потерь не-UTF8 имён.
    pub checkpoint: Vec<u8>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Document {
    version: u32,
    next_id: u64,
    records: Vec<Record>,
}

pub struct Journal {
    directory: PathBuf,
    document: Document,
    // Блокировка отдельного постоянного файла переживает замену history.json.
    _lock: File,
    writable: bool,
}

impl Journal {
    /// now — число секунд Unix. Не выполняет отмену или повтор незавершённых действий.
    pub fn open(directory: &Path, now: u64) -> io::Result<Self> {
        let mut builder = fs::DirBuilder::new();
        builder.recursive(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        builder.create(directory)?;
        let mut options = OpenOptions::new();
        options.read(true).write(true).create(true).truncate(false);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let lock = options.open(directory.join("history.lock"))?;
        lock.try_lock().map_err(|_| io::Error::new(io::ErrorKind::WouldBlock,
            "Could not lock the history; another instance of papka may be using it."))?;
        let document = match File::open(directory.join("history.json")) {
            Ok(file) => {
                let mut bytes = Vec::new();
                file.take(MAX_FILE_BYTES + 1).read_to_end(&mut bytes)?;
                if bytes.len() as u64 > MAX_FILE_BYTES { return Err(invalid("History exceeds the size limit.")); }
                serde_json::from_slice(&bytes).map_err(|_| invalid("History format is corrupted or unknown."))?
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => Document {
                version: 1, next_id: 1, records: Vec::new(),
            },
            Err(error) => return Err(error),
        };
        validate(&document)?;
        let mut journal = Self { directory: directory.into(), document, _lock: lock, writable: true };
        let mut next = journal.document.clone();
        prune(&mut next, now);
        if next.records.len() != journal.document.records.len() {
            journal.commit(next)?;
        }
        Ok(journal)
    }

    pub fn records(&self) -> &[Record] { &self.document.records }

    pub fn retain_recent(&mut self, now: u64) -> io::Result<()> {
        let mut next = self.document.clone();
        prune(&mut next, now);
        if next.records.len() != self.document.records.len() { self.commit(next)?; }
        Ok(())
    }

    /// Сначала надёжно записать намерение, затем выполнять файловое действие.
    pub fn begin(&mut self, action: Action, checkpoint: Vec<u8>, now: u64) -> io::Result<u64> {
        if self.document.records.iter().filter(|record| record.finished.is_none()).count() >= MAX_PENDING {
            return Err(invalid("Too many incomplete records; review the history."));
        }
        let mut next = self.document.clone();
        let id = next.next_id;
        next.next_id = id.checked_add(1).ok_or_else(|| invalid("History IDs exhausted."))?;
        next.records.push(Record { id, action, started_at: now, finished: None, checkpoint });
        prune(&mut next, now);
        self.commit(next)?;
        Ok(id)
    }

    /// Обновляет данные частичного выполнения, не объявляя действие завершённым.
    pub fn checkpoint(&mut self, id: u64, checkpoint: Vec<u8>) -> io::Result<()> {
        let mut next = self.document.clone();
        pending(&mut next, id)?.checkpoint = checkpoint;
        self.commit(next)
    }

    pub fn finish(&mut self, id: u64, result: ResultKind, checkpoint: Vec<u8>, now: u64) -> io::Result<()> {
        let mut next = self.document.clone();
        let record = pending(&mut next, id)?;
        record.checkpoint = checkpoint;
        record.finished = Some(Finished { at: now.max(record.started_at), result });
        prune(&mut next, now);
        self.commit(next)
    }

    /// Сохраняет ход отмены, не меняя время и итог исходного действия.
    pub fn update_record(&mut self, id: u64, checkpoint: Vec<u8>) -> io::Result<()> {
        let mut next = self.document.clone();
        let record = next.records.iter_mut().find(|record| record.id == id)
            .ok_or_else(|| invalid("History record not found."))?;
        record.checkpoint = checkpoint;
        self.commit(next)
    }

    fn commit(&mut self, next: Document) -> io::Result<()> {
        if !self.writable { return Err(invalid("Record state is unconfirmed; reopen the history.")); }
        validate(&next)?;
        let bytes = serde_json::to_vec(&next).map_err(|_| invalid("Could not serialize the history."))?;
        if bytes.len() as u64 > MAX_FILE_BYTES { return Err(invalid("History exceeds the size limit.")); }
        let mut temporary = tempfile::NamedTempFile::new_in(&self.directory)?;
        temporary.write_all(&bytes)?;
        temporary.as_file().sync_all()?;
        temporary.persist(self.directory.join("history.json")).map_err(|error| error.error)?;
        // Если синхронизация каталога не удалась, файл уже мог измениться.
        // Запретить дальнейшие записи из устаревшего состояния в памяти.
        self.writable = false;
        #[cfg(unix)]
        File::open(&self.directory)?.sync_all()?;
        self.document = next;
        self.writable = true;
        Ok(())
    }
}

fn pending(document: &mut Document, id: u64) -> io::Result<&mut Record> {
    document.records.iter_mut().find(|record| record.id == id && record.finished.is_none())
        .ok_or_else(|| invalid("Incomplete history record not found."))
}

fn prune(document: &mut Document, now: u64) {
    document.records.retain(|record| record.finished.as_ref()
        .is_none_or(|finished| now.saturating_sub(finished.at) < HISTORY_AGE_SECONDS));
    let mut completed: Vec<_> = document.records.iter().filter_map(|record|
        record.finished.as_ref().map(|finished| (finished.at, record.id))).collect();
    completed.sort_unstable();
    let expired: std::collections::BTreeSet<_> = completed.iter()
        .take(completed.len().saturating_sub(HISTORY_LIMIT)).map(|(_, id)| *id).collect();
    document.records.retain(|record| !expired.contains(&record.id));
}

fn validate(document: &Document) -> io::Result<()> {
    if document.version != 1 || document.next_id == 0 {
        return Err(invalid("History version or ID is not supported."));
    }
    let mut ids = std::collections::BTreeSet::new();
    for record in &document.records {
        if record.id == 0 || record.id >= document.next_id || !ids.insert(record.id)
            || record.checkpoint.len() > MAX_CHECKPOINT_BYTES
            || record.finished.as_ref().is_some_and(|finished| finished.at < record.started_at) {
            return Err(invalid("Invalid history record; the file was not changed."));
        }
    }
    Ok(())
}

fn invalid(message: &str) -> io::Error { io::Error::new(io::ErrorKind::InvalidData, message) }
