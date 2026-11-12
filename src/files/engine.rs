//! Загрузка журнала, диспетчер команд и итог фоновой операции.
use super::{Data, can_undo, checkpoint::Checkpoint, history_view::history_items, now};
use crate::{
    history::Journal,
    operations::{Completion, Context, Outcome},
    platform,
};
use std::{fs, io, path::PathBuf};

#[derive(Debug, Clone)]
pub enum Command {
    Create(PathBuf, bool),
    Copy(Vec<PathBuf>, PathBuf),
    Move(Vec<PathBuf>, PathBuf),
    ReceiveMove(Vec<PathBuf>, PathBuf),
    FinishExternalMove(super::DragSnapshot),
    Rename(PathBuf, PathBuf),
    Trash(Vec<PathBuf>),
    Undo(u64),
    Acknowledge(u64),
}
pub struct Engine {
    pub(super) journal: Option<Journal>,
}
impl Engine {
    pub fn new() -> Self {
        Self { journal: None }
    }
    pub fn load(&mut self, ctx: &Context) -> io::Result<()> {
        if self.journal.is_none() {
            self.journal = Some(Journal::open(
                &platform::AppDirectories::resolve()
                    .map_err(io::Error::other)?
                    .history,
                now(),
            )?);
        }
        self.journal.as_mut().unwrap().retain_recent(now())?;
        ctx.data(Data::History(history_items(
            self.journal.as_ref().unwrap().records(),
        )));
        Ok(())
    }
    pub fn run(&mut self, command: Command, ctx: &mut Context) -> Completion {
        let requested = match &command {
            Command::Undo(id) | Command::Acknowledge(id) => Some(*id),
            _ => None,
        };
        let undo = matches!(command, Command::Undo(_));
        if let Err(error) = self.load(ctx) {
            return Completion {
                outcome: Outcome::Failed(error.to_string()),
                completed: None,
                remaining: None,
                undo_available: false,
            };
        }
        let before = self
            .journal
            .as_ref()
            .unwrap()
            .records()
            .iter()
            .map(|r| r.id)
            .max()
            .unwrap_or(0);
        let original = command.clone();
        let result = self.execute(command, ctx);
        let records = self.journal.as_ref().unwrap().records();
        let record = records
            .iter()
            .find(|r| requested == Some(r.id))
            .or_else(|| records.iter().rev().find(|r| r.id > before));
        let counts = record
            .and_then(|r| serde_json::from_slice::<Checkpoint>(&r.checkpoint).ok())
            .map(|cp| {
                let done = cp
                    .items
                    .iter()
                    .filter(|i| if undo { i.undone } else { i.done })
                    .count() as u64;
                let total = if undo {
                    cp.items
                        .iter()
                        .filter(|i| {
                            i.done || (i.started && fs::symlink_metadata(i.target.path()).is_ok())
                        })
                        .count() as u64
                } else {
                    cp.items.len() as u64
                };
                (done, total.saturating_sub(done))
            });
        let undo_available = record.is_some_and(can_undo);
        if result
            .as_ref()
            .is_err_and(|e| e.kind() == io::ErrorKind::AlreadyExists)
        {
            let done: Vec<_> = record
                .and_then(|r| serde_json::from_slice::<Checkpoint>(&r.checkpoint).ok())
                .map(|cp| {
                    cp.items
                        .into_iter()
                        .filter(|i| i.done)
                        .map(|i| i.source.path())
                        .collect()
                })
                .unwrap_or_default();
            let retry = match original {
                Command::Copy(paths, target) | Command::ReceiveMove(paths, target) => {
                    Some(Command::Copy(
                        paths.into_iter().filter(|p| !done.contains(p)).collect(),
                        target,
                    ))
                }
                Command::Move(paths, target) => Some(Command::Move(
                    paths
                        .into_iter()
                        .filter(|p| !done.contains(p) && fs::symlink_metadata(p).is_ok())
                        .collect(),
                    target,
                )),
                command @ (Command::Create(..) | Command::Rename(..)) => Some(command),
                _ => None,
            };
            if let Some(command) = retry {
                ctx.data(Data::Conflict(command));
            }
        }
        ctx.data(Data::History(history_items(records)));
        Completion {
            outcome: match result {
                Ok(()) => Outcome::Success,
                Err(e) if e.kind() == io::ErrorKind::Interrupted => Outcome::Interrupted,
                Err(e) => Outcome::Failed(e.to_string()),
            },
            completed: counts.map(|c| c.0),
            remaining: counts.map(|c| c.1),
            undo_available,
        }
    }
}
