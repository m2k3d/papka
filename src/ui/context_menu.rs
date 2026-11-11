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
            "Закрепить текущую папку",
            Message::Pin(app.directory.clone()),
            !app.settings.quick_access.contains(&app.directory),
        ));
        if let Some(path) = target {
            menu = menu.push(item(
                "Убрать из быстрого доступа",
                Message::Unpin(path.clone()),
                true,
            ));
        } else if !app.settings.quick_access.is_empty() {
            menu = menu.push(horizontal_rule(1));
            for path in &app.settings.quick_access {
                let label = format!(
                    "Убрать: {}",
                    path.file_name()
                        .unwrap_or(path.as_os_str())
                        .to_string_lossy()
                );
                menu = menu.push(
                    button(text(label).size(14))
                        .padding([8, 12])
                        .width(Fill)
                        .style(button::text)
                        .on_press(Message::MenuAction(Box::new(Message::Unpin(path.clone())))),
                );
            }
        }
        return menu;
    }
    if let Some(path) = target {
        menu = menu
            .push(item("Открыть", Message::Open(path.clone()), idle))
            .push(horizontal_rule(1))
            .push(item("Копировать", Message::CopySelection(false), idle))
            .push(item("Вырезать", Message::CopySelection(true), idle))
            .push(item("Копировать в…", Message::Transfer(false), idle))
            .push(item("Переместить в…", Message::Transfer(true), idle))
            .push(item(
                "Переименовать",
                Message::Rename,
                idle && app.selection.len() == 1,
            ))
            .push(horizontal_rule(1))
            .push(item("В корзину", Message::Trash, idle));
        if app
            .entries
            .iter()
            .any(|entry| &entry.path == path && entry.directory)
        {
            let pinned = app.settings.quick_access.contains(path);
            menu = menu.push(item(
                if pinned {
                    "Убрать из быстрого доступа"
                } else {
                    "Закрепить в быстром доступе"
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
            .push(item("Создать папку", Message::Create(true), idle))
            .push(item("Создать файл", Message::Create(false), idle))
            .push(item(
                "Вставить",
                Message::Paste,
                idle && app
                    .clipboard
                    .as_ref()
                    .is_some_and(|(paths, _)| !paths.is_empty()),
            ))
            .push(horizontal_rule(1));
    }
    menu.push(item("Настройки", Message::Settings, true))
        .push(item("Цвета оформления", Message::Palette, true))
}
