//! Linux: переименование без перезаписи, отпечатки и двоичные пути.
use std::path::PathBuf;

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

pub fn sync_parent(path: &std::path::Path) -> std::io::Result<()> {
    std::fs::File::open(
        path.parent()
            .ok_or_else(|| std::io::Error::other("No parent directory."))?,
    )?
    .sync_all()
}
