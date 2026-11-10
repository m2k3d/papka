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
