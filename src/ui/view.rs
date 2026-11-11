//! Сборка основного окна из компонентов интерфейса.
use super::{Message, grid, panels, sidebar::sidebar, style::palette_background};
use crate::app::App;
use iced::widget::{column, container, mouse_area, row, rule::horizontal as horizontal_rule, text};
use iced::{Element, Fill, Theme};

pub(super) fn view(app: &App) -> Element<'_, Message> {
    let ready = app.mutation.is_none() && !app.closing;
    let idle = ready && app.dialog.is_none() && app.conflict.is_none();
    let title = app
        .directory
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "Filesystem".into());
    let mut content = column![text(title).size(20), horizontal_rule(1)].spacing(12);
    content = panels::notices(app, ready, content);
    content = panels::operation_dialog(app, ready, content);
    if app.show_settings {
        content = content.push(panels::settings(app));
    }
    if app.show_history {
        content = content.push(panels::history(app, idle));
    }
    let visible = app.visible();
    let pages = visible.len().div_ceil(grid::PAGE_SIZE).max(1);
    let page = app.page.min(pages - 1);
    content = content.push(grid::file_grid(app, idle, visible, page));
    content = content
        .push(horizontal_rule(1))
        .push(grid::footer(app, ready, page, pages));
    let surface = mouse_area(
        container(content)
            .padding(iced::Padding {
                top: 18.0,
                right: 18.0,
                bottom: 12.0,
                left: 18.0,
            })
            .width(Fill)
            .height(Fill)
            .style(|_: &Theme| container::Style {
                background: Some(
                    if app.settings.dark {
                        iced::color!(0x202127)
                    } else {
                        iced::color!(0xfafafd)
                    }
                    .into(),
                ),
                border: iced::Border {
                    radius: 6.into(),
                    ..Default::default()
                },
                ..Default::default()
            }),
    );
    let surface = surface.on_right_press(Message::Context(None));
    let shell = container(row![sidebar(app), surface].spacing(6))
        .padding(8)
        .height(Fill)
        .style(move |_: &Theme| container::Style {
            background: Some(palette_background(app)),
            ..Default::default()
        });
    super::overlays::show(app, shell.into(), idle)
}
