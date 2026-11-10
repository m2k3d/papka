//! Состояние и логика приложения, независимые от UI-фреймворка.

use std::collections::BTreeMap;

use crate::operations::{Background, Completion, Event, OperationId, OperationState};

#[derive(Default)]
pub struct App {
    pub background: Option<Background>,
    pub error: Option<String>,
    active: BTreeMap<OperationId, ActiveOperation>,
    last_completion: Option<(OperationId, Completion)>,
}

struct ActiveOperation {
    state: OperationState,
    completed: u64,
    total: Option<u64>,
}

impl App {
    pub fn apply(&mut self, event: Event) {
        match event {
            Event::Started(id) => {
                self.active.insert(id, ActiveOperation {
                    state: OperationState::Running,
                    completed: 0,
                    total: None,
                });
            }
            Event::Progress { id, completed, total } => {
                if let Some(operation) = self.active.get_mut(&id) {
                    operation.completed = completed;
                    operation.total = total;
                }
            }
            Event::Finished { id, completion } => {
                self.active.remove(&id);
                self.last_completion = Some((id, completion));
            }
        }
    }

    pub fn is_busy(&self) -> bool {
        self.active.values().any(|operation| operation.state == OperationState::Running)
    }
}

impl Drop for App {
    fn drop(&mut self) {
        if let Some(background) = &self.background {
            background.shutdown();
        }
    }
}
