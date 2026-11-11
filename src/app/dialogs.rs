//! Подготовка файловых диалогов, разрешение конфликтов и подтверждение.
use super::{App, Dialog};
use crate::files::Command;
use std::path::PathBuf;

impl App {
    pub fn begin_dialog(&mut self, dialog: Dialog) {
        self.retry = None;
        if self.conflict.is_some() {
            return;
        }
        if self.mutation.is_some() {
            return;
        }
        self.input = match &dialog {
            Dialog::Rename(path) => path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned(),
            Dialog::Transfer(_) => self.directory.to_string_lossy().into_owned(),
            _ => String::new(),
        };
        self.dialog = Some(dialog);
    }

    pub fn resolve_conflict(&mut self) {
        let Some(command) = self.conflict.take() else {
            return;
        };
        let (dialog, input) = match &command {
            Command::Create(path, directory) => (
                Dialog::Create(*directory),
                path.file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned(),
            ),
            Command::Rename(source, target) => (
                Dialog::Rename(source.clone()),
                target
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned(),
            ),
            Command::Copy(_, target) => (
                Dialog::Transfer(false),
                target.to_string_lossy().into_owned(),
            ),
            Command::Move(_, target) => (
                Dialog::Transfer(true),
                target.to_string_lossy().into_owned(),
            ),
            _ => return,
        };
        self.dialog = Some(dialog);
        self.input = input;
        self.retry = Some(command);
    }

    pub fn confirm(&mut self) {
        let Some(dialog) = self.dialog.clone() else {
            return;
        };
        let name = self.input.as_str();
        let command = match dialog {
            Dialog::Create(directory) => {
                if !valid_name(name) {
                    self.error = Some("Enter a name without /, other than . or ..".into());
                    return;
                }
                Command::Create(
                    if let Some(Command::Create(path, _)) = &self.retry {
                        path.with_file_name(name)
                    } else {
                        self.directory.join(name)
                    },
                    directory,
                )
            }
            Dialog::Rename(path) => {
                if !valid_name(name) {
                    self.error = Some("Enter a name without /, other than . or ..".into());
                    return;
                }
                Command::Rename(path.clone(), path.with_file_name(name))
            }
            Dialog::Transfer(moving) => {
                if name.is_empty() {
                    self.error = Some("Enter a destination directory.".into());
                    return;
                }
                let target = PathBuf::from(name);
                let target = if target.is_absolute() {
                    target
                } else {
                    self.directory.join(target)
                };
                let paths = match &self.retry {
                    Some(Command::Copy(paths, _)) | Some(Command::Move(paths, _)) => paths.clone(),
                    _ => self.selected(),
                };
                if moving {
                    Command::Move(paths, target)
                } else {
                    Command::Copy(paths, target)
                }
            }
        };
        self.dialog = None;
        self.execute(command);
    }

    /// Возвращает наличие единственного выбранного пути для установки фокуса в UI.
    pub fn rename_selected(&mut self) -> bool {
        if self.selection.len() != 1 {
            return false;
        }
        if let Some(path) = self.selection.first().cloned() {
            self.begin_dialog(Dialog::Rename(path));
            return true;
        }
        false
    }

    pub fn set_input(&mut self, input: String) {
        self.input = input;
    }

    pub fn dismiss(&mut self) {
        self.dialog = None;
        self.show_palette = false;
        self.show_path = false;
        self.context_menu = None;
    }

    pub fn clear_error(&mut self) {
        self.error = None;
        self.conflict = None;
    }

    pub fn set_error(&mut self, error: String) {
        self.error = Some(error);
    }
}
fn valid_name(name: &str) -> bool {
    !name.is_empty() && name != "." && name != ".." && !name.contains('/') && !name.contains('\0')
}
