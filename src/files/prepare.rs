//! Планирование назначения и проверки до записи намерения.
use super::{
    Command, SavedPath,
    checkpoint::{Checkpoint, Item},
    verification::collect,
};
use crate::{history::Action, operations::Context, platform};
use std::{fs, io, path::PathBuf};

pub(super) struct Plan {
    pub(super) action: Action,
    pub(super) checkpoint: Checkpoint,
    pub(super) create: Option<bool>,
}

pub(super) fn prepare(command: Command, ctx: &Context) -> io::Result<Plan> {
    let (action, pairs, create) = match command {
        Command::Create(path, directory) => (
            if directory {
                Action::CreateDirectory
            } else {
                Action::CreateFile
            },
            vec![(path.clone(), path)],
            Some(directory),
        ),
        Command::Copy(paths, directory) => (Action::Copy, targets(paths, directory)?, None),
        Command::Move(paths, directory) => (Action::Move, targets(paths, directory)?, None),
        Command::Rename(a, b) => (Action::Rename, vec![(a, b)], None),
        Command::Trash(paths) => {
            let mut pairs = Vec::new();
            for path in paths {
                let target = platform::trash_target(&path)?;
                pairs.push((path, target));
            }
            (Action::Trash, pairs, None)
        }
        Command::Undo(_) | Command::Acknowledge(_) => unreachable!(),
    };
    if pairs.is_empty() {
        return Err(io::Error::other("Ничего не выделено."));
    }
    if pairs.len() > 100 {
        return Err(io::Error::other(
            "Выберите не более 100 элементов за одну операцию.",
        ));
    }
    let mut cp = Checkpoint {
        version: 1,
        items: Vec::new(),
        undoing: false,
    };
    for (source, target) in pairs {
        if fs::symlink_metadata(&target).is_ok() {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                format!(
                    "Уже существует: {}. Выберите другое имя или расположение.",
                    target.display()
                ),
            ));
        }
        if create.is_none() {
            let metadata = fs::symlink_metadata(&source)?;
            let parent = target
                .parent()
                .ok_or_else(|| io::Error::other("Нет родительского каталога"))?;
            let canonical_parent = fs::canonicalize(parent)?;
            if metadata.is_dir() && canonical_parent.starts_with(fs::canonicalize(&source)?) {
                return Err(io::Error::other("Нельзя поместить папку внутрь неё самой."));
            }
            let mut preview = Vec::new();
            collect(&source, &mut preview, ctx)?;
        }
        let trash_info = if matches!(action, Action::Trash) {
            Some(SavedPath::new(&platform::trash_info_path(&target)?))
        } else {
            None
        };
        cp.items.push(Item {
            source: SavedPath::new(&source),
            target: SavedPath::new(&target),
            nodes: Vec::new(),
            started: false,
            done: false,
            undone: false,
            trash_info,
            backup: None,
            backup_nodes: Vec::new(),
            undo_trash: None,
            copied_entries: 0,
            copied_bytes: 0,
        });
    }
    Ok(Plan {
        action,
        checkpoint: cp,
        create,
    })
}

fn targets(paths: Vec<PathBuf>, directory: PathBuf) -> io::Result<Vec<(PathBuf, PathBuf)>> {
    if !directory.is_dir() {
        return Err(io::Error::other("Папка назначения недоступна."));
    }
    paths
        .into_iter()
        .map(|p| {
            let name = p
                .file_name()
                .ok_or_else(|| io::Error::other("Нельзя перемещать корень файловой системы."))?;
            let target = directory.join(name);
            Ok((p, target))
        })
        .collect()
}
