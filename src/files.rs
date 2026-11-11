//! Чтение каталогов и операции с журналом вне потока интерфейса.
use crate::{
    history::{Action, Journal, Record, ResultKind},
    operations::{Completion, Context, Outcome},
    platform,
};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
#[derive(Debug, Clone)]
pub struct Entry {
    pub path: PathBuf,
    pub name: String,
    sort_name: String,
    pub directory: bool,
    pub link: bool,
    pub hidden: bool,
    pub size: u64,
    pub modified: u64,
}
#[derive(Debug, Clone)]
pub enum Data {
    ListingBegin(PathBuf),
    ListingBatch(Vec<Entry>),
    ListingEnd(Option<String>),
    History(Vec<HistoryItem>),
    Settings(Settings),
    Conflict(Command),
    Warning(String),
}
#[derive(Debug, Clone)]
pub struct HistoryItem {
    pub id: u64,
    pub pending: bool,
    pub undo_available: bool,
    pub label: String,
    pub details: Vec<String>,
}
fn history_items(records: &[Record]) -> Vec<HistoryItem> {
    records
        .iter()
        .map(|record| HistoryItem {
            id: record.id,
            pending: record.finished.is_none(),
            undo_available: can_undo(record),
            label: undo_label(record),
            details: details(record),
        })
        .collect()
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub hidden: bool,
    pub descending: bool,
    pub sort: Sort,
    pub dark: bool,
    pub palette: [u32; 3],
    pub palette_count: u8,
    pub reduced_motion: bool,
    pub quick_access: Vec<PathBuf>,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            hidden: false,
            descending: false,
            sort: Sort::Name,
            dark: false,
            palette: [0xc7c2df, 0xe7c4b5, 0xb8cddd],
            palette_count: 3,
            reduced_motion: false,
            quick_access: default_quick_access(),
        }
    }
}
fn default_quick_access() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    for path in [
        dirs::home_dir(),
        dirs::desktop_dir(),
        dirs::document_dir(),
        dirs::download_dir(),
        dirs::audio_dir(),
        dirs::picture_dir(),
        dirs::video_dir(),
    ]
    .into_iter()
    .flatten()
    {
        if !paths.contains(&path) {
            paths.push(path);
        }
    }
    paths
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Sort {
    Name,
    Size,
    Modified,
}
impl std::fmt::Display for Sort {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Name => "Имя",
            Self::Size => "Размер",
            Self::Modified => "Изменён",
        })
    }
}
pub fn settings_load() -> io::Result<Settings> {
    let path = platform::AppDirectories::resolve()
        .map_err(io::Error::other)?
        .config
        .join("settings.json");
    match fs::read(path) {
        Ok(bytes) => serde_json::from_slice(&bytes).map_err(io::Error::other),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Settings::default()),
        Err(e) => Err(e),
    }
}
pub fn settings_save(settings: &Settings) -> io::Result<()> {
    let path = platform::AppDirectories::resolve()
        .map_err(io::Error::other)?
        .config;
    fs::create_dir_all(&path)?;
    let mut file = tempfile::NamedTempFile::new_in(&path)?;
    serde_json::to_writer(&mut file, settings)?;
    file.as_file().sync_all()?;
    file.persist(path.join("settings.json"))
        .map_err(|e| e.error)?;
    File::open(path)?.sync_all()
}
pub fn listing(path: PathBuf, settings: Settings, context: &mut Context) -> io::Result<()> {
    let path = fs::canonicalize(path)?;
    let mut entries = Vec::new();
    let mut skipped = 0;
    for item in fs::read_dir(&path)? {
        if context.is_cancelled() {
            return Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "Чтение прервано",
            ));
        }
        let item = match item {
            Ok(item) => item,
            Err(_) => {
                skipped += 1;
                continue;
            }
        };
        let p = item.path();
        let metadata = match fs::symlink_metadata(&p) {
            Ok(m) => m,
            Err(_) => {
                skipped += 1;
                continue;
            }
        };
        let name = item.file_name().to_string_lossy().into_owned();
        entries.push(Entry {
            sort_name: name.to_lowercase(),
            hidden: name.starts_with('.'),
            name,
            directory: if metadata.is_symlink() {
                p.is_dir()
            } else {
                metadata.is_dir()
            },
            link: metadata.is_symlink(),
            size: metadata.len(),
            modified: metadata
                .modified()
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map_or(0, |t| t.as_secs()),
            path: p,
        });
        context.progress(entries.len() as u64, None);
    }
    if context.is_cancelled() {
        return Err(io::Error::new(
            io::ErrorKind::Interrupted,
            "Чтение прервано",
        ));
    }
    entries.sort_by(|a, b| {
        b.directory.cmp(&a.directory).then_with(|| {
            let order = match settings.sort {
                Sort::Name => a.sort_name.cmp(&b.sort_name),
                Sort::Size => a.size.cmp(&b.size),
                Sort::Modified => a.modified.cmp(&b.modified),
            }
            .then_with(|| a.path.cmp(&b.path));
            if settings.descending {
                order.reverse()
            } else {
                order
            }
        })
    });
    context.data(Data::ListingBegin(path));
    let mut entries = entries.into_iter();
    loop {
        if context.is_cancelled() {
            return Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "Чтение прервано",
            ));
        }
        let batch: Vec<_> = entries.by_ref().take(256).collect();
        if batch.is_empty() {
            break;
        }
        context.data(Data::ListingBatch(batch));
    }
    context.data(Data::ListingEnd(
        (skipped > 0).then(|| format!("Не удалось прочитать элементов: {skipped}")),
    ));
    Ok(())
}

