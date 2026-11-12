//! Проверка и отмена подтверждённых результатов с сохранением хода отмены.
use super::{
    Engine, SavedPath,
    checkpoint::{Checkpoint, encode},
    verification::unchanged,
};
use crate::{
    history::{Action, Record, ResultKind},
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
            .ok_or_else(|| io::Error::other("Action not found in history."))?;
        let mut cp: Checkpoint = serde_json::from_slice(&record.checkpoint)?;
        verify_undo(&record, &cp, ctx)?;
        let copy_undo = undo_as_copy(&record, &cp);
        cp.undoing = true;
        journal.update_record(id, encode(&cp)?)?;
        for index in (0..cp.items.len()).rev() {
            if !cp.items[index].done || cp.items[index].undone {
                continue;
            }
            if ctx.is_cancelled() {
                return Err(io::Error::new(
                    io::ErrorKind::Interrupted,
                    "Undo interrupted. Review the history manually.",
                ));
            }
            unchanged(&cp.items[index].nodes, ctx)?;
            let item = &cp.items[index];
            if copy_undo || item.backup.is_some() {
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
                match platform::rename_exclusive(&item.target.path(), &item.source.path()) {
                    Ok(()) => {}
                    Err(error)
                        if error.raw_os_error() == Some(libc::EXDEV)
                            && matches!(record.action, Action::Move) =>
                    {
                        undo_move_across_devices(journal, id, &mut cp, index, ctx)?;
                    }
                    Err(error) => return Err(error),
                }
                if let Some(info) = &cp.items[index].trash_info {
                    fs::remove_file(info.path())?;
                }
            }
            sync_parent(&cp.items[index].target.path())?;
            if !copy_undo && matches!(record.action, Action::Move | Action::Rename | Action::Trash)
            {
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
                "Confirmed results undone. Incomplete items require manual review.",
            ));
        }
        Ok(())
    }
}

/// Проверить все подтверждённые результаты до первой записи начала отмены.
fn verify_undo(record: &Record, cp: &Checkpoint, ctx: &Context) -> io::Result<()> {
    if cp.version != 1 || record.finished.is_none() || cp.undoing {
        return Err(io::Error::other(
            "Incomplete action requires manual review. Automatic recovery is disabled.",
        ));
    }
    if !cp.items.iter().any(|i| i.done && !i.undone) {
        return Err(io::Error::other("No results available to undo."));
    }
    // Проверить все результаты до первого изменения.
    for item in cp.items.iter().filter(|i| i.done && !i.undone) {
        if item.nodes.is_empty() {
            return Err(io::Error::other("No confirmed data for a safe undo."));
        }
        unchanged(&item.nodes, ctx)?;
        if item.backup.is_some() {
            unchanged(&item.backup_nodes, ctx)?;
        }
        if !undo_as_copy(record, cp)
            && matches!(record.action, Action::Move | Action::Rename | Action::Trash)
            && fs::symlink_metadata(item.source.path()).is_ok()
        {
            return Err(io::Error::other(
                "The original name is in use. Free it before undoing.",
            ));
        }
    }

    Ok(())
}

/// Неуспешный внешний Move не разрешает отправителю удалять оригиналы.
/// Его подтверждённые результаты отменяются как копии, с проверкой неизменности.
fn undo_as_copy(record: &Record, cp: &Checkpoint) -> bool {
    matches!(
        record.action,
        Action::Copy | Action::CreateFile | Action::CreateDirectory
    ) || (matches!(record.action, Action::Move)
        && cp.receive_move
        && record
            .finished
            .as_ref()
            .is_some_and(|finished| !matches!(finished.result, ResultKind::Success)))
}

/// Входящий внешний Move может не иметь локальной резервной копии источника.
/// При ошибке сохраняем обе стороны и незавершённое состояние для ручного разбора.
fn undo_move_across_devices(
    journal: &mut crate::history::Journal,
    id: u64,
    cp: &mut Checkpoint,
    index: usize,
    ctx: &mut Context,
) -> io::Result<()> {
    let target = cp.items[index].target.path();
    let original = cp.items[index].source.path();
    super::copy::copy_tree(&target, &original, ctx, &mut |_, _| Ok(()), 0)?;
    unchanged(&cp.items[index].nodes, ctx)?;
    let trash = platform::trash_target(&target)?;
    cp.items[index].undo_trash = Some(SavedPath::new(&trash));
    journal.update_record(id, encode(cp)?)?;
    platform::write_trash_info(&target, &platform::trash_info_path(&trash)?)?;
    unchanged(&cp.items[index].nodes, ctx)?;
    platform::rename_exclusive(&target, &trash)?;
    sync_parent(&trash)?;
    sync_parent(&target)?;
    sync_parent(&original)?;
    Ok(())
}
