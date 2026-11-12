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
    pub palette_positions: [[f32; 2]; 3],
    pub palette_intensity: f32,
    // Сохраняется для совместимости старого формата; новые схемы задаёт palette_harmony.
    pub palette_combination: u8,
    // Отсутствие поля у старых настроек означает свободную палитру без миграции цветов.
    #[serde(default)]
    pub palette_harmony: Option<PaletteHarmony>,
    pub reduced_motion: bool,
    pub quick_access: Vec<PathBuf>,
}
/// Угловые отношения оттенков; значения не зависят от порядка вариантов в UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PaletteHarmony {
    Single,
    Complementary,
    SingleAnalogous,
    SplitComplementary,
    Analogous,
    Triadic,
    Custom,
}
impl PaletteHarmony {
    pub fn choices(count: u8) -> &'static [Self] {
        match count.clamp(1, 3) {
            1 => &[Self::Single, Self::Custom],
            2 => &[Self::Complementary, Self::SingleAnalogous, Self::Custom],
            _ => &[
                Self::SplitComplementary,
                Self::Analogous,
                Self::Triadic,
                Self::Custom,
            ],
        }
    }
    pub fn for_count(self, count: u8) -> Self {
        if Self::choices(count).contains(&self) {
            self
        } else if self == Self::Analogous && count == 2 {
            Self::SingleAnalogous
        } else if self == Self::SingleAnalogous && count == 3 {
            Self::Analogous
        } else {
            Self::choices(count)[0]
        }
    }
    fn angles(self) -> &'static [f32] {
        match self {
            Self::Complementary => &[0.0, 180.0],
            Self::SingleAnalogous => &[0.0, 310.0],
            Self::SplitComplementary => &[0.0, 150.0, 210.0],
            Self::Analogous => &[0.0, 50.0, 310.0],
            Self::Triadic => &[0.0, 120.0, 240.0],
            _ => &[0.0],
        }
    }
}
impl std::fmt::Display for PaletteHarmony {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Single => "Single",
            Self::Complementary => "Complementary",
            Self::SingleAnalogous | Self::Analogous => "Analogous",
            Self::SplitComplementary => "Split complementary",
            Self::Triadic => "Triadic",
            Self::Custom => "Custom",
        })
    }
}
impl Settings {
    pub const DEFAULT_PALETTE: [u32; 3] = [0xc7c2df, 0xdfc7c2, 0xc2dfc7];

    pub fn harmony(&self) -> PaletteHarmony {
        self.palette_harmony
            .unwrap_or(PaletteHarmony::Custom)
            .for_count(self.palette_count)
    }

    pub fn wheel_position(&self, index: usize) -> [f32; 2] {
        if self.palette_harmony.is_some() {
            circle_position(self.palette_positions[index])
        } else {
            color_position(self.palette[index])
        }
    }

    pub fn prepare_palette_wheel(&mut self) {
        if self.palette_harmony.is_none() {
            self.palette_positions = self.palette.map(color_position);
            self.palette_harmony = Some(PaletteHarmony::Custom);
        }
    }

    pub fn apply_palette_color(&mut self, index: usize, color: u32) {
        self.prepare_palette_wheel();
        let [mut hue, saturation, lightness] = color_hsl(color);
        // У серых цветов нет оттенка: сохраняем направление точки на круге.
        if saturation < 0.0001 {
            let p = self.wheel_position(index);
            hue = (p[1] - 0.5)
                .atan2(p[0] - 0.5)
                .to_degrees()
                .rem_euclid(360.0);
        }
        self.apply_wheel_color(index, hue, saturation, lightness);
        // HEX применяется точно, без дополнительного округления RGB → HSL → RGB.
        self.palette[index] = color;
    }

