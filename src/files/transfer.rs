//! Перенос между файловыми системами с резервированием источника.
use super::{
    SavedPath,
    checkpoint::{Checkpoint, encode},
    copy::copy_tree,
    verification::{collect, unchanged},
};
use crate::{
    history::Journal,
    operations::Context,
    platform::{self, sync_parent},
};
use std::{io, path::Path};

pub(super) fn transfer_across_devices(
    journal: &mut Journal,
    id: u64,
    cp: &mut Checkpoint,
    index: usize,
    ctx: &mut Context,
    source: &Path,
    target: &Path,
) -> io::Result<()> {
    let mut source_nodes = Vec::new();
    collect(source, &mut source_nodes, ctx)?;
    copy_tree(
        source,
        target,
        ctx,
        &mut |count, bytes| {
            cp.items[index].copied_entries += count;
            cp.items[index].copied_bytes += bytes;
            journal.checkpoint(id, encode(&cp)?)
        },
        0,
    )?;
    unchanged(&source_nodes, ctx)?;
    // Источник сохраняется в корзине своего тома для обратного переноса.
    let backup = platform::trash_target(source)?;
    cp.items[index].backup = Some(SavedPath::new(&backup));
    journal.checkpoint(id, encode(&cp)?)?;
    platform::write_trash_info(source, &platform::trash_info_path(&backup)?)?;
    unchanged(&source_nodes, ctx)?;
    platform::rename_exclusive(source, &backup)?;
    sync_parent(source)?;
    sync_parent(&backup)?;
    let mut backup_nodes = Vec::new();
    collect(&backup, &mut backup_nodes, ctx)?;
    cp.items[index].backup_nodes = backup_nodes;
    Ok(())
}
