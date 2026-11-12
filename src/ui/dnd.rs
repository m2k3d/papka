//! Жизненный цикл жеста, протокол DND и передача действий файловому движку.
use super::Message;
use crate::{
    app::App,
    files::{Command, Data, DragSnapshot},
    operations::{Event, Outcome},
};
use guth_wayland_dnd::{DndEvent, DropAction};
use iced::{Point, Rectangle, Task};
use std::path::PathBuf;

pub(super) fn available(app: &App) -> bool {
    app.background.is_some()
        && app.mutation.is_none()
        && !app.closing
        && app.dialog.is_none()
        && app.conflict.is_none()
        && app.context_menu.is_none()
        && !app.show_path
        && !app.show_palette
        && !app.show_settings
        && !app.show_history
}
pub(super) fn cancel_preparation(app: &mut App) {
    app.dnd.held = false;
    app.dnd.generation = app.dnd.generation.wrapping_add(1);
    if let Some(ticket) = app.dnd.preparing.take() {
        ticket.cancellation.request();
    }
}
pub(super) fn start(app: &mut App, path: PathBuf) {
    if !available(app) || app.dnd.active || app.dnd.preparing.is_some() {
        return;
    }
    if app.dnd.native.is_none() {
        app.set_error("File dragging requires a Wayland session.".into());
        return;
    }
    if !app.selection.contains(&path) {
        app.select_context(path);
    }
    let paths = app.selected();
    app.dnd.generation = app.dnd.generation.wrapping_add(1);
    let generation = app.dnd.generation;
    app.dnd.held = true;
    if let Some(background) = &app.background {
        match background.submit(move |ctx| {
            let result = DragSnapshot::capture(&paths, ctx).map_err(|error| error.to_string());
            ctx.data(Data::DragPrepared(generation, result));
            crate::app::completion(Ok(()))
        }) {
            Ok(ticket) => app.dnd.preparing = Some(ticket),
            Err(error) => app.set_error(error.to_string()),
        }
    }
}
pub(super) fn background(app: &mut App, event: &Event) {
    match event {
        Event::Data {
            data: Data::DragPrepared(generation, result),
            ..
        } if *generation == app.dnd.generation => {
            app.dnd.preparing = None;
            if !app.dnd.held || !available(app) {
                return;
            }
            match result {
                Ok(snapshot) => {
                    if let Some(native) = &app.dnd.native {
                        native.handle.start_drag(snapshot.paths());
                        app.dnd.snapshot = Some(snapshot.clone());
                        app.dnd.active = true;
                    }
                }
                Err(error) => app.set_error(error.clone()),
            }
        }
        Event::Finished { id, completion } => {
            if app
                .dnd
                .preparing
                .as_ref()
                .is_some_and(|ticket| ticket.id == *id)
            {
                app.dnd.preparing = None;
                app.dnd.held = false;
                if let Outcome::Failed(error) = &completion.outcome {
                    app.set_error(error.clone());
                }
            }
            if let Some((operation, token)) = app.dnd.incoming {
                if operation == *id {
                    if let Some(native) = &app.dnd.native {
                        native
                            .handle
                            .complete_drop(token, matches!(completion.outcome, Outcome::Success));
                    }
                    app.dnd.incoming = None;
                }
            }
            if app.dnd.outgoing_cleanup == Some(*id) {
                app.dnd.outgoing_cleanup = None;
                app.operation_notice = Some(if matches!(completion.outcome, Outcome::Success) {
                    "External move completed. Undo of source cleanup restores originals; external changes stay in place."
                } else {
                    "External copy completed; source cleanup is incomplete. Check the history."
                }.into());
            }
        }
        _ => {}
    }
}
pub(super) fn wake(app: &App) -> Task<Message> {
    let Some(native) = &app.dnd.native else {
        return Task::none();
    };
    let events = native.handle.poll();
    if events.is_empty() {
        return Task::none();
    }
    iced::advanced::widget::operate(super::drop_zone::Collect::default())
        .map(move |zones| Message::DropZones(zones, events.clone()))
}
pub(super) fn events(app: &mut App, zones: Vec<(PathBuf, Rectangle)>, events: Vec<DndEvent>) {
    let target = |x, y| {
        zones
            .iter()
            .rev()
            .find(|(_, bounds)| bounds.contains(Point::new(x, y)))
            .map(|(path, _)| path.clone())
    };
    for event in events {
        match event {
            DndEvent::Hovered { x, y } => {
                if !app.dnd.active {
                    app.dnd.snapshot = None;
                }
                app.dnd.self_hover = app.dnd.active;
                app.dnd.hover = if available(app) { target(x, y) } else { None };
            }
            DndEvent::Left => {
                app.dnd.hover = None;
                app.dnd.self_hover = false;
            }
            DndEvent::Dropped {
                x,
                y,
                paths,
                action,
                completion_id,
            } => {
                app.dnd.hover = None;
                let destination = available(app).then(|| target(x, y)).flatten();
                let own = app
                    .dnd
                    .snapshot
                    .as_ref()
                    .is_some_and(|snapshot| snapshot.paths() == paths);
                if let Some(destination) = destination {
                    // Свой drop выполняется одной обычной операцией с полноценной отменой.
                    let command = if own {
                        if app.selection_control {
                            Command::Copy(paths, destination)
                        } else {
                            Command::Move(paths, destination)
                        }
                    } else if action == DropAction::Move {
                        Command::ReceiveMove(paths, destination)
                    } else {
                        Command::Copy(paths, destination)
                    };
                    app.execute(command);
                    if let Some(token) = completion_id {
                        if !own && let Some(ticket) = &app.mutation {
                            app.dnd.incoming = Some((ticket.id, token));
                        } else if let Some(native) = &app.dnd.native {
                            native.handle.complete_drop(token, false);
                        }
                    }
                } else {
                    if let Some(token) = completion_id
                        && let Some(native) = &app.dnd.native
                    {
                        native.handle.complete_drop(token, false);
                    }
                    app.set_error("Drop files onto a folder or the file area when no operation or dialog is active.".into());
                }
                app.dnd.self_hover = false;
                if !app.dnd.active {
                    app.dnd.snapshot = None;
                }
            }
            DndEvent::DragEnded { moved_paths } => {
                app.dnd.active = false;
                app.dnd.held = false;
                app.dnd.hover = None;
                if moved_paths.is_empty() && app.dnd.self_hover {
                    // Copy может завершиться в протоколе перед доставкой своего Dropped.
                    continue;
                }
                if let Some(snapshot) = app.dnd.snapshot.take()
                    && !moved_paths.is_empty()
                {
                    if snapshot.paths() == moved_paths && available(app) {
                        app.execute(Command::FinishExternalMove(snapshot));
                        app.dnd.outgoing_cleanup = app.mutation.as_ref().map(|ticket| ticket.id);
                    } else {
                        app.set_error("External copy completed; originals were preserved because source cleanup is unavailable.".into());
                    }
                }
            }
            DndEvent::DragStartFailed { reason } => {
                app.dnd.active = false;
                app.dnd.snapshot = None;
                app.set_error(format!("Could not start file drag: {reason}"));
            }
            DndEvent::WorkerUnavailable { reason } => {
                app.dnd.detach();
                app.set_error(format!("File dragging is unavailable: {reason}"));
            }
            DndEvent::HoveredFiles { .. } | DndEvent::DragStarted => {}
        }
    }
}
pub(super) fn notifications(
    wake: &crate::platform::DndWake,
) -> impl iced::futures::Stream<Item = Message> + use<> {
    let wake = wake.clone();
    iced::stream::channel(1, async move |mut output| {
        use iced::futures::SinkExt;
        while wake.notified().await {
            if output.send(Message::DndWake).await.is_err() {
                break;
            }
        }
    })
}

/// Не предлагать Move, пока под указателем нет доступной папки назначения.
pub(super) fn sync_actions(app: &mut App) {
    let allowed = available(app) && app.dnd.hover.is_some();
    if allowed != app.dnd.incoming_allowed {
        if let Some(native) = &app.dnd.native {
            native.handle.set_incoming_move_allowed(allowed);
        }
        app.dnd.incoming_allowed = allowed;
    }
}
