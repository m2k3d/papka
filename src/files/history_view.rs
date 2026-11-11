//! Сведения журнала для интерфейса и доступность отмены.
use super::checkpoint::Checkpoint;
use crate::history::{Action, Record, ResultKind};

#[derive(Debug, Clone)]
pub struct HistoryItem {
    pub id: u64,
    pub pending: bool,
    pub undo_available: bool,
    pub label: String,
    pub details: Vec<String>,
}
pub(super) fn history_items(records: &[Record]) -> Vec<HistoryItem> {
    records
        .iter()
        .map(|record| HistoryItem {
            id: record.id,
            pending: record.finished.is_none(),
            undo_available: can_undo(record),
            label: super::undo_label(record),
            details: super::details(record),
        })
        .collect()
}

pub fn undo_label(record: &Record) -> String {
    let name = match record.action {
        Action::Copy => "Copy",
        Action::Move => "Move",
        Action::Rename => "Rename",
        Action::Trash => "Trash",
        Action::CreateFile => "Create file",
        Action::CreateDirectory => "Create folder",
    };
    let state = match &record.finished {
        None => "incomplete, review manually",
        Some(f) => match f.result {
            ResultKind::Success => "completed",
            ResultKind::Partial => "partial",
            ResultKind::Failed => "failed",
            ResultKind::Interrupted => "interrupted",
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
                    "{} → {} · {} · items written: {}{}",
                    item.source.path().display(),
                    item.target.path().display(),
                    if item.undone {
                        "undone"
                    } else if item.done {
                        "done"
                    } else if item.started {
                        "needs review"
                    } else {
                        "not started"
                    },
                    item.copied_entries,
                    if cp.undoing {
                        " · undo incomplete"
                    } else {
                        ""
                    }
                );
                line.push_str(&format!(" · bytes written: {}", item.copied_bytes));
                if let Some(backup) = &item.backup {
                    line.push_str(&format!(" · backup path: {}", backup.path().display()));
                }
                if let Some(trash) = &item.undo_trash {
                    line.push_str(&format!(" · trash copy: {}", trash.path().display()));
                }
                line
            })
            .collect(),
        Err(_) => {
            vec!["Unknown recovery data; automatic undo is unavailable.".into()]
        }
    }
}
