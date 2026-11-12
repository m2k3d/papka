//! Изменение настроек и их фоновое сохранение.
use super::{App, completion};
use crate::files::{self, Sort};
use std::path::PathBuf;

impl App {
    pub fn save_settings(&mut self) {
        self.page = 0;
        self.gestures.anchor = None;
        self.refresh();
        self.persist_settings();
    }

    pub fn persist_settings(&mut self) {
        let Some(background) = &self.background else {
            return;
        };
        let settings = self.settings.clone();
        match background.submit(move |_| completion(files::settings_save(&settings))) {
            Ok(ticket) => {
                self.work.auxiliary.insert(ticket.id);
            }
            Err(e) => self.error = Some(e.to_string()),
        }
    }

    pub fn pin(&mut self, path: PathBuf) {
        if !self.settings.quick_access.contains(&path) {
            self.settings.quick_access.push(path);
            self.persist_settings();
        }
    }

    pub fn unpin(&mut self, path: PathBuf) {
        self.settings
            .quick_access
            .retain(|candidate| candidate != &path);
        self.persist_settings();
    }

    pub fn open_palette(&mut self) {
        self.show_palette = true;
        self.palette_active = self
            .palette_active
            .min(self.settings.palette_count.clamp(1, 3) as usize - 1);
        self.palette_inputs = self.settings.palette.map(|color| format!("{color:06X}"));
    }

    pub fn close_palette(&mut self) {
        self.show_palette = false;
        self.persist_settings();
    }

    pub fn set_palette_count(&mut self, count: u8) {
        self.settings.prepare_palette_wheel();
        let harmony = self.settings.harmony();
        let old = self.settings.palette_count.clamp(1, 3);
        if count.clamp(1, 3) < old {
            let remove = self.palette_active.min(old as usize - 1);
            self.settings.palette[remove..old as usize].rotate_left(1);
            self.settings.palette_positions[remove..old as usize].rotate_left(1);
            self.palette_inputs[remove..old as usize].rotate_left(1);
        }
        self.settings.palette_count = count.clamp(1, 3);
        self.palette_active = if count > old {
            (count.clamp(1, 3) - 1) as usize
        } else {
            self.palette_active.min((count.clamp(1, 3) - 1) as usize)
        };
        self.settings
            .apply_palette_harmony(harmony.for_count(self.settings.palette_count));
        self.palette_inputs = self.settings.palette.map(|color| format!("{color:06X}"));
        self.persist_settings();
    }

    pub fn set_palette_point(&mut self, index: usize, point: [f32; 2]) {
        if index < self.settings.palette_count.clamp(1, 3) as usize {
            self.palette_active = index;
            self.settings.move_palette_point(index, point);
            self.palette_inputs = self.settings.palette.map(|color| format!("{color:06X}"));
        }
    }

    pub fn set_palette_color(&mut self, index: usize, value: String) {
        if index < self.settings.palette_count.clamp(1, 3) as usize {
            self.palette_inputs[index] = value;
            if let Some(color) = parse_palette_color(&self.palette_inputs[index]) {
                self.settings.apply_palette_color(index, color);
                for i in 0..3 {
                    if i != index {
                        self.palette_inputs[i] = format!("{:06X}", self.settings.palette[i]);
                    }
                }
                self.persist_settings();
            }
        }
    }

    pub fn palette_input_valid(&self, index: usize) -> bool {
        self.palette_inputs
            .get(index)
            .is_some_and(|value| parse_palette_color(value).is_some())
    }

    pub fn set_palette(&mut self, colors: [u32; 3]) {
        let defaults = files::Settings::default();
        self.settings.palette = colors;
        self.settings.palette_count = defaults.palette_count;
        self.settings.palette_positions = defaults.palette_positions;
        self.settings.palette_intensity = defaults.palette_intensity;
        self.settings.palette_combination = defaults.palette_combination;
        self.settings.palette_harmony = None;
        self.settings.prepare_palette_wheel();
        if colors == files::Settings::DEFAULT_PALETTE {
            self.settings
                .apply_palette_harmony(files::PaletteHarmony::Triadic);
        }
        self.palette_active = 0;
        self.palette_inputs = self.settings.palette.map(|color| format!("{color:06X}"));
        self.persist_settings();
    }

    pub fn cycle_palette_combination(&mut self) {
        let choices = files::PaletteHarmony::choices(self.settings.palette_count);
        let current = choices
            .iter()
            .position(|&h| h == self.settings.harmony())
            .unwrap_or(choices.len() - 1);
        self.settings
            .apply_palette_harmony(choices[(current + 1) % choices.len()]);
        self.palette_inputs = self.settings.palette.map(|color| format!("{color:06X}"));
        self.persist_settings();
    }

    pub fn set_hidden(&mut self, value: bool) {
        self.settings.hidden = value;
        self.selection.clear();
        self.save_settings();
    }

    pub fn set_dark(&mut self, value: bool) {
        self.settings.dark = value;
        self.persist_settings();
    }

    pub fn set_reduced_motion(&mut self, value: bool) {
        self.settings.reduced_motion = value;
        self.save_settings();
    }

    pub fn set_descending(&mut self, value: bool) {
        self.settings.descending = value;
        self.save_settings();
    }

    pub fn set_sort(&mut self, value: Sort) {
        self.settings.sort = value;
        self.save_settings();
    }
}

fn parse_palette_color(value: &str) -> Option<u32> {
    let hex = value.trim();
    let hex = hex.strip_prefix('#').unwrap_or(hex);
    (hex.len() == 6 && hex.bytes().all(|byte| byte.is_ascii_hexdigit()))
        .then(|| u32::from_str_radix(hex, 16).ok())
        .flatten()
}
