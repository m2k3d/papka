//! Чтение каталога и история переходов.
use super::{App, completion, state::HistoryDirection};
use crate::files::{self, Entry};
use std::path::PathBuf;

impl App {
    pub fn refresh(&mut self) {
        self.read(self.directory.clone());
    }

    fn read(&mut self, path: PathBuf) {
        let Some(background) = &self.background else {
            return;
        };
        let request = path.clone();
        let settings = self.settings.clone();
        match background.read_latest(move |ctx| {
            completion(files::listing(request.clone(), settings, ctx).map_err(|e| {
                std::io::Error::new(
                    e.kind(),
                    format!("Could not read {}: {e}", request.display()),
                )
            }))
        }) {
            Ok(ticket) => {
                self.navigation.pending_listing = None;
                self.loading = Some(ticket.id);
                self.navigation.requested = Some(path);
            }
            Err(e) => self.error = Some(e.to_string()),
        }
    }

    pub fn navigate(&mut self, path: PathBuf) {
        if self.dialog.is_some() || self.closing {
            return;
        }
        let path = if path.starts_with("~") {
            dirs::home_dir()
                .map(|home| home.join(path.strip_prefix("~").unwrap()))
                .unwrap_or(path)
        } else {
            path
        };
        self.path_dirty = false;
        let path = if path.is_absolute() {
            path
        } else {
            self.directory.join(path)
        };
        self.navigation.history_direction = HistoryDirection::Direct;
        self.read(path);
    }

    pub fn back(&mut self) {
        if self.loading.is_some() || self.dialog.is_some() || self.closing {
            return;
        }
        if let Some(path) = self.navigation.back.last().cloned() {
            self.read_history(path);
            self.navigation.history_direction = HistoryDirection::Back;
        }
    }

    pub fn forward(&mut self) {
        if self.loading.is_some() || self.dialog.is_some() || self.closing {
            return;
        }
        if let Some(path) = self.navigation.forward.last().cloned() {
            self.read_history(path);
            self.navigation.history_direction = HistoryDirection::Forward;
        }
    }

    fn read_history(&mut self, path: PathBuf) {
        self.read(path);
        self.navigation.requested = None;
    }

    pub fn can_back(&self) -> bool {
        !self.navigation.back.is_empty() && self.loading.is_none()
    }

    pub fn can_forward(&self) -> bool {
        !self.navigation.forward.is_empty() && self.loading.is_none()
    }

    pub(super) fn commit_listing(
        &mut self,
        path: PathBuf,
        entries: Vec<Entry>,
        warning: Option<String>,
    ) {
        if path != self.directory {
            if self.navigation.history_direction == HistoryDirection::Back {
                self.navigation.back.pop();
                self.navigation.forward.push(self.directory.clone());
            } else if self.navigation.history_direction == HistoryDirection::Forward {
                self.navigation.forward.pop();
                self.navigation.back.push(self.directory.clone());
            } else if self.navigation.requested.is_some() {
                self.navigation.back.push(self.directory.clone());
                self.navigation.forward.clear();
            }
            self.directory = path;
            self.path_dirty = false;
            if !self.settings.reduced_motion {
                self.transition = Some(std::time::Instant::now());
            }
            self.selection.clear();
            self.gestures.anchor = None;
            self.gestures.cursor = None;
            self.page = 0;
        }
        if !self.path_dirty {
            self.path_input = self.directory.to_string_lossy().into_owned();
        }
        self.entries = entries;
        let present: std::collections::HashSet<_> = self.entries.iter().map(|e| &e.path).collect();
        self.selection.retain(|p| present.contains(p));
        self.status = warning.unwrap_or_else(|| format!("Items: {}", self.entries.len()));
    }

    pub fn visible(&self) -> Vec<&Entry> {
        self.entries
            .iter()
            .filter(|e| self.settings.hidden || !e.hidden)
            .collect()
    }

    pub fn go_to_input(&mut self) {
        self.show_path = false;
        self.navigate(PathBuf::from(&self.path_input));
    }

    pub fn up(&mut self) {
        if let Some(path) = self.directory.parent() {
            self.navigate(path.to_path_buf());
        }
    }

    pub fn refresh_if_idle(&mut self) {
        if self.loading.is_none()
            && self.mutation.is_none()
            && self.dialog.is_none()
            && !self.closing
        {
            self.refresh();
        }
    }
}
