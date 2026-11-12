//! Проверки неизменности дерева перед операциями и отменой.
use super::{SavedPath, checkpoint::Node};
use crate::{
    operations::Context,
    platform::{self, file_stamp as stamp},
};
use std::{fs, io, path::Path};

// Отпечаток включает имена, inode, ctime и метаданные всего дерева.
// Содержимое ссылок не обходится. В журнале остаётся один отпечаток на корень.
fn tree_stamp(path: &Path, context: Option<&Context>, depth: usize) -> io::Result<u64> {
    use std::hash::{Hash, Hasher};
    if context.is_some_and(Context::is_cancelled) {
        return Err(io::Error::new(io::ErrorKind::Interrupted, "Interrupted"));
    }
    if depth > 256 {
        return Err(io::Error::other("Directory tree is too deep."));
    }
    let before = stamp(path)?;
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    before.hash(&mut hasher);
    let metadata = fs::symlink_metadata(path)?;
    if metadata.is_symlink() {
        platform::path_bytes(&fs::read_link(path)?).hash(&mut hasher);
    } else if metadata.is_dir() {
        let mut children = fs::read_dir(path)?.collect::<io::Result<Vec<_>>>()?;
        children.sort_by_key(|e| e.file_name());
        for child in children {
            platform::path_bytes(Path::new(&child.file_name())).hash(&mut hasher);
            tree_stamp(&child.path(), context, depth + 1)?.hash(&mut hasher);
        }
    }
    if stamp(path)? != before {
        return Err(io::Error::other(
            "Data changed during verification. Try again.",
        ));
    }
    Ok(hasher.finish())
}
pub(super) fn collect(path: &Path, nodes: &mut Vec<Node>, context: &Context) -> io::Result<()> {
    let before = stamp(path)?;
    let tree = tree_stamp(path, Some(context), 0)?;
    if stamp(path)? != before {
        return Err(io::Error::other("Data changed during verification."));
    }
    nodes.push(Node {
        path: SavedPath::new(path),
        stamp: before,
        tree,
    });
    Ok(())
}
pub(super) fn unchanged(nodes: &[Node], context: &Context) -> io::Result<()> {
    if nodes.is_empty() {
        return Err(io::Error::other("No confirmed data for a safe undo."));
    }
    for node in nodes {
        let path = node.path.path();
        if stamp(&path)? != node.stamp || tree_stamp(&path, Some(context), 0)? != node.tree {
            return Err(io::Error::other(
                "Data changed after the operation. Undo stopped; files preserved.",
            ));
        }
    }
    Ok(())
}

/// Снимок источников до передачи стороннему приложению.
#[derive(Debug, Clone)]
pub struct DragSnapshot(Vec<Node>);
impl DragSnapshot {
    pub fn capture(paths: &[std::path::PathBuf], ctx: &Context) -> io::Result<Self> {
        if paths.is_empty() || paths.len() > 100 {
            return Err(io::Error::other("Drag between 1 and 100 items."));
        }
        let mut nodes = Vec::new();
        for path in paths {
            collect(path, &mut nodes, ctx)?;
        }
        Ok(Self(nodes))
    }
    pub fn paths(&self) -> Vec<std::path::PathBuf> {
        self.0.iter().map(|node| node.path.path()).collect()
    }
    /// Получатель может сам переместить источник до подтверждения протокола.
    pub(super) fn remaining(&self) -> io::Result<Self> {
        let mut nodes = Vec::new();
        for node in &self.0 {
            match fs::symlink_metadata(node.path.path()) {
                Ok(_) => nodes.push(node.clone()),
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => return Err(error),
            }
        }
        Ok(Self(nodes))
    }
    pub(super) fn verify(&self, ctx: &Context) -> io::Result<()> {
        unchanged(&self.0, ctx).map_err(|error| {
            io::Error::other(format!(
                "Drag sources could not be verified; originals preserved: {error}"
            ))
        })
    }
    pub(super) fn verify_path(&self, path: &Path, ctx: &Context) -> io::Result<()> {
        let node = self
            .0
            .iter()
            .find(|node| node.path.path() == path)
            .ok_or_else(|| io::Error::other("Unknown drag source; original preserved."))?;
        unchanged(std::slice::from_ref(node), ctx)
    }
}
