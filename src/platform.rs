//! Интеграции ОС и расположение пользовательских данных приложения.

use std::path::PathBuf;

/// Определённые ОС пути. Их получение не создаёт каталоги и не пишет данные.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppDirectories {
    pub config: PathBuf,
    pub history: PathBuf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirectoryError {
    ConfigUnavailable,
    HistoryUnavailable,
    UnsupportedPlatform,
}

impl std::fmt::Display for DirectoryError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::ConfigUnavailable => "Не удалось определить папку настроек.",
            Self::HistoryUnavailable => "Не удалось определить папку истории операций.",
            Self::UnsupportedPlatform => "Хранение настроек на этой ОС пока не поддерживается.",
        })
    }
}

impl std::error::Error for DirectoryError {}

impl AppDirectories {
    pub fn resolve() -> Result<Self, DirectoryError> {
        resolve_directories()
    }
}

#[cfg(target_os = "linux")]
fn resolve_directories() -> Result<AppDirectories, DirectoryError> {
    Ok(AppDirectories {
        config: dirs::config_dir()
            .ok_or(DirectoryError::ConfigUnavailable)?
            .join("papka"),
        history: dirs::state_dir()
            .ok_or(DirectoryError::HistoryUnavailable)?
            .join("papka"),
    })
}

#[cfg(target_os = "windows")]
fn resolve_directories() -> Result<AppDirectories, DirectoryError> {
    Ok(AppDirectories {
        config: dirs::config_dir()
            .ok_or(DirectoryError::ConfigUnavailable)?
            .join("papka"),
        history: dirs::data_local_dir()
            .ok_or(DirectoryError::HistoryUnavailable)?
            .join("papka"),
    })
}

#[cfg(target_os = "macos")]
fn resolve_directories() -> Result<AppDirectories, DirectoryError> {
    let base = dirs::data_dir()
        .ok_or(DirectoryError::ConfigUnavailable)?
        .join("papka");
    Ok(AppDirectories {
        config: base.join("config"),
        history: base.join("history"),
    })
}

#[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
fn resolve_directories() -> Result<AppDirectories, DirectoryError> {
    Err(DirectoryError::UnsupportedPlatform)
}

/// Linux renameat2 исключает перезапись назначения, в том числе при гонке.
pub fn rename_exclusive(source: &std::path::Path, target: &std::path::Path) -> std::io::Result<()> {
    use std::os::unix::ffi::OsStrExt;
    let source = std::ffi::CString::new(source.as_os_str().as_bytes())?;
    let target = std::ffi::CString::new(target.as_os_str().as_bytes())?;
    let result = unsafe {
        libc::renameat2(
            libc::AT_FDCWD,
            source.as_ptr(),
            libc::AT_FDCWD,
            target.as_ptr(),
            libc::RENAME_NOREPLACE,
        )
    };
    if result == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

/// Домашняя корзина freedesktop. При EXDEV операция отказывает без удаления.
pub fn trash_directory(source: &std::path::Path) -> std::io::Result<PathBuf> {
    use std::os::unix::fs::{DirBuilderExt, MetadataExt};
    // Для ссылки выбирается файловая система самой ссылки, а не её цели.
    let parent = source
        .parent()
        .ok_or_else(|| std::io::Error::other("Нельзя удалить корень файловой системы."))?;
    let parent = std::fs::canonicalize(parent)?;
    let device = std::fs::metadata(&parent)?.dev();
    let home = dirs::data_dir().ok_or_else(|| std::io::Error::other("Не определена корзина."))?;
    let mut home_ancestor = home.as_path();
    while !home_ancestor.exists() {
        home_ancestor = home_ancestor
            .parent()
            .ok_or_else(|| std::io::Error::other("Не определена корзина."))?;
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
            "Нельзя повторно отправить содержимое корзины в неё же.",
        ));
    }
    for path in [&root, &root.join("files"), &root.join("info")] {
        let mut builder = std::fs::DirBuilder::new();
        builder.recursive(true).mode(0o700).create(path)?;
        let m = std::fs::symlink_metadata(path)?;
        if !m.is_dir() || m.is_symlink() || m.uid() != uid || m.mode() & 0o077 != 0 {
            return Err(std::io::Error::other(
                "Небезопасный каталог корзины. Удаление отменено.",
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
        .ok_or_else(|| std::io::Error::other("Не определена корзина."))?;
    let mut name = target
        .file_name()
        .ok_or_else(|| std::io::Error::other("Нет имени файла."))?
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
        .ok_or_else(|| std::io::Error::other("Некорректный путь корзины."))?;
    let stored = if root
        .file_name()
        .is_some_and(|n| n.as_bytes().starts_with(b".Trash-"))
    {
        source
            .strip_prefix(
                root.parent()
                    .ok_or_else(|| std::io::Error::other("Не определён том корзины."))?,
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
            "Не удалось определить дату удаления.",
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
pub fn open_file(path: &std::path::Path) -> std::io::Result<()> {
    // Абсолютный путь исключает интерпретацию имени как аргумента командной строки.
    let status = std::process::Command::new("xdg-open")
        .arg(path)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()?;
    if status.success() {
        Ok(())
    } else {
        Err(std::io::Error::other(
            "Не удалось открыть файл стандартным приложением.",
        ))
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq, Hash)]
pub struct FileStamp {
    dev: u64,
    ino: u64,
    size: u64,
    mtime: i64,
    mtime_ns: i64,
    ctime: i64,
    ctime_ns: i64,
    mode: u32,
}
pub fn file_stamp(path: &std::path::Path) -> std::io::Result<FileStamp> {
    use std::os::unix::fs::MetadataExt;
    let m = std::fs::symlink_metadata(path)?;
    Ok(FileStamp {
        dev: m.dev(),
        ino: m.ino(),
        size: m.size(),
        mtime: m.mtime(),
        mtime_ns: m.mtime_nsec(),
        ctime: m.ctime(),
        ctime_ns: m.ctime_nsec(),
        mode: m.mode(),
    })
}

/// Двоичное представление пути Linux для журнала без потерь не-UTF8 имён.
pub fn path_bytes(path: &std::path::Path) -> Vec<u8> {
    use std::os::unix::ffi::OsStrExt;
    path.as_os_str().as_bytes().to_vec()
}
pub fn path_from_bytes(bytes: Vec<u8>) -> PathBuf {
    use std::os::unix::ffi::OsStringExt;
    std::ffi::OsString::from_vec(bytes).into()
}
pub fn copy_symlink(source: &std::path::Path, target: &std::path::Path) -> std::io::Result<()> {
    std::os::unix::fs::symlink(std::fs::read_link(source)?, target)
}
