//! Применение фоновых событий и итогов операций.
use super::{
    App,
    state::{HistoryDirection, PendingListing},
};
use crate::{
    files::Data,
    operations::{Completion, Event, Outcome},
};

impl App {
    pub fn apply(&mut self, event: Event) {
        match event {
            Event::Data {
                id,
                data: Data::ListingBegin(path),
            } if self.loading == Some(id) => {
                self.navigation.pending_listing = Some(PendingListing {
                    id,
                    path,
                    entries: Vec::new(),
                });
            }
            Event::Data {
                id,
                data: Data::ListingBatch(entries),
            } if self.loading == Some(id) => {
                if let Some(pending) = &mut self.navigation.pending_listing {
                    if pending.id == id {
                        pending.entries.extend(entries);
                    }
                }
            }
            Event::Data {
                id,
                data: Data::ListingEnd(warning),
            } if self.loading == Some(id) => {
                if let Some(pending) = self.navigation.pending_listing.take() {
                    self.commit_listing(pending.path, pending.entries, warning);
                }
            }
            Event::Data {
                data: Data::Warning(warning),
                ..
            } => self.error = Some(warning),
            Event::Data {
                data: Data::Conflict(command),
                ..
            } => self.conflict = Some(command),
            Event::Data {
                data: Data::History(records),
                ..
            } => self.history = records,
            Event::Data {
                data: Data::Settings(settings),
                ..
            } => {
                self.settings = settings;
                self.refresh();
            }
            Event::Data { .. } | Event::Started(_) => {}
            Event::Progress {
                id,
                completed,
                total,
            } => {
                if self.mutation.as_ref().is_some_and(|t| t.id == id) {
                    self.status = total.map_or_else(
                        || format!("Записано: {:.1} МиБ", completed as f64 / 1_048_576.),
                        |total| format!("Обработано: {completed} / {total}"),
                    );
                }
            }
            Event::Finished { id, completion } => self.operation_finished(id, completion),
        }
    }

    fn operation_finished(&mut self, id: u64, completion: Completion) {
        let mutation = self.mutation.as_ref().is_some_and(|t| t.id == id);
        let read = self.loading == Some(id);
        let auxiliary = self.work.auxiliary.remove(&id);
        if !read && !mutation && !auxiliary {
            return;
        }
        if read {
            self.loading = None;
            self.navigation.pending_listing = None;
            self.navigation.history_direction = HistoryDirection::Direct;
        }
        if mutation {
            self.mutation = None;
            if !self.closing {
                self.refresh();
            }
        }
        let counts = completion
            .completed
            .zip(completion.remaining)
            .map(|(done, left)| format!("Обработано: {done}; осталось: {left}. "))
            .unwrap_or_default();
        let succeeded = matches!(&completion.outcome, Outcome::Success);
        match completion.outcome {
            Outcome::Failed(error) => {
                if read {
                    self.navigation.read_error = true;
                    self.path_dirty = false;
                    self.path_input = self.directory.to_string_lossy().into_owned();
                }
                self.error = Some(error);
                self.status =
                    format!("{counts}Операция не завершена полностью. Проверьте историю.");
            }
            Outcome::Interrupted if mutation => {
                self.status = format!("{counts}Прерывание завершено. Проверьте историю.")
            }
            Outcome::Success if mutation => {
                self.status = format!("Элементов: {}", self.entries.len());
                self.operation_notice = None;
            }
            Outcome::Success if read && self.navigation.read_error => {
                self.error = None;
                self.navigation.read_error = false;
            }
            _ => {}
        }
        if mutation && !succeeded {
            self.operation_notice = Some(self.status.clone());
        }
    }
}
