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
        self.persist_settings();
    }

    pub fn set_palette_color(&mut self, index: usize, value: String) {
        if index < 3 {
            self.palette_inputs[index] = value.clone();
            let hex = value.trim().trim_start_matches('#');
            if hex.len() == 6 {
                if let Ok(color) = u32::from_str_radix(hex, 16) {
                    self.settings.palette[index] = color;
                    self.persist_settings();
                }
            }
        }
    }

    pub fn set_palette(&mut self, colors: [u32; 3]) {
        self.settings.palette = colors;
        self.palette_inputs = colors.map(|color| format!("{color:06X}"));
        self.persist_settings();
    }

    pub fn cycle_palette_combination(&mut self) {
        self.settings.palette_combination = (self.settings.palette_combination % 4 + 1) % 4;
        self.settings.palette_positions = match self.settings.palette_combination {
            1 => [[0.22, 0.3], [0.5, 0.5], [0.78, 0.7]],
            2 => [[0.25, 0.5], [0.75, 0.5], [0.5, 0.22]],
            3 => [[0.5, 0.22], [0.25, 0.72], [0.75, 0.72]],
            _ => [[0.64, 0.28], [0.25, 0.5], [0.57, 0.76]],
        };
        self.persist_settings();
    }

    pub fn set_hidden(&mut self, value: bool) {
        self.settings.hidden = value;
        self.selection.clear();
        self.save_settings();
    }

    pub fn set_dark(&mut self, value: bool) {
        self.settings.dark = value;
        self.save_settings();
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