// Пути Linux сохраняются без потерь не-UTF8 имён.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedPath(Vec<u8>);
impl SavedPath {
    pub fn new(path: &Path) -> Self {
        Self(platform::path_bytes(path))
    }
    pub fn path(&self) -> PathBuf {
        platform::path_from_bytes(self.0.clone())
    }
}
use platform::{FileStamp as Stamp, file_stamp as stamp};
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Node {
    path: SavedPath,
    stamp: Stamp,
    tree: u64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Item {
    source: SavedPath,
    target: SavedPath,
    nodes: Vec<Node>,
    started: bool,
    done: bool,
    undone: bool,
    trash_info: Option<SavedPath>,
    backup: Option<SavedPath>,
    backup_nodes: Vec<Node>,
    undo_trash: Option<SavedPath>,
    copied_entries: u64,
    copied_bytes: u64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Checkpoint {
    version: u8,
    items: Vec<Item>,
    undoing: bool,
}
fn encode(cp: &Checkpoint) -> io::Result<Vec<u8>> {
    let bytes = serde_json::to_vec(cp)?;
    if bytes.len() > 64 * 1024 {
        return Err(io::Error::other(
            "Достигнут предел данных отмены (64 КиБ). Операция остановлена.",
        ));
    }
    Ok(bytes)
}
// Отпечаток включает имена, inode, ctime и метаданные всего дерева.
// Содержимое ссылок не обходится. В журнале остаётся один отпечаток на корень.
fn tree_stamp(path: &Path, context: Option<&Context>, depth: usize) -> io::Result<u64> {
    use std::hash::{Hash, Hasher};
    if context.is_some_and(Context::is_cancelled) {
        return Err(io::Error::new(io::ErrorKind::Interrupted, "Прервано"));
    }
    if depth > 256 {
        return Err(io::Error::other("Слишком глубокое дерево каталогов."));
    }
    let before = stamp(path)?;
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    before.hash(&mut hasher);
    let metadata = fs::symlink_metadata(path)?;
    if metadata.is_symlink() {
        platform::path_bytes(&fs::read_link(path)?).hash(&mut hasher);
    } else if metadata.is_dir() {
        let mut children = fs::read_dir(path)?.collect::<io::Result<Vec<_>>>()?;
        children.sort_by_key(|e| e.file_name());
        for child in children {
            platform::path_bytes(Path::new(&child.file_name())).hash(&mut hasher);
            tree_stamp(&child.path(), context, depth + 1)?.hash(&mut hasher);
        }
    }
    if stamp(path)? != before {
        return Err(io::Error::other(
            "Данные менялись во время проверки. Повторите действие.",
        ));
    }
    Ok(hasher.finish())
}
fn collect(path: &Path, nodes: &mut Vec<Node>, context: &Context) -> io::Result<()> {
    let before = stamp(path)?;
    let tree = tree_stamp(path, Some(context), 0)?;
    if stamp(path)? != before {
        return Err(io::Error::other("Данные изменились во время проверки."));
    }
    nodes.push(Node {
        path: SavedPath::new(path),
        stamp: before,
        tree,
    });
    Ok(())
}
fn unchanged(nodes: &[Node], context: &Context) -> io::Result<()> {
    if nodes.is_empty() {
        return Err(io::Error::other(
            "Нет подтверждённых данных для безопасной отмены.",
        ));
    }
    for node in nodes {
        let path = node.path.path();
        if stamp(&path)? != node.stamp || tree_stamp(&path, Some(context), 0)? != node.tree {
            return Err(io::Error::other(
                "Данные изменились после операции. Отмена остановлена; файлы сохранены.",
            ));
        }
    }
    Ok(())
}

#[derive(Debug, Clone)]
pub enum Command {
    Create(PathBuf, bool),
    Copy(Vec<PathBuf>, PathBuf),
    Move(Vec<PathBuf>, PathBuf),
    Rename(PathBuf, PathBuf),
    Trash(Vec<PathBuf>),
    Undo(u64),
    Acknowledge(u64),
}
pub struct Engine {
    journal: Option<Journal>,
}
impl Engine {
    pub fn new() -> Self {
        Self { journal: None }
    }
    pub fn load(&mut self, ctx: &Context) -> io::Result<()> {
        if self.journal.is_none() {
            self.journal = Some(Journal::open(
                &platform::AppDirectories::resolve()
                    .map_err(io::Error::other)?
                    .history,
                now(),
            )?);
        }
        self.journal.as_mut().unwrap().retain_recent(now())?;
        ctx.data(Data::History(history_items(
            self.journal.as_ref().unwrap().records(),
        )));
        Ok(())
    }
    pub fn run(&mut self, command: Command, ctx: &mut Context) -> Completion {
        let requested = match &command {
            Command::Undo(id) | Command::Acknowledge(id) => Some(*id),
            _ => None,
        };
        let undo = matches!(command, Command::Undo(_));
        if let Err(error) = self.load(ctx) {
            return Completion {
                outcome: Outcome::Failed(error.to_string()),
                completed: None,
                remaining: None,
                undo_available: false,
            };
        }
        let before = self
            .journal
            .as_ref()
            .unwrap()
            .records()
            .iter()
            .map(|r| r.id)
            .max()
            .unwrap_or(0);
        let original = command.clone();
        let result = self.execute(command, ctx);
        let records = self.journal.as_ref().unwrap().records();
        let record = records
            .iter()
            .find(|r| requested == Some(r.id))
            .or_else(|| records.iter().rev().find(|r| r.id > before));
        let counts = record
            .and_then(|r| serde_json::from_slice::<Checkpoint>(&r.checkpoint).ok())
            .map(|cp| {
                let done = cp
                    .items
                    .iter()
                    .filter(|i| if undo { i.undone } else { i.done })
                    .count() as u64;
                let total = if undo {
                    cp.items
                        .iter()
                        .filter(|i| {
                            i.done || (i.started && fs::symlink_metadata(i.target.path()).is_ok())
                        })
                        .count() as u64
                } else {
                    cp.items.len() as u64
                };
                (done, total.saturating_sub(done))
            });
        let undo_available = record.is_some_and(can_undo);
        if result
            .as_ref()
            .is_err_and(|e| e.kind() == io::ErrorKind::AlreadyExists)
        {
            let done: Vec<_> = record
                .and_then(|r| serde_json::from_slice::<Checkpoint>(&r.checkpoint).ok())
                .map(|cp| {
                    cp.items
                        .into_iter()
                        .filter(|i| i.done)
                        .map(|i| i.source.path())
                        .collect()
                })
                .unwrap_or_default();
            let retry = match original {
                Command::Copy(paths, target) => Some(Command::Copy(
                    paths.into_iter().filter(|p| !done.contains(p)).collect(),
                    target,
                )),
                Command::Move(paths, target) => Some(Command::Move(
                    paths
                        .into_iter()
                        .filter(|p| !done.contains(p) && fs::symlink_metadata(p).is_ok())
                        .collect(),
                    target,
                )),
                command @ (Command::Create(..) | Command::Rename(..)) => Some(command),
                _ => None,
            };
            if let Some(command) = retry {
                ctx.data(Data::Conflict(command));
            }
        }
        ctx.data(Data::History(history_items(records)));
        Completion {
            outcome: match result {
                Ok(()) => Outcome::Success,
                Err(e) if e.kind() == io::ErrorKind::Interrupted => Outcome::Interrupted,
                Err(e) => Outcome::Failed(e.to_string()),
            },
            completed: counts.map(|c| c.0),
            remaining: counts.map(|c| c.1),
            undo_available,
        }
    }
    fn execute(&mut self, command: Command, ctx: &mut Context) -> io::Result<()> {
        if let Command::Undo(id) = command {
            return self.undo(id, ctx);
        }
        if let Command::Acknowledge(id) = command {
            let journal = self.journal.as_mut().unwrap();
            let record = journal
                .records()
                .iter()
                .find(|r| r.id == id && r.finished.is_none())
                .cloned()
                .ok_or_else(|| io::Error::other("Незавершённая запись не найдена."))?;
            return journal.finish(id, ResultKind::Interrupted, record.checkpoint, now());
        }
        let (action, pairs, create) = match command {
            Command::Create(path, directory) => (
                if directory {
                    Action::CreateDirectory
                } else {
                    Action::CreateFile
                },
                vec![(path.clone(), path)],
                Some(directory),
            ),
            Command::Copy(paths, directory) => (Action::Copy, targets(paths, directory)?, None),
            Command::Move(paths, directory) => (Action::Move, targets(paths, directory)?, None),
            Command::Rename(a, b) => (Action::Rename, vec![(a, b)], None),
            Command::Trash(paths) => {
                let mut pairs = Vec::new();
                for path in paths {
                    let target = platform::trash_target(&path)?;
                    pairs.push((path, target));
                }
                (Action::Trash, pairs, None)
            }
            Command::Undo(_) | Command::Acknowledge(_) => unreachable!(),
        };
        if pairs.is_empty() {
            return Err(io::Error::other("Ничего не выделено."));
        }
        if pairs.len() > 100 {
            return Err(io::Error::other(
                "Выберите не более 100 элементов за одну операцию.",
            ));
        }
        let mut cp = Checkpoint {
            version: 1,
            items: Vec::new(),
            undoing: false,
        };
        for (source, target) in pairs {
            if fs::symlink_metadata(&target).is_ok() {
                return Err(io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    format!(
                        "Уже существует: {}. Выберите другое имя или расположение.",
                        target.display()
                    ),
                ));
            }
            if create.is_none() {
                let metadata = fs::symlink_metadata(&source)?;
                let parent = target
                    .parent()
                    .ok_or_else(|| io::Error::other("Нет родительского каталога"))?;
                let canonical_parent = fs::canonicalize(parent)?;
                if metadata.is_dir() && canonical_parent.starts_with(fs::canonicalize(&source)?) {
                    return Err(io::Error::other("Нельзя поместить папку внутрь неё самой."));
                }
                let mut preview = Vec::new();
                collect(&source, &mut preview, ctx)?;
            }
            let trash_info = if matches!(action, Action::Trash) {
                Some(SavedPath::new(&platform::trash_info_path(&target)?))
            } else {
                None
            };
            cp.items.push(Item {
                source: SavedPath::new(&source),
                target: SavedPath::new(&target),
                nodes: Vec::new(),
                started: false,
                done: false,
                undone: false,
                trash_info,
                backup: None,
                backup_nodes: Vec::new(),
                undo_trash: None,
                copied_entries: 0,
                copied_bytes: 0,
            });
        }
        let journal = self.journal.as_mut().unwrap();
        let id = journal.begin(action.clone(), encode(&cp)?, now())?;
        let result = (|| {
            for index in 0..cp.items.len() {
                if ctx.is_cancelled() {
                    return Err(io::Error::new(
                        io::ErrorKind::Interrupted,
                        "Операция прервана; частичные результаты сохранены в истории.",
                    ));
                }
                let source = cp.items[index].source.path();
                let target = cp.items[index].target.path();
                cp.items[index].started = true;
                journal.checkpoint(id, encode(&cp)?)?;
                if let Some(directory) = create {
                    if directory {
                        fs::create_dir(&target)?;
                    } else {
                        OpenOptions::new()
                            .write(true)
                            .create_new(true)
                            .open(&target)?
                            .sync_all()?;
                    }
                } else if matches!(action, Action::Copy) {
                    copy_tree(
                        &source,
                        &target,
                        ctx,
                        &mut |count, bytes| {
                            cp.items[index].copied_entries += count;
                            cp.items[index].copied_bytes += bytes;
                            journal.checkpoint(id, encode(&cp)?)
                        },
                        0,
                    )?;
                } else {
                    if matches!(action, Action::Trash) {
                        platform::write_trash_info(
                            &source,
                            &cp.items[index].trash_info.as_ref().unwrap().path(),
                        )?;
                    }
                    match platform::rename_exclusive(&source, &target) {
                        Ok(()) => {}
                        Err(e)
                            if e.raw_os_error() == Some(libc::EXDEV)
                                && matches!(action, Action::Move) =>
                        {
                            let mut source_nodes = Vec::new();
                            collect(&source, &mut source_nodes, ctx)?;
                            copy_tree(
                                &source,
                                &target,
                                ctx,
                                &mut |count, bytes| {
                                    cp.items[index].copied_entries += count;
                                    cp.items[index].copied_bytes += bytes;
                                    journal.checkpoint(id, encode(&cp)?)
                                },
                                0,
                            )?;
                            unchanged(&source_nodes, ctx)?;
                            // Источник сохраняется в корзине своего тома для обратного переноса.
                            let backup = platform::trash_target(&source)?;
                            cp.items[index].backup = Some(SavedPath::new(&backup));
                            journal.checkpoint(id, encode(&cp)?)?;
                            platform::write_trash_info(
                                &source,
                                &platform::trash_info_path(&backup)?,
                            )?;
                            unchanged(&source_nodes, ctx)?;
                            platform::rename_exclusive(&source, &backup)?;
                            sync_parent(&source)?;
                            sync_parent(&backup)?;
                            let mut backup_nodes = Vec::new();
                            collect(&backup, &mut backup_nodes, ctx)?;
                            cp.items[index].backup_nodes = backup_nodes;
                        }
                        Err(e) => return Err(e),
                    }
                }
                sync_parent(&target)?;
                if create.is_none() && !matches!(action, Action::Copy) {
                    sync_parent(&source)?;
                }
                let mut nodes = Vec::new();
                collect(&target, &mut nodes, ctx)?;
                cp.items[index].nodes = nodes;
                cp.items[index].done = true;
                journal.checkpoint(id, encode(&cp)?)?;
                ctx.progress(index as u64 + 1, Some(cp.items.len() as u64));
            }
            Ok(())
        })();
        let kind = match &result {
            Ok(()) => ResultKind::Success,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => ResultKind::Interrupted,
            Err(_)
                if cp.items.iter().any(|i| {
                    i.done || (i.started && fs::symlink_metadata(i.target.path()).is_ok())
                }) =>
            {
                ResultKind::Partial
            }
            Err(_) => ResultKind::Failed,
        };
        journal.finish(id, kind, encode(&cp)?, now())?;
        result
    }
    fn undo(&mut self, id: u64, ctx: &mut Context) -> io::Result<()> {
        let journal = self.journal.as_mut().unwrap();
        let record = journal
            .records()
            .iter()
            .find(|r| r.id == id)
            .cloned()
            .ok_or_else(|| io::Error::other("Действие отсутствует в истории."))?;
        let mut cp: Checkpoint = serde_json::from_slice(&record.checkpoint)?;
        if cp.version != 1 || record.finished.is_none() || cp.undoing {
            return Err(io::Error::other(
                "Незавершённое действие требует ручной проверки. Автоматическое восстановление запрещено.",
            ));
        }
        if !cp.items.iter().any(|i| i.done && !i.undone) {
            return Err(io::Error::other("Нет доступных результатов для отмены."));
        }
        // Проверить все результаты до первого изменения.
        for item in cp.items.iter().filter(|i| i.done && !i.undone) {
            if item.nodes.is_empty() {
                return Err(io::Error::other(
                    "Нет подтверждённых данных для безопасной отмены.",
                ));
            }
            unchanged(&item.nodes, ctx)?;
            if item.backup.is_some() {
                unchanged(&item.backup_nodes, ctx)?;
            }
            if matches!(record.action, Action::Move | Action::Rename | Action::Trash)
                && fs::symlink_metadata(item.source.path()).is_ok()
            {
                return Err(io::Error::other(
                    "Исходное имя занято. Освободите его перед отменой.",
                ));
            }
        }
        cp.undoing = true;
        journal.update_record(id, encode(&cp)?)?;
        for index in (0..cp.items.len()).rev() {
            if !cp.items[index].done || cp.items[index].undone {
                continue;
            }
            if ctx.is_cancelled() {
                return Err(io::Error::new(
                    io::ErrorKind::Interrupted,
                    "Отмена прервана. Требуется ручная проверка истории.",
                ));
            }
            unchanged(&cp.items[index].nodes, ctx)?;
            let item = &cp.items[index];
            if matches!(
                record.action,
                Action::Copy | Action::CreateFile | Action::CreateDirectory
            ) || item.backup.is_some()
            {
                // Отмена копии сохраняет её в корзине, поэтому даже внешняя гонка
                // не приводит к безвозвратной потере появившихся данных.
                let target = item.target.path();
                let trash = platform::trash_target(&target)?;
                cp.items[index].undo_trash = Some(SavedPath::new(&trash));
                journal.update_record(id, encode(&cp)?)?;
                platform::write_trash_info(&target, &platform::trash_info_path(&trash)?)?;
                unchanged(&cp.items[index].nodes, ctx)?;
                platform::rename_exclusive(&target, &trash)?;
                sync_parent(&target)?;
                sync_parent(&trash)?;
                if let Some(backup) = &cp.items[index].backup {
                    unchanged(&cp.items[index].backup_nodes, ctx)?;
                    platform::rename_exclusive(&backup.path(), &cp.items[index].source.path())?;
                    fs::remove_file(platform::trash_info_path(&backup.path())?)?;
                    sync_parent(&backup.path())?;
                }
            } else {
                platform::rename_exclusive(&item.target.path(), &item.source.path())?;
                if let Some(info) = &item.trash_info {
                    fs::remove_file(info.path())?;
                }
            }
            sync_parent(&cp.items[index].target.path())?;
            if matches!(record.action, Action::Move | Action::Rename | Action::Trash) {
                sync_parent(&cp.items[index].source.path())?;
            }
            cp.items[index].undone = true;
            journal.update_record(id, encode(&cp)?)?;
        }
        cp.undoing = false;
        journal.update_record(id, encode(&cp)?)?;
        if cp.items.iter().any(|item| {
            !item.done && item.started && fs::symlink_metadata(item.target.path()).is_ok()
        }) {
            return Err(io::Error::other(
                "Подтверждённые результаты отменены. Незавершённые элементы требуют ручной проверки.",
            ));
        }
        Ok(())
    }
}
fn targets(paths: Vec<PathBuf>, directory: PathBuf) -> io::Result<Vec<(PathBuf, PathBuf)>> {
    if !directory.is_dir() {
        return Err(io::Error::other("Папка назначения недоступна."));
    }
    paths
        .into_iter()
        .map(|p| {
            let name = p
                .file_name()
                .ok_or_else(|| io::Error::other("Нельзя перемещать корень файловой системы."))?;
            let target = directory.join(name);
            Ok((p, target))
        })
        .collect()
}
fn copy_tree(
    source: &Path,
    target: &Path,
    ctx: &mut Context,
    checkpoint: &mut impl FnMut(u64, u64) -> io::Result<()>,
    depth: usize,
) -> io::Result<()> {
    if depth > 256 {
        return Err(io::Error::other("Слишком глубокое дерево каталогов."));
    }
    if ctx.is_cancelled() {
        return Err(io::Error::new(
            io::ErrorKind::Interrupted,
            "Копирование прервано; часть данных могла быть скопирована.",
        ));
    }
    let before = stamp(source)?;
    let metadata = fs::symlink_metadata(source)?;
    if metadata.is_symlink() {
        platform::copy_symlink(source, target)?;
    } else if metadata.is_dir() {
        fs::create_dir(target)?;
        sync_parent(target)?;
        for item in fs::read_dir(source)? {
            let item = item?;
            copy_tree(
                &item.path(),
                &target.join(item.file_name()),
                ctx,
                checkpoint,
                depth + 1,
            )?;
        }
        fs::set_permissions(target, metadata.permissions())?;
        File::open(target)?.sync_all()?;
    } else if metadata.is_file() {
        let mut input = File::open(source)?;
        let mut output = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(target)?;
        sync_parent(target)?;
        let mut buffer = vec![0; 256 * 1024];
        let mut pending_bytes = 0;
        loop {
            if ctx.is_cancelled() {
                return Err(io::Error::new(
                    io::ErrorKind::Interrupted,
                    "Копирование прервано; частичный файл сохранён.",
                ));
            }
            let n = input.read(&mut buffer)?;
            if n == 0 {
                break;
            }
            output.write_all(&buffer[..n])?;
            pending_bytes += n as u64;
            if pending_bytes >= 8 * 1024 * 1024 {
                output.sync_data()?;
                checkpoint(0, pending_bytes)?;
                ctx.copied(pending_bytes);
                pending_bytes = 0;
            }
        }
        output.sync_all()?;
        checkpoint(0, pending_bytes)?;
        ctx.copied(pending_bytes);
        output.set_permissions(metadata.permissions())?;
        output.sync_all()?;
    } else {
        return Err(io::Error::other(
            "Копирование специальных файлов не поддерживается.",
        ));
    }
    sync_parent(target)?;
    checkpoint(1, 0)?;
    if stamp(source)? != before {
        return Err(io::Error::other(
            "Источник изменился во время копирования. Копия требует проверки.",
        ));
    }
    Ok(())
}
pub fn undo_label(record: &Record) -> String {
    let name = match record.action {
        Action::Copy => "Копирование",
        Action::Move => "Перемещение",
        Action::Rename => "Переименование",
        Action::Trash => "Корзина",
        Action::CreateFile => "Создание файла",
        Action::CreateDirectory => "Создание папки",
    };
    let state = match &record.finished {
        None => "не завершено, проверьте вручную",
        Some(f) => match f.result {
            ResultKind::Success => "завершено",
            ResultKind::Partial => "частично",
            ResultKind::Failed => "ошибка",
            ResultKind::Interrupted => "прервано",
        },
    };
    format!("#{}, {} — {}", record.id, name, state)
}
pub fn can_undo(record: &Record) -> bool {
    record.finished.is_some()
        && serde_json::from_slice::<Checkpoint>(&record.checkpoint).is_ok_and(|cp| {
            cp.version == 1
                && !cp.undoing
                && cp
                    .items
                    .iter()
                    .any(|i| i.done && !i.undone && !i.nodes.is_empty())
        })
}

pub fn details(record: &Record) -> Vec<String> {
    match serde_json::from_slice::<Checkpoint>(&record.checkpoint) {
        Ok(cp) => cp
            .items
            .iter()
            .map(|item| {
                let mut line = format!(
                    "{} → {} · {} · записано элементов: {}{}",
                    item.source.path().display(),
                    item.target.path().display(),
                    if item.undone {
                        "отменено"
                    } else if item.done {
                        "готово"
                    } else if item.started {
                        "требует проверки"
                    } else {
                        "не начато"
                    },
                    item.copied_entries,
                    if cp.undoing {
                        " · отмена не завершена"
                    } else {
                        ""
                    }
                );
                line.push_str(&format!(" · записано байт: {}", item.copied_bytes));
                if let Some(backup) = &item.backup {
                    line.push_str(&format!(" · резервный путь: {}", backup.path().display()));
                }
                if let Some(trash) = &item.undo_trash {
                    line.push_str(&format!(" · копия в корзине: {}", trash.path().display()));
                }
                line
            })
            .collect(),
        Err(_) => {
            vec!["Неизвестные данные восстановления; автоматическая отмена недоступна.".into()]
        }
    }
}

fn sync_parent(path: &Path) -> io::Result<()> {
    File::open(
        path.parent()
            .ok_or_else(|| io::Error::other("Нет родительского каталога."))?,
    )?
    .sync_all()
}
