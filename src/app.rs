//! Состояние приложения; файловый ввод-вывод выполняется рабочими потоками.
use crate::{
    files::{self, Command, Data, Engine, Entry, HistoryItem, Settings},
    operations::{Background, Completion, Event, Outcome, Ticket},
};
use std::{
    collections::BTreeSet,
    path::PathBuf,
    sync::{Arc, Mutex},
};

pub struct App {
    pub background: Option<Background>,
    pub error: Option<String>,
    pub status: String,
    pub operation_notice: Option<String>,
    pub directory: PathBuf,
    pub path_input: String,
    pub path_dirty: bool,
    read_error: bool,
    pub entries: Vec<Entry>,
    pub selection: BTreeSet<PathBuf>,
    pub settings: Settings,
    pub history: Vec<HistoryItem>,
    pub expanded_history: Option<u64>,
    pub context_menu: Option<(Option<PathBuf>, iced::Point)>,
    pub pointer: iced::Point,
    pub show_path: bool,
    pub show_settings: bool,
    pub show_palette: bool,
    pub palette_inputs: [String; 3],
    pub show_history: bool,
    pub dialog: Option<Dialog>,
    pub conflict: Option<Command>,
    retry: Option<Command>,
    pub input: String,
    pub loading: Option<u64>,
    pub mutation: Option<Ticket>,
    pub closing: bool,
    pub selection_control: bool,
    pub selection_shift: bool,
    back: Vec<PathBuf>,
    forward: Vec<PathBuf>,
    requested: Option<PathBuf>,
    pending_listing: Option<(u64, PathBuf, Vec<Entry>)>,
    engine: Arc<Mutex<Engine>>,
    anchor: Option<PathBuf>,
    cursor: Option<PathBuf>,
    pub page: usize,
    auxiliary: BTreeSet<u64>,
    history_direction: i8,
    pub transition: Option<std::time::Instant>,
    pub last_file_scroll: Option<std::time::Instant>,
    last_click: Option<(PathBuf, std::time::Instant)>,
    pub clipboard: Option<(Vec<PathBuf>, bool)>,
}
#[derive(Debug, Clone)]
pub enum Dialog {
    Create(bool),
    Rename(PathBuf),
    Transfer(bool),
}
impl Default for App {
    fn default() -> Self {
        let directory = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"));
        Self {
            background: None,
            error: None,
            status: "Запуск…".into(),
            operation_notice: None,
            path_input: directory.to_string_lossy().into_owned(),
            path_dirty: false,
            read_error: false,
            directory,
            entries: vec![],
            selection: BTreeSet::new(),
            settings: Settings::default(),
            history: vec![],
            expanded_history: None,
            context_menu: None,
            pointer: iced::Point::ORIGIN,
            show_path: false,
            show_settings: false,
            show_palette: false,
            palette_inputs: Default::default(),
            show_history: false,
            dialog: None,
            conflict: None,
            retry: None,
            input: String::new(),
            loading: None,
            mutation: None,
            closing: false,
            selection_control: false,
            selection_shift: false,
            back: vec![],
            forward: vec![],
            requested: None,
            pending_listing: None,
            engine: Arc::new(Mutex::new(Engine::new())),
            anchor: None,
            cursor: None,
            page: 0,
            auxiliary: BTreeSet::new(),
            history_direction: 0,
            transition: None,
            last_file_scroll: None,
            last_click: None,
            clipboard: None,
        }
    }
}
pub fn completion(result: std::io::Result<()>) -> Completion {
    Completion {
        outcome: match result {
            Ok(()) => Outcome::Success,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => Outcome::Interrupted,
            Err(e) => Outcome::Failed(e.to_string()),
        },
        completed: None,
        remaining: None,
        undo_available: false,
    }
}
impl App {
    pub fn ready(&mut self, background: Background) {
        self.background = Some(background.clone());
        let engine = self.engine.clone();
        match background.submit(move |ctx| {
            match files::settings_load() {
                Ok(settings) => ctx.data(Data::Settings(settings)),
                Err(e) => ctx.data(Data::Warning(format!(
                    "Не удалось загрузить настройки: {e}"
                ))),
            }
            completion(engine.lock().unwrap_or_else(|e| e.into_inner()).load(ctx))
        }) {
            Ok(t) => self.mutation = Some(t),
            Err(e) => self.error = Some(e.to_string()),
        }
        self.refresh();
    }
    pub fn refresh(&mut self) {
        self.read(self.directory.clone());
    }
    fn read(&mut self, path: PathBuf) {
        let Some(background) = &self.background else {
            return;
        };
        let request = path.clone();
        let settings = self.settings.clone();
        match background.read_latest(move |ctx| {
            completion(files::listing(request.clone(), settings, ctx).map_err(|e| {
                std::io::Error::new(
                    e.kind(),
                    format!("Не удалось прочитать {}: {e}", request.display()),
                )
            }))
        }) {
            Ok(ticket) => {
                self.pending_listing = None;
                self.loading = Some(ticket.id);
                self.requested = Some(path);
            }
            Err(e) => self.error = Some(e.to_string()),
        }
    }
    pub fn navigate(&mut self, path: PathBuf) {
        if self.dialog.is_some() || self.closing {
            return;
        }
        let path = if path.starts_with("~") {
            dirs::home_dir()
                .map(|home| home.join(path.strip_prefix("~").unwrap()))
                .unwrap_or(path)
        } else {
            path
        };
        self.path_dirty = false;
        let path = if path.is_absolute() {
            path
        } else {
            self.directory.join(path)
        };
        self.history_direction = 0;
        self.read(path);
    }
    pub fn back(&mut self) {
        if self.loading.is_some() || self.dialog.is_some() || self.closing {
            return;
        }
        if let Some(path) = self.back.last().cloned() {
            self.read_history(path);
            self.history_direction = -1;
        }
    }
    pub fn forward(&mut self) {
        if self.loading.is_some() || self.dialog.is_some() || self.closing {
            return;
        }
        if let Some(path) = self.forward.last().cloned() {
            self.read_history(path);
            self.history_direction = 1;
        }
    }
    fn read_history(&mut self, path: PathBuf) {
        self.read(path);
        self.requested = None;
    }
    pub fn can_back(&self) -> bool {
        !self.back.is_empty() && self.loading.is_none()
    }
    pub fn can_forward(&self) -> bool {
        !self.forward.is_empty() && self.loading.is_none()
    }
    pub fn apply(&mut self, event: Event) {
        match event {
            Event::Data {
                id,
                data: Data::ListingBegin(path),
            } if self.loading == Some(id) => {
                self.pending_listing = Some((id, path, Vec::new()));
            }
            Event::Data {
                id,
                data: Data::ListingBatch(entries),
            } if self.loading == Some(id) => {
                if let Some((pending_id, _, pending)) = &mut self.pending_listing {
                    if *pending_id == id {
                        pending.extend(entries);
                    }
                }
            }
            Event::Data {
                id,
                data: Data::ListingEnd(warning),
            } if self.loading == Some(id) => {
                if let Some((_, path, entries)) = self.pending_listing.take() {
                    self.commit_listing(path, entries, warning);
                }
            }
            Event::Data {
                data: Data::Warning(warning),
                ..
            } => self.error = Some(warning),
            Event::Data {
                data: Data::Conflict(command),
                ..
            } => self.conflict = Some(command),
            Event::Data {
                data: Data::History(records),
                ..
            } => self.history = records,
            Event::Data {
                data: Data::Settings(settings),
                ..
            } => {
                self.settings = settings;
                self.refresh();
            }
            Event::Data { .. } | Event::Started(_) => {}
            Event::Progress {
                id,
                completed,
                total,
            } => {
                if self.mutation.as_ref().is_some_and(|t| t.id == id) {
                    self.status = total.map_or_else(
                        || format!("Записано: {:.1} МиБ", completed as f64 / 1_048_576.),
                        |total| format!("Обработано: {completed} / {total}"),
                    );
                }
            }
            Event::Finished { id, completion } => {
                let mutation = self.mutation.as_ref().is_some_and(|t| t.id == id);
                let read = self.loading == Some(id);
                let auxiliary = self.auxiliary.remove(&id);
                if !read && !mutation && !auxiliary {
                    return;
                }
                if read {
                    self.loading = None;
                    self.pending_listing = None;
                    self.history_direction = 0;
                }
                if mutation {
                    self.mutation = None;
                    if !self.closing {
                        self.refresh();
                    }
                }
                let counts = completion
                    .completed
                    .zip(completion.remaining)
                    .map(|(done, left)| format!("Обработано: {done}; осталось: {left}. "))
                    .unwrap_or_default();
                match completion.outcome {
                    Outcome::Failed(error) => {
                        if read {
                            self.read_error = true;
                            self.path_dirty = false;
                            self.path_input = self.directory.to_string_lossy().into_owned();
                        }
                        self.error = Some(error);
                        self.status =
                            format!("{counts}Операция не завершена полностью. Проверьте историю.");
                    }
                    Outcome::Interrupted if mutation => {
                        self.status = format!("{counts}Прерывание завершено. Проверьте историю.")
                    }
                    Outcome::Success if mutation => self.status = format!("{counts}Готово"),
                    Outcome::Success if read && self.read_error => {
                        self.error = None;
                        self.read_error = false;
                    }
                    _ => {}
                }
                if mutation {
                    self.operation_notice = Some(self.status.clone());
                }
            }
        }
    }
    fn commit_listing(&mut self, path: PathBuf, entries: Vec<Entry>, warning: Option<String>) {
        if path != self.directory {
            if self.history_direction < 0 {
                self.back.pop();
                self.forward.push(self.directory.clone());
            } else if self.history_direction > 0 {
                self.forward.pop();
                self.back.push(self.directory.clone());
            } else if self.requested.is_some() {
                self.back.push(self.directory.clone());
                self.forward.clear();
            }
            self.directory = path;
            self.path_dirty = false;
            if !self.settings.reduced_motion {
                self.transition = Some(std::time::Instant::now());
            }
            self.selection.clear();
            self.anchor = None;
            self.cursor = None;
            self.page = 0;
        }
        if !self.path_dirty {
            self.path_input = self.directory.to_string_lossy().into_owned();
        }
        self.entries = entries;
        let present: std::collections::HashSet<_> = self.entries.iter().map(|e| &e.path).collect();
        self.selection.retain(|p| present.contains(p));
        self.status = warning.unwrap_or_else(|| format!("Элементов: {}", self.entries.len()));
    }
    pub fn visible(&self) -> Vec<&Entry> {
        self.entries
            .iter()
            .filter(|e| self.settings.hidden || !e.hidden)
            .collect()
    }
    /// Правый клик сохраняет множественное выделение и не считается двойным щелчком.
    pub fn select_context(&mut self, path: PathBuf) {
        self.last_click = None;
        if !self.selection.contains(&path) {
            self.selection.clear();
            self.selection.insert(path.clone());
        }
        self.anchor = Some(path.clone());
        self.cursor = Some(path);
    }
    pub fn select(&mut self, path: PathBuf) {
        if self.dialog.is_some() {
            return;
        }
        let now = std::time::Instant::now();
        if !self.selection_control
            && !self.selection_shift
            && self
                .last_click
                .as_ref()
                .is_some_and(|(p, time)| *p == path && now.duration_since(*time).as_millis() < 350)
        {
            self.last_click = None;
            self.open(path);
            return;
        }
        self.last_click = Some((path.clone(), now));
        let visible = self.visible();
        let index = visible.iter().position(|e| e.path == path);
        if self.selection_shift {
            let anchor = self
                .anchor
                .as_ref()
                .and_then(|path| visible.iter().position(|e| &e.path == path));
            if let (Some(a), Some(b)) = (anchor, index) {
                let paths: Vec<_> = visible
                    .iter()
                    .skip(a.min(b))
                    .take(a.max(b) - a.min(b) + 1)
                    .map(|e| e.path.clone())
                    .collect();
                self.cursor = Some(path);
                self.selection.extend(paths);
                return;
            }
        }
        self.anchor = Some(path.clone());
        self.cursor = Some(path.clone());
        if self.selection_control {
            if !self.selection.remove(&path) {
                self.selection.insert(path);
            }
        } else {
            self.selection.clear();
            self.selection.insert(path);
        }
    }
    pub fn step_selection(&mut self, down: bool) {
        if self.dialog.is_some() {
            return;
        }
        let visible = self.visible();
        if visible.is_empty() {
            return;
        }
        let position = self
            .cursor
            .as_ref()
            .and_then(|path| visible.iter().position(|e| &e.path == path));
        let index = position.map_or(0, |i| {
            if down {
                (i + 1).min(visible.len() - 1)
            } else {
                i.saturating_sub(1)
            }
        });
        let path = visible[index].path.clone();
        self.last_click = None;
        self.select(path);
        self.last_click = None;
        self.page = index / 200;
    }
    pub fn can_close(&self) -> bool {
        self.mutation.is_none() && self.auxiliary.is_empty()
    }
    pub fn selected(&self) -> Vec<PathBuf> {
        self.selection.iter().cloned().collect()
    }
    pub fn open(&mut self, path: PathBuf) {
        if self.entries.iter().any(|e| e.path == path && e.directory) {
            self.navigate(path);
        } else if let Some(background) = &self.background {
            if self.mutation.is_some() {
                return;
            }
            match background.submit(move |_| completion(crate::platform::open_file(&path))) {
                Ok(t) => self.mutation = Some(t),
                Err(e) => self.error = Some(e.to_string()),
            }
        }
    }
    pub fn execute(&mut self, command: Command) {
        if self.closing {
            return;
        }
        if self.conflict.is_some() {
            self.error = Some("Сначала выберите действие для конфликта имён.".into());
            return;
        }
        if self.mutation.is_some() {
            self.error = Some("Дождитесь завершения текущей операции.".into());
            return;
        }
        let Some(background) = &self.background else {
            return;
        };
        let engine = self.engine.clone();
        match background.submit(move |ctx| {
            engine
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .run(command, ctx)
        }) {
            Ok(ticket) => {
                self.mutation = Some(ticket);
                self.error = None;
                self.conflict = None;
                self.operation_notice = None;
                self.status = "Выполняется…".into();
            }
            Err(e) => self.error = Some(e.to_string()),
        }
    }
    pub fn save_settings(&mut self) {
        self.page = 0;
        self.anchor = None;
        self.refresh();
        let Some(background) = &self.background else {
            return;
        };
        let settings = self.settings.clone();
        match background.submit(move |_| completion(files::settings_save(&settings))) {
            Ok(ticket) => {
                self.auxiliary.insert(ticket.id);
            }
            Err(e) => self.error = Some(e.to_string()),
        }
    }
    pub fn begin_dialog(&mut self, dialog: Dialog) {
        self.retry = None;
        if self.conflict.is_some() {
            return;
        }
        if self.mutation.is_some() {
            return;
        }
        self.input = match &dialog {
            Dialog::Rename(path) => path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned(),
            Dialog::Transfer(_) => self.directory.to_string_lossy().into_owned(),
            _ => String::new(),
        };
        self.dialog = Some(dialog);
    }
    pub fn resolve_conflict(&mut self) {
        let Some(command) = self.conflict.take() else {
            return;
        };
        let (dialog, input) = match &command {
            Command::Create(path, directory) => (
                Dialog::Create(*directory),
                path.file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned(),
            ),
            Command::Rename(source, target) => (
                Dialog::Rename(source.clone()),
                target
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned(),
            ),
            Command::Copy(_, target) => (
                Dialog::Transfer(false),
                target.to_string_lossy().into_owned(),
            ),
            Command::Move(_, target) => (
                Dialog::Transfer(true),
                target.to_string_lossy().into_owned(),
            ),
            _ => return,
        };
        self.dialog = Some(dialog);
        self.input = input;
        self.retry = Some(command);
    }
    pub fn confirm(&mut self) {
        let Some(dialog) = self.dialog.clone() else {
            return;
        };
        let name = self.input.as_str();
        let command = match dialog {
            Dialog::Create(directory) => {
                if !valid_name(name) {
                    self.error = Some("Введите имя без /, отличное от . и ..".into());
                    return;
                }
                Command::Create(
                    if let Some(Command::Create(path, _)) = &self.retry {
                        path.with_file_name(name)
                    } else {
                        self.directory.join(name)
                    },
                    directory,
                )
            }
            Dialog::Rename(path) => {
                if !valid_name(name) {
                    self.error = Some("Введите имя без /, отличное от . и ..".into());
                    return;
                }
                Command::Rename(path.clone(), path.with_file_name(name))
            }
            Dialog::Transfer(moving) => {
                if name.is_empty() {
                    self.error = Some("Введите папку назначения.".into());
                    return;
                }
                let target = PathBuf::from(name);
                let target = if target.is_absolute() {
                    target
                } else {
                    self.directory.join(target)
                };
                let paths = match &self.retry {
                    Some(Command::Copy(paths, _)) | Some(Command::Move(paths, _)) => paths.clone(),
                    _ => self.selected(),
                };
                if moving {
                    Command::Move(paths, target)
                } else {
                    Command::Copy(paths, target)
                }
            }
        };
        self.dialog = None;
        self.execute(command);
    }
}
fn valid_name(name: &str) -> bool {
    !name.is_empty() && name != "." && name != ".." && !name.contains('/') && !name.contains('\0')
}
impl Drop for App {
    fn drop(&mut self) {
        if let Some(background) = &self.background {
            background.shutdown();
        }
    }
}
