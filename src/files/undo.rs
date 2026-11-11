//! Проверка и отмена подтверждённых результатов с сохранением хода отмены.
use super::{
    Engine, SavedPath,
    checkpoint::{Checkpoint, encode},
    verification::unchanged,
};
use crate::{
    history::{Action, Record},
    operations::Context,
    platform::{self, sync_parent},
};
use std::{fs, io};

impl Engine {
    pub(super) fn undo(&mut self, id: u64, ctx: &mut Context) -> io::Result<()> {
        let journal = self.journal.as_mut().unwrap();
        let record = journal
            .records()
            .iter()
            .find(|r| r.id == id)
            .cloned()
            .ok_or_else(|| io::Error::other("Действие отсутствует в истории."))?;
        let mut cp: Checkpoint = serde_json::from_slice(&record.checkpoint)?;
        verify_undo(&record, &cp, ctx)?;
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

/// Проверить все подтверждённые результаты до первой записи начала отмены.
fn verify_undo(record: &Record, cp: &Checkpoint, ctx: &Context) -> io::Result<()> {
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

    Ok(())
}
