//! Постановка команд, буфер файлов и завершение приложения.
use super::{App, completion};
use crate::{
    files::{self, Command, Data},
    operations::Background,
};
use std::path::PathBuf;

impl App {
    pub fn ready(&mut self, background: Background) {
        self.background = Some(background.clone());
        let engine = self.work.engine.clone();
        match background.submit(move |ctx| {
            match files::settings_load() {
                Ok(settings) => ctx.data(Data::Settings(settings)),
                Err(e) => ctx.data(Data::Warning(format!(
                    "Could not load settings: {e}"
                ))),
            }
            completion(engine.lock().unwrap_or_else(|e| e.into_inner()).load(ctx))
        }) {
            Ok(t) => self.mutation = Some(t),
            Err(e) => self.error = Some(e.to_string()),
        }
        self.refresh();
    }

    pub fn can_close(&self) -> bool {
        self.mutation.is_none() && self.work.auxiliary.is_empty()
    }

    pub fn open(&mut self, path: PathBuf) {
        if self.entries.iter().any(|e| e.path == path && e.directory) {
            self.navigate(path);
        } else if let Some(background) = &self.background {
            if self.mutation.is_some() {
                return;
            }
            match background.submit(move |_| completion(crate::platform::open_file(&path))) {
                Ok(t) => self.mutation = Some(t),
                Err(e) => self.error = Some(e.to_string()),
            }
        }
    }

    pub fn execute(&mut self, command: Command) {
        if self.closing {
            return;
        }
        if self.conflict.is_some() {
            self.error = Some("Choose how to resolve the name conflict first.".into());
            return;
        }
        if self.mutation.is_some() {
            self.error = Some("Wait for the current operation to finish.".into());
            return;
        }
        let Some(background) = &self.background else {
            return;
        };
        let engine = self.work.engine.clone();
        match background.submit(move |ctx| {
            engine
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .run(command, ctx)
        }) {
            Ok(ticket) => {
                self.mutation = Some(ticket);
                self.error = None;
                self.conflict = None;
                self.operation_notice = None;
                self.status = "Working…".into();
            }
            Err(e) => self.error = Some(e.to_string()),
        }
    }

    pub fn copy_selection(&mut self, moving: bool) {
        if self.dialog.is_none() {
            self.clipboard = Some((self.selected(), moving));
        }
    }

    pub fn paste(&mut self) {
        if self.dialog.is_none() {
            if let Some((paths, moving)) = self.clipboard.clone() {
                let directory = self.directory.clone();
                self.execute(if moving {
                    Command::Move(paths, directory)
                } else {
                    Command::Copy(paths, directory)
                });
            }
        }
    }

    pub fn stop(&mut self) {
        if let Some(ticket) = &self.mutation {
            ticket.cancellation.request();
            self.status = "Stopping…".into();
        }
    }

    pub fn request_close(&mut self) {
        self.closing = true;
        if let Some(ticket) = &self.mutation {
            ticket.cancellation.request();
            self.status = "Finishing the operation before closing…".into();
        }
    }
}
