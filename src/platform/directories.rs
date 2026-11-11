//! Расположение данных приложения по правилам целевой ОС.
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
            Self::ConfigUnavailable => "Could not locate the settings directory.",
            Self::HistoryUnavailable => "Could not locate the operation history directory.",
            Self::UnsupportedPlatform => "Settings storage is not supported on this OS yet.",
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
