//! Выполнение запланированных файловых действий с журналированием.
use super::{
    Command, Engine,
    checkpoint::{Checkpoint, encode},
    copy::copy_tree,
    now,
    transfer::transfer_across_devices,
    verification::collect,
};
use crate::{
    history::{Action, Journal, ResultKind},
    operations::Context,
    platform::{self, sync_parent},
};
use std::{
    fs::{self, OpenOptions},
    io,
};

impl Engine {
    pub(super) fn execute(&mut self, command: Command, ctx: &mut Context) -> io::Result<()> {
        if let Command::Undo(id) = command {
            return self.undo(id, ctx);
        }
        if let Command::Acknowledge(id) = command {
            let journal = self.journal.as_mut().unwrap();
            let record = journal
                .records()
                .iter()
                .find(|r| r.id == id && r.finished.is_none())
                .cloned()
                .ok_or_else(|| io::Error::other("Незавершённая запись не найдена."))?;
            return journal.finish(id, ResultKind::Interrupted, record.checkpoint, now());
        }
        let super::prepare::Plan {
            action,
            checkpoint: mut cp,
            create,
        } = super::prepare::prepare(command, ctx)?;
        let journal = self.journal.as_mut().unwrap();
        let id = journal.begin(action.clone(), encode(&cp)?, now())?;
        let result = execute_items(journal, id, &mut cp, &action, create, ctx);
        let kind = match &result {
            Ok(()) => ResultKind::Success,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => ResultKind::Interrupted,
            Err(_)
                if cp.items.iter().any(|i| {
                    i.done || (i.started && fs::symlink_metadata(i.target.path()).is_ok())
                }) =>
            {
                ResultKind::Partial
            }
            Err(_) => ResultKind::Failed,
        };
        journal.finish(id, kind, encode(&cp)?, now())?;
        result
    }
}

/// Намерение уже сохранено; каждый элемент подтверждается отдельной контрольной точкой.
fn execute_items(
    journal: &mut Journal,
    id: u64,
    cp: &mut Checkpoint,
    action: &Action,
    create: Option<bool>,
    ctx: &mut Context,
) -> io::Result<()> {
    for index in 0..cp.items.len() {
        if ctx.is_cancelled() {
            return Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "Операция прервана; частичные результаты сохранены в истории.",
            ));
        }
        let source = cp.items[index].source.path();
        let target = cp.items[index].target.path();
        cp.items[index].started = true;
        journal.checkpoint(id, encode(cp)?)?;
        if let Some(directory) = create {
            if directory {
                fs::create_dir(&target)?;
            } else {
                OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&target)?
                    .sync_all()?;
            }
        } else if matches!(action, Action::Copy) {
            copy_tree(
                &source,
                &target,
                ctx,
                &mut |count, bytes| {
                    cp.items[index].copied_entries += count;
                    cp.items[index].copied_bytes += bytes;
                    journal.checkpoint(id, encode(cp)?)
                },
                0,
            )?;
        } else {
            if matches!(action, Action::Trash) {
                platform::write_trash_info(
                    &source,
                    &cp.items[index].trash_info.as_ref().unwrap().path(),
                )?;
            }
            match platform::rename_exclusive(&source, &target) {
                Ok(()) => {}
                Err(e)
                    if e.raw_os_error() == Some(libc::EXDEV) && matches!(action, Action::Move) =>
                {
                    transfer_across_devices(journal, id, cp, index, ctx, &source, &target)?;
                }
                Err(e) => return Err(e),
            }
        }
        sync_parent(&target)?;
        if create.is_none() && !matches!(action, Action::Copy) {
            sync_parent(&source)?;
        }
        let mut nodes = Vec::new();
        collect(&target, &mut nodes, ctx)?;
        cp.items[index].nodes = nodes;
        cp.items[index].done = true;
        journal.checkpoint(id, encode(cp)?)?;
        ctx.progress(index as u64 + 1, Some(cp.items.len() as u64));
    }
    Ok(())
}
