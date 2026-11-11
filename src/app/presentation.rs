//! Меню, редактор пути, дополнительные панели и временные состояния интерфейса.
use super::App;
use std::{path::PathBuf, time::Instant};

impl App {
    pub fn context_available(&self) -> bool {
        self.dialog.is_none() && self.conflict.is_none() && !self.show_path && !self.show_palette
    }

    pub fn mouse_navigation_available(&self) -> bool {
        self.context_available() && self.context_menu.is_none() && !self.closing
    }

    pub fn open_context(&mut self, target: Option<PathBuf>, sidebar: bool) -> bool {
        if !self.context_available() {
            return false;
        }
        if !sidebar {
            if let Some(path) = &target {
                self.select_context(path.clone());
            }
        }
        self.sidebar_context = sidebar;
        self.context_menu = Some((target, self.pointer));
        true
    }

    pub fn close_context(&mut self) {
        self.context_menu = None;
    }

    pub fn set_pointer(&mut self, position: iced::Point) {
        self.pointer = position;
    }

    pub fn edit_path(&mut self) {
        self.context_menu = None;
        self.show_path = true;
        self.path_input = self.directory.to_string_lossy().into_owned();
        self.path_dirty = false;
    }

    pub fn close_path(&mut self) {
        self.show_path = false;
        self.path_dirty = false;
    }

    pub fn set_path_input(&mut self, path: String) {
        self.path_input = path;
        self.path_dirty = true;
    }

    pub fn toggle_settings(&mut self) {
        self.show_settings = !self.show_settings;
        self.show_history = false;
    }

    pub fn toggle_history(&mut self) {
        self.show_history = !self.show_history;
        self.show_settings = false;
    }

    pub fn toggle_history_details(&mut self, id: u64) {
        self.expanded_history = if self.expanded_history == Some(id) {
            None
        } else {
            Some(id)
        };
    }

    pub fn sidebar_scrolled(&mut self) {
        self.last_sidebar_scroll = Some(Instant::now());
    }

    pub fn files_scrolled(&mut self) {
        self.last_file_scroll = Some(Instant::now());
    }

    pub fn advance_frame(&mut self) {
        if self
            .last_sidebar_scroll
            .is_some_and(|t| t.elapsed().as_millis() >= 1000)
        {
            self.last_sidebar_scroll = None;
        }
        if self
            .last_file_scroll
            .is_some_and(|t| t.elapsed().as_millis() >= 1000)
        {
            self.last_file_scroll = None;
        }
        if self
            .transition
            .is_some_and(|t| t.elapsed().as_millis() >= 180)
        {
            self.transition = None;
        }
    }
}
