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
                    action("Закрыть", Message::ClearError, true)
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
                text("Имя занято. Как продолжить?").size(14),
                action(
                    "Изменить имя / папку назначения",
                    Message::ResolveConflict,
                    ready
                ),
                action("Оставить как есть", Message::ClearError, true)
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
            Dialog::Create(true) => "Новая папка",
            Dialog::Create(false) => "Новый файл",
            Dialog::Rename(_) => "Новое имя",
            Dialog::Transfer(true) => "Переместить в папку",
            Dialog::Transfer(false) => "Копировать в папку",
        };
        content = content.push(
            container(
                column![
                    text(title).size(20),
                    row![
                        text_input("Имя или путь", &app.input)
                            .id("operation-input")
                            .on_input(Message::Input)
                            .on_submit(Message::Confirm)
                            .padding(10),
                        action("Выполнить", Message::Confirm, ready),
                        action("Отмена", Message::Dismiss, true)
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
            text("Настройки").size(22),
            checkbox(app.settings.hidden)
                .label("Показывать скрытые файлы")
                .on_toggle(Message::Hidden),
            checkbox(app.settings.dark)
                .label("Тёмная тема")
                .on_toggle(Message::Dark),
            checkbox(app.settings.reduced_motion)
                .label("Уменьшить движение")
                .on_toggle(Message::Reduced),
            row![
                text("Сортировка"),
                pick_list(
                    [Sort::Name, Sort::Size, Sort::Modified],
                    Some(app.settings.sort),
                    Message::Sort
                ),
                checkbox(app.settings.descending)
                    .label("В обратном порядке")
                    .on_toggle(Message::Descending)
            ]
            .spacing(16)
            .align_y(iced::Center),
            text("Папки показываются первыми. Ctrl / Shift — множественное выделение. F2 — имя, F5 — обновить. Alt + стрелки — переходы.").size(13),
        ]
        .spacing(16),
    )
    .padding(20)
    .style(container::bordered_box)
    .into()
}

pub(super) fn history(app: &App, idle: bool) -> Element<'_, Message> {
    let mut history = column![
        text("История · до 100 действий за 30 дней").size(22),
        text(
            "После сбоя незавершённые действия не повторяются. Отмена проверяет сохранность данных."
        )
        .size(13)
    ]
    .spacing(12);
    for record in app.history.iter().rev() {
        history = history.push(
            row![
                text(&record.label).size(14).width(Fill),
                action(
                    "Отменить действие",
                    Message::Undo(record.id),
                    idle && record.undo_available
                )
            ]
            .align_y(iced::Center),
        );
        if record.pending {
            history = history.push(action(
                "Проверено вручную · закрыть запись",
                Message::Acknowledge(record.id),
                idle,
            ));
        }
        history = history.push(action("Пути и сведения", Message::Details(record.id), true));
        if app.expanded_history == Some(record.id) {
            for detail in &record.details {
                history = history.push(text(detail).size(12));
            }
        }
    }
    if app.history.is_empty() {
        history = history.push(text("История пока пуста"));
    }
    scrollable(container(history).padding(16))
        .height(Length::Fixed(200.))
        .into()
}
