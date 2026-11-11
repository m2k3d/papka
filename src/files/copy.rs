//! Рекурсивное копирование с прерыванием и контрольными точками.
use crate::{
    operations::Context,
    platform::{self, file_stamp as stamp, sync_parent},
};
use std::{
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
    path::Path,
};

pub(super) fn copy_tree(
    source: &Path,
    target: &Path,
    ctx: &mut Context,
    checkpoint: &mut impl FnMut(u64, u64) -> io::Result<()>,
    depth: usize,
) -> io::Result<()> {
    if depth > 256 {
        return Err(io::Error::other("Directory tree is too deep."));
    }
    if ctx.is_cancelled() {
        return Err(io::Error::new(
            io::ErrorKind::Interrupted,
            "Copy interrupted; some data may have been copied.",
        ));
    }
    let before = stamp(source)?;
    let metadata = fs::symlink_metadata(source)?;
    if metadata.is_symlink() {
        platform::copy_symlink(source, target)?;
    } else if metadata.is_dir() {
        fs::create_dir(target)?;
        sync_parent(target)?;
        for item in fs::read_dir(source)? {
            let item = item?;
            copy_tree(
                &item.path(),
                &target.join(item.file_name()),
                ctx,
                checkpoint,
                depth + 1,
            )?;
        }
        fs::set_permissions(target, metadata.permissions())?;
        File::open(target)?.sync_all()?;
    } else if metadata.is_file() {
        let mut input = File::open(source)?;
        let mut output = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(target)?;
        sync_parent(target)?;
        let mut buffer = vec![0; 256 * 1024];
        let mut pending_bytes = 0;
        loop {
            if ctx.is_cancelled() {
                return Err(io::Error::new(
                    io::ErrorKind::Interrupted,
                    "Copy interrupted; partial file preserved.",
                ));
            }
            let n = input.read(&mut buffer)?;
            if n == 0 {
                break;
            }
            output.write_all(&buffer[..n])?;
            pending_bytes += n as u64;
            if pending_bytes >= 8 * 1024 * 1024 {
                output.sync_data()?;
                checkpoint(0, pending_bytes)?;
                ctx.copied(pending_bytes);
                pending_bytes = 0;
            }
        }
        output.sync_all()?;
        checkpoint(0, pending_bytes)?;
        ctx.copied(pending_bytes);
        output.set_permissions(metadata.permissions())?;
        output.sync_all()?;
    } else {
        return Err(io::Error::other(
            "Copying special files is not supported.",
        ));
    }
    sync_parent(target)?;
    checkpoint(1, 0)?;
    if stamp(source)? != before {
        return Err(io::Error::other(
            "Source changed during copying. Review the copy.",
        ));
    }
    Ok(())
}
