//! Размещение редакторов и контекстного меню поверх окна.
use super::{
    Message,
    context_menu::context_menu,
    palette::palette_editor,
    widgets::{action, backdrop},
};
use crate::app::App;
use iced::widget::{column, container, row, scrollable, stack, text, text_input};
use iced::{Element, Fill};

pub(super) fn show<'a>(
    app: &'a App,
    shell: Element<'a, Message>,
    idle: bool,
) -> Element<'a, Message> {
    if app.show_palette {
        stack![
            shell,
            backdrop(Message::ClosePalette, true),
            container(iced::widget::opaque(palette_editor(app)))
                .padding(24)
                .width(Fill)
                .height(Fill)
                .align_x(iced::Left)
                .align_y(iced::Bottom)
        ]
        .into()
    } else if app.show_path {
        let editor = container(
            column![
                row![
                    text("Current path").size(20).width(Fill),
                    action("×", Message::ClosePath, true)
                ],
                text_input("Directory path", &app.path_input)
                    .id("path")
                    .on_input(Message::Path)
                    .on_submit(Message::Go)
                    .padding(12),
                row![
                    action("Copy full path", Message::CopyPath, true)
                ]
                .spacing(8)
                .wrap(),
            ]
            .spacing(16),
        )
        .padding(20)
        .width(560)
        .style(super::style::panel);
        stack![
            shell,
            backdrop(Message::ClosePath, true),
            container(iced::widget::opaque(editor))
                .padding(24)
                .width(Fill)
                .height(Fill)
                .align_x(iced::Left)
                .align_y(iced::Top)
        ]
        .into()
    } else if let Some((target, position)) = &app.context_menu {
        let target = target.clone();
        let position = *position;
        let overlay = iced::widget::responsive(move |size| {
            let width = 240.0_f32.min((size.width - 16.0).max(1.0));
            let max_height = (size.height - 16.0).max(1.0);
            let height = if target.is_some() {
                330.0_f32
            } else {
                270.0_f32
            }
            .min(max_height);
            let menu = context_menu(app, target.as_ref(), idle);
            container(iced::widget::opaque(
                container(scrollable(menu))
                    .padding(6)
                    .width(width)
                    .max_height(height)
                    .style(super::style::panel),
            ))
            .padding(iced::Padding {
                left: position.x.clamp(8.0, (size.width - width - 8.0).max(8.0)),
                top: position.y.clamp(8.0, (size.height - height - 8.0).max(8.0)),
                right: 0.0,
                bottom: 0.0,
            })
            .width(Fill)
            .height(Fill)
            .into()
        });
        stack![shell, backdrop(Message::CloseContext, false), overlay].into()
    } else {
        shell.into()
    }
}
