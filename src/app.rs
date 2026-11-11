//! Состояние приложения; правила действий разделены по тематическим модулям.
mod commands;
mod dialogs;
mod events;
mod navigation;
mod presentation;
mod selection;
mod settings;
mod state;

use crate::{
    files::{Command, Entry, HistoryItem, Settings},
    operations::{Background, Ticket},
};
use state::{NavigationState, SelectionGestures, WorkState};
use std::{collections::BTreeSet, path::PathBuf};

pub struct App {
    navigation: NavigationState,
    gestures: SelectionGestures,
    work: WorkState,
    // Поля представления доступны существующим тестовым фикстурам.
    // Рабочий интерфейс изменяет их только через методы приложения.
    pub background: Option<Background>,
    pub error: Option<String>,
    pub status: String,
    pub operation_notice: Option<String>,
    pub directory: PathBuf,
    pub path_input: String,
    pub path_dirty: bool,
    pub entries: Vec<Entry>,
    pub selection: BTreeSet<PathBuf>,
    pub settings: Settings,
    pub history: Vec<HistoryItem>,
    pub expanded_history: Option<u64>,
    pub sidebar_context: bool,
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
    pub page: usize,
    pub transition: Option<std::time::Instant>,
    pub last_sidebar_scroll: Option<std::time::Instant>,
    pub last_file_scroll: Option<std::time::Instant>,
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
            navigation: NavigationState::default(),
            gestures: SelectionGestures::default(),
            work: WorkState::default(),
            background: None,
            error: None,
            status: "Запуск…".into(),
            operation_notice: None,
            path_input: directory.to_string_lossy().into_owned(),
            path_dirty: false,
            directory,
            entries: vec![],
            selection: BTreeSet::new(),
            settings: Settings::default(),
            history: vec![],
            expanded_history: None,
            sidebar_context: false,
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
            page: 0,
            transition: None,
            last_sidebar_scroll: None,
            last_file_scroll: None,
            clipboard: None,
        }
    }
}

impl Drop for App {
    fn drop(&mut self) {
        if let Some(background) = &self.background {
            background.shutdown();
        }
    }
}

pub fn completion(result: std::io::Result<()>) -> crate::operations::Completion {
    use crate::operations::{Completion, Outcome};
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
