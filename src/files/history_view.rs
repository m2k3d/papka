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
