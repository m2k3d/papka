//! Настройки приложения и их атомарное сохранение.
use crate::platform;
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File},
    io,
    path::PathBuf,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub hidden: bool,
    pub descending: bool,
    pub sort: Sort,
    pub dark: bool,
    pub palette: [u32; 3],
    pub palette_count: u8,
    pub reduced_motion: bool,
    pub quick_access: Vec<PathBuf>,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            hidden: false,
            descending: false,
            sort: Sort::Name,
            dark: false,
            palette: [0xc7c2df, 0xe7c4b5, 0xb8cddd],
            palette_count: 3,
            reduced_motion: false,
            quick_access: default_quick_access(),
        }
    }
}
fn default_quick_access() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    for path in [
        dirs::home_dir(),
        dirs::desktop_dir(),
        dirs::document_dir(),
        dirs::download_dir(),
        dirs::audio_dir(),
        dirs::picture_dir(),
        dirs::video_dir(),
    ]
    .into_iter()
    .flatten()
    {
        if !paths.contains(&path) {
            paths.push(path);
        }
    }
    paths
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Sort {
    Name,
    Size,
    Modified,
}
impl std::fmt::Display for Sort {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Name => "Имя",
            Self::Size => "Размер",
            Self::Modified => "Изменён",
        })
    }
}
pub fn settings_load() -> io::Result<Settings> {
    let path = platform::AppDirectories::resolve()
        .map_err(io::Error::other)?
        .config
        .join("settings.json");
    match fs::read(path) {
        Ok(bytes) => serde_json::from_slice(&bytes).map_err(io::Error::other),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Settings::default()),
        Err(e) => Err(e),
    }
}
pub fn settings_save(settings: &Settings) -> io::Result<()> {
    let path = platform::AppDirectories::resolve()
        .map_err(io::Error::other)?
        .config;
    fs::create_dir_all(&path)?;
    let mut file = tempfile::NamedTempFile::new_in(&path)?;
    serde_json::to_writer(&mut file, settings)?;
    file.as_file().sync_all()?;
    file.persist(path.join("settings.json"))
        .map_err(|e| e.error)?;
    File::open(path)?.sync_all()
}
