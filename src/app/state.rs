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

#[derive(Default)]
pub(crate) struct DragState {
    pub native: Option<crate::platform::NativeDnd>,
    pub held: bool,
    pub active: bool,
    pub self_hover: bool,
    pub generation: u64,
    pub preparing: Option<crate::operations::Ticket>,
    pub snapshot: Option<crate::files::DragSnapshot>,
    pub hover: Option<PathBuf>,
    pub incoming: Option<(u64, u64)>,
    pub outgoing_cleanup: Option<u64>,
    pub incoming_allowed: bool,
}
impl DragState {
    pub fn detach(&mut self) {
        if let Some(ticket) = self.preparing.take() {
            ticket.cancellation.request();
        }
        self.native = None;
        self.snapshot = None;
        self.active = false;
        self.held = false;
        self.hover = None;
        self.incoming_allowed = false;
        self.self_hover = false;
        self.incoming = None;
    }
}
