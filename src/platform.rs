//! Платформенный API; файловые интеграции текущего релиза реализованы для Linux.
#[path = "platform/directories.rs"]
mod directories;

#[cfg(target_os = "linux")]
#[path = "platform/linux/filesystem.rs"]
mod filesystem;
#[cfg(target_os = "linux")]
#[path = "platform/linux/open.rs"]
mod open;
#[cfg(target_os = "linux")]
#[path = "platform/linux/trash.rs"]
mod trash;

pub use directories::{AppDirectories, DirectoryError};
#[cfg(target_os = "linux")]
pub use filesystem::{
    FileStamp, copy_symlink, file_stamp, path_bytes, path_from_bytes, rename_exclusive, sync_parent,
};
#[cfg(target_os = "linux")]
pub use open::open_file;
#[cfg(target_os = "linux")]
pub use trash::{trash_directory, trash_info_path, trash_target, write_trash_info};

#[path = "platform/linux/dnd.rs"]
mod dnd;
pub use dnd::{NativeDnd, Wake as DndWake};
