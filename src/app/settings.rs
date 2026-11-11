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
        self.palette_inputs = self.settings.palette.map(|color| format!("{color:06X}"));
    }

    pub fn close_palette(&mut self) {
        self.show_palette = false;
    }

    pub fn set_palette_count(&mut self, count: u8) {
        self.settings.palette_count = count.clamp(1, 3);
        self.save_settings();
    }

    pub fn set_palette_color(&mut self, index: usize, value: String) {
        if index < 3 {
            self.palette_inputs[index] = value.clone();
            let hex = value.trim().trim_start_matches('#');
            if hex.len() == 6 {
                if let Ok(color) = u32::from_str_radix(hex, 16) {
                    self.settings.palette[index] = color;
                    self.save_settings();
                }
            }
        }
    }

    pub fn set_palette(&mut self, colors: [u32; 3]) {
        self.settings.palette = colors;
        self.palette_inputs = colors.map(|color| format!("{color:06X}"));
        self.save_settings();
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
