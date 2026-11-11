//! Чтение, сортировка и отправка каталога порциями.
use super::{Data, Settings, Sort};
use crate::operations::Context;
use std::{fs, io, path::PathBuf, time::UNIX_EPOCH};

#[derive(Debug, Clone)]
pub struct Entry {
    pub path: PathBuf,
    pub name: String,
    sort_name: String,
    pub directory: bool,
    pub link: bool,
    pub hidden: bool,
    pub size: u64,
    pub modified: u64,
}

pub fn listing(path: PathBuf, settings: Settings, context: &mut Context) -> io::Result<()> {
    let path = fs::canonicalize(path)?;
    let mut entries = Vec::new();
    let mut skipped = 0;
    for item in fs::read_dir(&path)? {
        if context.is_cancelled() {
            return Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "Чтение прервано",
            ));
        }
        let item = match item {
            Ok(item) => item,
            Err(_) => {
                skipped += 1;
                continue;
            }
        };
        let p = item.path();
        let metadata = match fs::symlink_metadata(&p) {
            Ok(m) => m,
            Err(_) => {
                skipped += 1;
                continue;
            }
        };
        let name = item.file_name().to_string_lossy().into_owned();
        entries.push(Entry {
            sort_name: name.to_lowercase(),
            hidden: name.starts_with('.'),
            name,
            directory: if metadata.is_symlink() {
                p.is_dir()
            } else {
                metadata.is_dir()
            },
            link: metadata.is_symlink(),
            size: metadata.len(),
            modified: metadata
                .modified()
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map_or(0, |t| t.as_secs()),
            path: p,
        });
        context.progress(entries.len() as u64, None);
    }
    if context.is_cancelled() {
        return Err(io::Error::new(
            io::ErrorKind::Interrupted,
            "Чтение прервано",
        ));
    }
    entries.sort_by(|a, b| {
        b.directory.cmp(&a.directory).then_with(|| {
            let order = match settings.sort {
                Sort::Name => a.sort_name.cmp(&b.sort_name),
                Sort::Size => a.size.cmp(&b.size),
                Sort::Modified => a.modified.cmp(&b.modified),
            }
            .then_with(|| a.path.cmp(&b.path));
            if settings.descending {
                order.reverse()
            } else {
                order
            }
        })
    });
    context.data(Data::ListingBegin(path));
    let mut entries = entries.into_iter();
    loop {
        if context.is_cancelled() {
            return Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "Чтение прервано",
            ));
        }
        let batch: Vec<_> = entries.by_ref().take(256).collect();
        if batch.is_empty() {
            break;
        }
        context.data(Data::ListingBatch(batch));
    }
    context.data(Data::ListingEnd(
        (skipped > 0).then(|| format!("Не удалось прочитать элементов: {skipped}")),
    ));
    Ok(())
}
