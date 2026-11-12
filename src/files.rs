//! Файловые действия: общий API, данные фоновых событий и тематические модули.
#[path = "files/checkpoint.rs"]
mod checkpoint;
#[path = "files/copy.rs"]
mod copy;
#[path = "files/engine.rs"]
mod engine;
#[path = "files/execute.rs"]
mod execute;
#[path = "files/history_view.rs"]
mod history_view;
#[path = "files/listing.rs"]
mod listing;
#[path = "files/prepare.rs"]
mod prepare;
#[path = "files/settings.rs"]
mod settings;
#[path = "files/transfer.rs"]
mod transfer;
#[path = "files/undo.rs"]
mod undo;
#[path = "files/verification.rs"]
mod verification;

pub use checkpoint::SavedPath;
pub use engine::{Command, Engine};
pub use history_view::{HistoryItem, can_undo, details, undo_label};
pub use listing::{Entry, listing};
pub use settings::{
    PaletteHarmony, Settings, Sort, circle_position, hsl_color, settings_load, settings_save,
};

use std::{
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
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