    pub fn move_palette_point(&mut self, index: usize, point: [f32; 2]) {
        self.prepare_palette_wheel();
        let p = circle_position(point);
        let (x, y) = (p[0] - 0.5, p[1] - 0.5);
        let lightness = (2.0 * x.hypot(y)).clamp(0.0, 1.0);
        let hue = if lightness > 0.0001 {
            y.atan2(x).to_degrees().rem_euclid(360.0)
        } else {
            let old = self.wheel_position(index);
            (old[1] - 0.5)
                .atan2(old[0] - 0.5)
                .to_degrees()
                .rem_euclid(360.0)
        };
        // Как в Zen: угол задаёт оттенок, радиус — светлоту; насыщенность 90–100%.
        self.apply_wheel_color(index, hue, 0.9 + 0.1 * lightness, lightness);
    }

    pub fn apply_palette_harmony(&mut self, harmony: PaletteHarmony) {
        self.prepare_palette_wheel();
        self.palette_harmony = Some(harmony.for_count(self.palette_count));
        self.apply_palette_color(0, self.palette[0]);
    }

    fn apply_wheel_color(&mut self, index: usize, hue: f32, saturation: f32, lightness: f32) {
        let harmony = self.harmony();
        self.palette_harmony = Some(harmony);
        if harmony == PaletteHarmony::Custom {
            self.palette[index] = hsl_color(hue, saturation, lightness);
            self.palette_positions[index] = wheel_point(hue, lightness);
            return;
        }
        let angles = harmony.angles();
        let base = hue - angles[index];
        for (i, angle) in angles.iter().enumerate() {
            let hue = (base + angle).rem_euclid(360.0);
            self.palette[i] = hsl_color(hue, saturation, lightness);
            self.palette_positions[i] = wheel_point(hue, lightness);
        }
    }
}

pub fn circle_position(point: [f32; 2]) -> [f32; 2] {
    let [x, y] = point.map(|v| if v.is_finite() { v - 0.5 } else { 0.0 });
    let scale = (2.0 * x.hypot(y)).max(1.0);
    [0.5 + x / scale, 0.5 + y / scale]
}
fn wheel_point(hue: f32, lightness: f32) -> [f32; 2] {
    let angle = hue.to_radians();
    [
        0.5 + 0.5 * lightness * angle.cos(),
        0.5 + 0.5 * lightness * angle.sin(),
    ]
}
fn color_position(color: u32) -> [f32; 2] {
    let [hue, _, lightness] = color_hsl(color);
    wheel_point(hue, lightness)
}
fn color_hsl(color: u32) -> [f32; 3] {
    let [r, g, b] = [16, 8, 0].map(|shift| ((color >> shift) & 255) as f32 / 255.0);
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let delta = max - min;
    let lightness = (max + min) * 0.5;
    if delta < 0.00001 {
        return [0.0, 0.0, lightness];
    }
    let sector = if max == r {
        (g - b) / delta
    } else if max == g {
        (b - r) / delta + 2.0
    } else {
        (r - g) / delta + 4.0
    };
    [
        (sector * 60.0).rem_euclid(360.0),
        delta / (1.0 - (2.0 * lightness - 1.0).abs()),
        lightness,
    ]
}
pub fn hsl_color(hue: f32, saturation: f32, lightness: f32) -> u32 {
    let c = (1.0 - (2.0 * lightness - 1.0).abs()) * saturation;
    let sector = hue.rem_euclid(360.0) / 60.0;
    let x = c * (1.0 - (sector.rem_euclid(2.0) - 1.0).abs());
    let channels = match sector as u8 {
        0 => [c, x, 0.0],
        1 => [x, c, 0.0],
        2 => [0.0, c, x],
        3 => [0.0, x, c],
        4 => [x, 0.0, c],
        _ => [c, 0.0, x],
    };
    channels.into_iter().fold(0, |color, channel| {
        (color << 8)
            | ((channel + lightness - c * 0.5) * 255.0)
                .round()
                .clamp(0.0, 255.0) as u32
    })
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            hidden: false,
            descending: false,
            sort: Sort::Name,
            dark: false,
            palette: Self::DEFAULT_PALETTE,
            palette_count: 3,
            palette_positions: Self::DEFAULT_PALETTE.map(color_position),
            palette_intensity: 0.35,
            palette_combination: 0,
            palette_harmony: Some(PaletteHarmony::Triadic),
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
            Self::Name => "Name",
            Self::Size => "Size",
            Self::Modified => "Modified",
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
