//! Маршрутизация клавиатуры, мыши и закрытия окна.
use super::{Message, update};
use crate::app::App;
use iced::{
    Task,
    keyboard::{self, key::Named},
};

pub(super) fn handle(app: &mut App, event: iced::Event) -> Task<Message> {
    match event {
        iced::Event::Mouse(iced::mouse::Event::CursorMoved { position }) => {
            app.set_pointer(position)
        }
        iced::Event::Mouse(iced::mouse::Event::ButtonPressed(button))
            if matches!(
                button,
                iced::mouse::Button::Back | iced::mouse::Button::Forward
            ) =>
        {
            if app.mouse_navigation_available() {
                return update(
                    app,
                    if button == iced::mouse::Button::Back {
                        Message::Back
                    } else {
                        Message::Forward
                    },
                );
            }
        }
        iced::Event::Window(iced::window::Event::CloseRequested) => {
            app.request_close();
        }
        iced::Event::Keyboard(keyboard::Event::ModifiersChanged(modifiers)) => {
            app.selection_modifiers(modifiers.control(), modifiers.shift());
        }
        iced::Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers, .. }) => {
            if app.context_menu.is_some() {
                if key == keyboard::Key::Named(Named::Escape) {
                    app.close_context();
                }
                return Task::none();
            }
            if app.show_path {
                if key == keyboard::Key::Named(Named::Escape) {
                    return update(app, Message::ClosePath);
                }
                return Task::none();
            }
            if app.show_palette {
                if key == keyboard::Key::Named(Named::Escape) {
                    app.close_palette();
                }
                return Task::none();
            }
            let ctrl = modifiers.control();
            let shortcut = match key {
                keyboard::Key::Named(Named::F5) => Some(Message::Refresh),
                keyboard::Key::Named(Named::Escape) => Some(Message::Dismiss),
                keyboard::Key::Named(Named::F2) if app.dialog.is_none() => Some(Message::Rename),
                keyboard::Key::Named(Named::Delete)
                    if app.dialog.is_none() && !app.selection.is_empty() =>
                {
                    Some(Message::Trash)
                }
                keyboard::Key::Named(Named::Enter)
                    if app.dialog.is_none() && app.selection.len() == 1 =>
                {
                    app.selection.first().cloned().map(Message::Open)
                }
                keyboard::Key::Named(Named::ArrowLeft) if modifiers.alt() => Some(Message::Back),
                keyboard::Key::Named(Named::ArrowRight) if modifiers.alt() => {
                    Some(Message::Forward)
                }
                keyboard::Key::Named(Named::ArrowUp) if modifiers.alt() => Some(Message::Up),
                keyboard::Key::Named(Named::ArrowDown) if app.dialog.is_none() => {
                    app.step_selection(true);
                    None
                }
                keyboard::Key::Named(Named::ArrowUp) if app.dialog.is_none() => {
                    app.step_selection(false);
                    None
                }
                keyboard::Key::Character(c) if ctrl => match c.as_str() {
                    "l" => return update(app, Message::EditPath),
                    "r" => Some(Message::Refresh),
                    "h" => Some(Message::Hidden(!app.settings.hidden)),
                    "a" if app.dialog.is_none() => {
                        app.select_all();
                        None
                    }
                    "c" | "x" if app.dialog.is_none() => {
                        Some(Message::CopySelection(c.as_str() == "x"))
                    }
                    "v" if app.dialog.is_none() => Some(Message::Paste),
                    "z" if app.dialog.is_none() => app
                        .history
                        .iter()
                        .rev()
                        .find(|r| r.undo_available)
                        .map(|r| Message::Undo(r.id)),
                    _ => None,
                },
                _ => None,
            };
            if let Some(message) = shortcut {
                return update(app, message);
            }
        }
        _ => {}
    };
    super::update::finish(app)
}
