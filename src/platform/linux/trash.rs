//! Корзина freedesktop и запись trashinfo на Linux.
use std::path::PathBuf;

/// Домашняя корзина freedesktop. При EXDEV операция отказывает без удаления.
pub fn trash_directory(source: &std::path::Path) -> std::io::Result<PathBuf> {
    use std::os::unix::fs::{DirBuilderExt, MetadataExt};
    // Для ссылки выбирается файловая система самой ссылки, а не её цели.
    let parent = source
        .parent()
        .ok_or_else(|| std::io::Error::other("Cannot trash the filesystem root."))?;
    let parent = std::fs::canonicalize(parent)?;
    let device = std::fs::metadata(&parent)?.dev();
    let home = dirs::data_dir().ok_or_else(|| std::io::Error::other("Could not locate the trash."))?;
    let mut home_ancestor = home.as_path();
    while !home_ancestor.exists() {
        home_ancestor = home_ancestor
            .parent()
            .ok_or_else(|| std::io::Error::other("Could not locate the trash."))?;
    }
    let uid = unsafe { libc::geteuid() };
    let root = if std::fs::metadata(home_ancestor)?.dev() == device {
        home.join("Trash")
    } else {
        let mut mount = parent.clone();
        while let Some(up) = mount.parent() {
            if std::fs::metadata(up)?.dev() != device {
                break;
            }
            mount = up.to_path_buf();
        }
        mount.join(format!(".Trash-{uid}"))
    };
    if source.starts_with(&root) {
        return Err(std::io::Error::other(
            "Cannot move trash contents into the trash again.",
        ));
    }
    for path in [&root, &root.join("files"), &root.join("info")] {
        let mut builder = std::fs::DirBuilder::new();
        builder.recursive(true).mode(0o700).create(path)?;
        let m = std::fs::symlink_metadata(path)?;
        if !m.is_dir() || m.is_symlink() || m.uid() != uid || m.mode() & 0o077 != 0 {
            return Err(std::io::Error::other(
                "Unsafe trash directory. Nothing was trashed.",
            ));
        }
    }
    Ok(root.join("files"))
}
pub fn trash_target(source: &std::path::Path) -> std::io::Result<PathBuf> {
    let directory = trash_directory(source)?;
    let time = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(std::io::Error::other)?
        .as_nanos();
    Ok(directory.join(format!("papka-{}-{time}", std::process::id())))
}

pub fn trash_info_path(target: &std::path::Path) -> std::io::Result<PathBuf> {
    let root = target
        .parent()
        .and_then(|p| p.parent())
        .ok_or_else(|| std::io::Error::other("Could not locate the trash."))?;
    let mut name = target
        .file_name()
        .ok_or_else(|| std::io::Error::other("No file name."))?
        .to_os_string();
    name.push(".trashinfo");
    Ok(root.join("info").join(name))
}
pub fn write_trash_info(source: &std::path::Path, info: &std::path::Path) -> std::io::Result<()> {
    use std::{
        io::Write,
        os::unix::{ffi::OsStrExt, fs::OpenOptionsExt},
    };
    let mut encoded = String::new();
    let root = info
        .parent()
        .and_then(|p| p.parent())
        .ok_or_else(|| std::io::Error::other("Invalid trash path."))?;
    let stored = if root
        .file_name()
        .is_some_and(|n| n.as_bytes().starts_with(b".Trash-"))
    {
        source
            .strip_prefix(
                root.parent()
                    .ok_or_else(|| std::io::Error::other("Could not determine the trash volume."))?,
            )
            .map_err(std::io::Error::other)?
    } else {
        source
    };
    for &b in stored.as_os_str().as_bytes() {
        if b.is_ascii_alphanumeric() || b"/-._~".contains(&b) {
            encoded.push(b as char);
        } else {
            encoded.push_str(&format!("%{b:02X}"));
        }
    }
    let seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(std::io::Error::other)?
        .as_secs() as libc::time_t;
    let mut time: libc::tm = unsafe { std::mem::zeroed() };
    if unsafe { libc::localtime_r(&seconds, &mut time) }.is_null() {
        return Err(std::io::Error::other(
            "Could not determine the deletion date.",
        ));
    }
    let date = format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}",
        time.tm_year + 1900,
        time.tm_mon + 1,
        time.tm_mday,
        time.tm_hour,
        time.tm_min,
        time.tm_sec
    );
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(info)?;
    write!(file, "[Trash Info]\nPath={encoded}\nDeletionDate={date}\n")?;
    file.sync_all()?;
    std::fs::File::open(info.parent().unwrap())?.sync_all()
}
