//! Версионированные данные отмены и пути без потерь имён.
use crate::platform;
use serde::{Deserialize, Serialize};
use std::{
    io,
    path::{Path, PathBuf},
};

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
use crate::platform::FileStamp as Stamp;
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct Node {
    pub(super) path: SavedPath,
    pub(super) stamp: Stamp,
    pub(super) tree: u64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct Item {
    pub(super) source: SavedPath,
    pub(super) target: SavedPath,
    pub(super) nodes: Vec<Node>,
    pub(super) started: bool,
    pub(super) done: bool,
    pub(super) undone: bool,
    pub(super) trash_info: Option<SavedPath>,
    pub(super) backup: Option<SavedPath>,
    pub(super) backup_nodes: Vec<Node>,
    pub(super) undo_trash: Option<SavedPath>,
    pub(super) copied_entries: u64,
    pub(super) copied_bytes: u64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct Checkpoint {
    pub(super) version: u8,
    pub(super) items: Vec<Item>,
    pub(super) undoing: bool,
}
pub(super) fn encode(cp: &Checkpoint) -> io::Result<Vec<u8>> {
    let bytes = serde_json::to_vec(cp)?;
    if bytes.len() > 64 * 1024 {
        return Err(io::Error::other(
            "Undo data limit reached (64 KiB). Operation stopped.",
        ));
    }
    Ok(bytes)
}
