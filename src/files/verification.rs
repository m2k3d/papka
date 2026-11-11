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
        return Err(io::Error::new(io::ErrorKind::Interrupted, "Прервано"));
    }
    if depth > 256 {
        return Err(io::Error::other("Слишком глубокое дерево каталогов."));
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
            "Данные менялись во время проверки. Повторите действие.",
        ));
    }
    Ok(hasher.finish())
}
pub(super) fn collect(path: &Path, nodes: &mut Vec<Node>, context: &Context) -> io::Result<()> {
    let before = stamp(path)?;
    let tree = tree_stamp(path, Some(context), 0)?;
    if stamp(path)? != before {
        return Err(io::Error::other("Данные изменились во время проверки."));
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
        return Err(io::Error::other(
            "Нет подтверждённых данных для безопасной отмены.",
        ));
    }
    for node in nodes {
        let path = node.path.path();
        if stamp(&path)? != node.stamp || tree_stamp(&path, Some(context), 0)? != node.tree {
            return Err(io::Error::other(
                "Данные изменились после операции. Отмена остановлена; файлы сохранены.",
            ));
        }
    }
    Ok(())
}
