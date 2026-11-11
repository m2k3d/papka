//! Внутренние данные переходов, жестов выделения и фонового выполнения.
use crate::files::{Engine, Entry};
use std::{
    collections::BTreeSet,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Instant,
};

#[derive(Default)]
pub(super) struct NavigationState {
    pub(super) read_error: bool,
    pub(super) back: Vec<PathBuf>,
    pub(super) forward: Vec<PathBuf>,
    pub(super) requested: Option<PathBuf>,
    pub(super) pending_listing: Option<PendingListing>,
    pub(super) history_direction: HistoryDirection,
}

/// Направление подтверждается только после успешного получения списка каталога.
#[derive(Default, Clone, Copy, PartialEq, Eq)]
pub(super) enum HistoryDirection {
    #[default]
    Direct,
    Back,
    Forward,
}

pub(super) struct PendingListing {
    pub(super) id: u64,
    pub(super) path: PathBuf,
    pub(super) entries: Vec<Entry>,
}

#[derive(Default)]
pub(super) struct SelectionGestures {
    pub(super) anchor: Option<PathBuf>,
    pub(super) cursor: Option<PathBuf>,
    pub(super) last_click: Option<(PathBuf, Instant)>,
}

pub(super) struct WorkState {
    pub(super) engine: Arc<Mutex<Engine>>,
    pub(super) auxiliary: BTreeSet<u64>,
}

impl Default for WorkState {
    fn default() -> Self {
        Self {
            engine: Arc::new(Mutex::new(Engine::new())),
            auxiliary: BTreeSet::new(),
        }
    }
}
