//! Выделение, клавиатурный курсор и последовательность щелчков.
use super::App;
use std::path::PathBuf;

impl App {
    /// Правый клик сохраняет группу выделения и прерывает последовательность двойного щелчка.
    pub fn select_context(&mut self, path: PathBuf) {
        self.gestures.last_click = None;
        if !self.selection.contains(&path) {
            self.selection.clear();
            self.selection.insert(path.clone());
        }
        self.gestures.anchor = Some(path.clone());
        self.gestures.cursor = Some(path);
    }

    pub fn select(&mut self, path: PathBuf) {
        if self.dialog.is_some() {
            return;
        }
        let now = std::time::Instant::now();
        if !self.selection_control
            && !self.selection_shift
            && self
                .gestures
                .last_click
                .as_ref()
                .is_some_and(|(p, time)| *p == path && now.duration_since(*time).as_millis() < 350)
        {
            self.gestures.last_click = None;
            self.open(path);
            return;
        }
        self.gestures.last_click = Some((path.clone(), now));
        let visible = self.visible();
        let index = visible.iter().position(|e| e.path == path);
        if self.selection_shift {
            let anchor = self
                .gestures
                .anchor
                .as_ref()
                .and_then(|path| visible.iter().position(|e| &e.path == path));
            if let (Some(a), Some(b)) = (anchor, index) {
                let paths: Vec<_> = visible
                    .iter()
                    .skip(a.min(b))
                    .take(a.max(b) - a.min(b) + 1)
                    .map(|e| e.path.clone())
                    .collect();
                self.gestures.cursor = Some(path);
                self.selection.extend(paths);
                return;
            }
        }
        self.gestures.anchor = Some(path.clone());
        self.gestures.cursor = Some(path.clone());
        if self.selection_control {
            if !self.selection.remove(&path) {
                self.selection.insert(path);
            }
        } else {
            self.selection.clear();
            self.selection.insert(path);
        }
    }

    pub fn step_selection(&mut self, down: bool) {
        if self.dialog.is_some() {
            return;
        }
        let visible = self.visible();
        if visible.is_empty() {
            return;
        }
        let position = self
            .gestures
            .cursor
            .as_ref()
            .and_then(|path| visible.iter().position(|e| &e.path == path));
        let index = position.map_or(0, |i| {
            if down {
                (i + 1).min(visible.len() - 1)
            } else {
                i.saturating_sub(1)
            }
        });
        let path = visible[index].path.clone();
        self.gestures.last_click = None;
        self.select(path);
        self.gestures.last_click = None;
        self.page = index / 200;
    }

    pub fn selected(&self) -> Vec<PathBuf> {
        self.selection.iter().cloned().collect()
    }

    pub fn selection_modifiers(&mut self, control: bool, shift: bool) {
        self.selection_control = control;
        self.selection_shift = shift;
    }

    pub fn select_all(&mut self) {
        self.selection = self
            .visible()
            .iter()
            .map(|entry| entry.path.clone())
            .collect();
    }

    pub fn change_page(&mut self, next: bool) {
        if next {
            self.page += 1;
        } else {
            self.page = self.page.saturating_sub(1);
        }
    }
}
