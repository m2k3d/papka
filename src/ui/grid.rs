//! Сетка файлов и нижняя строка со страницами и прогрессом.
use super::{
    Message,
    icons::file_icon,
    style::{panel_scrollable, theme},
    widgets::action,
};
use crate::{app::App, files::Entry};
use iced::widget::{button, column, container, mouse_area, row, space, text};
use iced::{Element, Fill, Length, Theme};

pub(super) const PAGE_SIZE: usize = 200;

pub(super) fn file_grid<'a>(
    app: &'a App,
    idle: bool,
    visible: Vec<&'a Entry>,
    page: usize,
) -> Element<'a, Message> {
    let grid = iced::widget::responsive(move |bounds| {
        let gap = 12.0;
        let columns = ((bounds.width + gap) / (132.0 + gap)).floor().max(1.0) as usize;
        let mut rows = column![].spacing(14).width(Fill);
        let mut cards = row![].spacing(gap).width(Fill);
        if visible.is_empty() {
            cards = cards.push(
                container(
                    text(if app.loading.is_some() {
                        "Чтение каталога…"
                    } else {
                        "Папка пуста"
                    })
                    .size(16),
                )
                .padding(32),
            );
        }
        let entries = visible.iter().skip(page * PAGE_SIZE).take(PAGE_SIZE);
        let count = entries.len();
        for (index, entry) in entries.enumerate() {
            let selected = app.selection.contains(&entry.path);
            let icon = file_icon(entry.directory, entry.link);
            let card = column![
                container(icon).center_x(Fill).height(60),
                text(&entry.name)
                    .size(13)
                    .width(Fill)
                    .align_x(iced::Center)
                    .color({
                        let opacity = if app.settings.reduced_motion {
                            1.0
                        } else {
                            app.transition.map_or(1.0, |start| {
                                0.65 + 0.35 * (start.elapsed().as_secs_f32() / 0.18).min(1.0)
                            })
                        };
                        theme(app.settings.dark).palette().text.scale_alpha(opacity)
                    }),
            ]
            .spacing(8)
            .align_x(iced::Center);
            cards = cards.push(
                mouse_area(
                    button(card)
                        .width(Fill)
                        .padding(12)
                        .style(move |theme: &Theme, status| {
                            let mut style = button::text(theme, status);
                            style.border.radius = 12.into();
                            if selected {
                                style.background =
                                    Some(theme.palette().primary.scale_alpha(0.18).into());
                            }
                            style
                        })
                        .on_press_maybe(idle.then(|| Message::Select(entry.path.clone()))),
                )
                .on_right_press(Message::Context(Some(entry.path.clone()))),
            );
            if (index + 1) % columns == 0 {
                rows = rows.push(cards);
                cards = row![].spacing(gap).width(Fill);
            }
        }
        if count == 0 {
            rows = rows.push(cards);
        } else if count % columns != 0 {
            // Неполный ряд сохраняет ширину столбцов остальных рядов.
            for _ in count % columns..columns {
                cards = cards.push(space().width(Fill));
            }
            rows = rows.push(cards);
        }
        rows.into()
    })
    .height(Length::Shrink);
    let files = panel_scrollable(
        grid.into(),
        app.last_file_scroll,
        app.settings.reduced_motion,
    )
    .id("files")
    .on_scroll(|_| Message::FilesScrolled);
    files.into()
}

pub(super) fn footer(app: &App, ready: bool, page: usize, pages: usize) -> Element<'_, Message> {
    let mut footer = row![
        text(if app.loading.is_some() {
            "Обновление…"
        } else {
            app.operation_notice.as_deref().unwrap_or(&app.status)
        })
        .size(13)
        .width(Fill),
        text(format!("Выделено: {}", app.selection.len())).size(13)
    ]
    .spacing(12)
    .align_y(iced::Center);
    if pages > 1 {
        footer = footer
            .push(action("‹", Message::Page(false), page > 0))
            .push(text(format!("{} / {}", page + 1, pages)))
            .push(action("›", Message::Page(true), page + 1 < pages));
    }
    if !ready {
        footer = footer.push(action("Прервать", Message::Stop, true));
    }
    footer.into()
}
