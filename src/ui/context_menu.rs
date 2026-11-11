//! Команды контекстного меню файлов и быстрого доступа.
use super::{Message, widgets::action};
use crate::app::App;
use iced::Fill;
use iced::widget::{button, column, rule::horizontal as horizontal_rule, text};
use std::path::PathBuf;

pub(super) fn context_menu<'a>(
    app: &'a App,
    target: Option<&PathBuf>,
    idle: bool,
) -> iced::widget::Column<'a, Message> {
    let item = |label, message, enabled| {
        action(label, Message::MenuAction(Box::new(message)), enabled).width(Fill)
    };
    let mut menu = column![].spacing(2);
    if app.sidebar_context {
        menu = menu.push(item(
            "Pin current folder",
            Message::Pin(app.directory.clone()),
            !app.settings.quick_access.contains(&app.directory),
        ));
        if let Some(path) = target {
            menu = menu.push(item(
                "Unpin from Quick access",
                Message::Unpin(path.clone()),
                true,
            ));
        } else if !app.settings.quick_access.is_empty() {
            menu = menu.push(horizontal_rule(1));
            for path in &app.settings.quick_access {
                let label = format!(
                    "Unpin: {}",
                    path.file_name()
                        .unwrap_or(path.as_os_str())
                        .to_string_lossy()
                );
                menu = menu.push(
                    button(text(label).size(14))
                        .padding([8, 12])
                        .width(Fill)
                        .style(super::style::action_button)
                        .on_press(Message::MenuAction(Box::new(Message::Unpin(path.clone())))),
                );
            }
        }
        return menu
            .push(horizontal_rule(1))
            .push(item("Change colors", Message::Palette, true));
    }
    if let Some(path) = target {
        menu = menu
            .push(item("Open", Message::Open(path.clone()), idle))
            .push(horizontal_rule(1))
            .push(item("Copy", Message::CopySelection(false), idle))
            .push(item("Cut", Message::CopySelection(true), idle))
            .push(item("Copy to…", Message::Transfer(false), idle))
            .push(item("Move to…", Message::Transfer(true), idle))
            .push(item(
                "Rename",
                Message::Rename,
                idle && app.selection.len() == 1,
            ))
            .push(horizontal_rule(1))
            .push(item("Move to Trash", Message::Trash, idle));
        if app
            .entries
            .iter()
            .any(|entry| &entry.path == path && entry.directory)
        {
            let pinned = app.settings.quick_access.contains(path);
            menu = menu.push(item(
                if pinned {
                    "Unpin from Quick access"
                } else {
                    "Pin to Quick access"
                },
                if pinned {
                    Message::Unpin(path.clone())
                } else {
                    Message::Pin(path.clone())
                },
                true,
            ));
        }
    } else {
        menu = menu
            .push(item("New folder", Message::Create(true), idle))
            .push(item("New file", Message::Create(false), idle))
            .push(item(
                "Paste",
                Message::Paste,
                idle && app
                    .clipboard
                    .as_ref()
                    .is_some_and(|(paths, _)| !paths.is_empty()),
            ))
            .push(horizontal_rule(1));
    }
    menu.push(item("Settings", Message::Settings, true))
        .push(item("Appearance colors", Message::Palette, true))
}
