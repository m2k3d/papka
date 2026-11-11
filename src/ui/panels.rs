//! Уведомления, диалог файловой команды, настройки и история.
use super::{Message, widgets::action};
use crate::{
    app::{App, Dialog},
    files::Sort,
};
use iced::widget::{
    Column, checkbox, column, container, pick_list, row, scrollable, text, text_input,
};
use iced::{Element, Fill, Length};

pub(super) fn notices<'a>(
    app: &'a App,
    ready: bool,
    mut content: Column<'a, Message>,
) -> Column<'a, Message> {
    if let Some(error) = &app.error {
        content = content.push(
            container(
                row![
                    text(error).size(14).width(Fill),
                    action("Close", Message::ClearError, true)
                ]
                .spacing(12),
            )
            .padding(12)
            .style(container::bordered_box),
        );
    }
    if app.conflict.is_some() {
        content = content.push(
            row![
                text("This name is already in use. How would you like to proceed?").size(14),
                action(
                    "Change name / destination",
                    Message::ResolveConflict,
                    ready
                ),
                action("Leave unchanged", Message::ClearError, true)
            ]
            .spacing(12),
        );
    }
    content
}

pub(super) fn operation_dialog<'a>(
    app: &'a App,
    ready: bool,
    mut content: Column<'a, Message>,
) -> Column<'a, Message> {
    if let Some(dialog) = &app.dialog {
        let title = match dialog {
            Dialog::Create(true) => "New folder",
            Dialog::Create(false) => "New file",
            Dialog::Rename(_) => "New name",
            Dialog::Transfer(true) => "Move to folder",
            Dialog::Transfer(false) => "Copy to folder",
        };
        content = content.push(
            container(
                column![
                    text(title).size(20),
                    row![
                        text_input("Name or path", &app.input)
                            .id("operation-input")
                            .on_input(Message::Input)
                            .on_submit(Message::Confirm)
                            .padding(10),
                        action("Confirm", Message::Confirm, ready),
                        action("Cancel", Message::Dismiss, true)
                    ]
                    .spacing(8)
                ]
                .spacing(12),
            )
            .padding(20)
            .style(container::bordered_box),
        );
    }
    content
}

pub(super) fn settings(app: &App) -> Element<'_, Message> {
    container(
        column![
            text("Settings").size(22),
            checkbox(app.settings.hidden)
                .label("Show hidden files")
                .on_toggle(Message::Hidden),
            checkbox(app.settings.dark)
                .label("Dark theme")
                .on_toggle(Message::Dark),
            checkbox(app.settings.reduced_motion)
                .label("Reduce motion")
                .on_toggle(Message::Reduced),
            row![
                text("Sort by"),
                pick_list(
                    [Sort::Name, Sort::Size, Sort::Modified],
                    Some(app.settings.sort),
                    Message::Sort
                ),
                checkbox(app.settings.descending)
                    .label("Reverse order")
                    .on_toggle(Message::Descending)
            ]
            .spacing(16)
            .align_y(iced::Center),
            text("Folders appear first. Ctrl / Shift: select multiple items. F2: rename. F5: refresh. Alt + arrows: navigate.").size(13),
        ]
        .spacing(16),
    )
    .padding(20)
    .style(container::bordered_box)
    .into()
}

pub(super) fn history(app: &App, idle: bool) -> Element<'_, Message> {
    let mut history = column![
        text("History · up to 100 actions over 30 days").size(22),
        text(
            "Incomplete actions are not retried after a crash. Undo checks that your data is unchanged."
        )
        .size(13)
    ]
    .spacing(12);
    for record in app.history.iter().rev() {
        history = history.push(
            row![
                text(&record.label).size(14).width(Fill),
                action(
                    "Undo action",
                    Message::Undo(record.id),
                    idle && record.undo_available
                )
            ]
            .align_y(iced::Center),
        );
        if record.pending {
            history = history.push(action(
                "Reviewed manually · close record",
                Message::Acknowledge(record.id),
                idle,
            ));
        }
        history = history.push(action("Paths and details", Message::Details(record.id), true));
        if app.expanded_history == Some(record.id) {
            for detail in &record.details {
                history = history.push(text(detail).size(12));
            }
        }
    }
    if app.history.is_empty() {
        history = history.push(text("No history yet"));
    }
    scrollable(container(history).padding(16))
        .height(Length::Fixed(200.))
        .into()
}
