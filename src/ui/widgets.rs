//! Общие кнопки и подложки интерфейса.
use super::Message;
use iced::widget::{button, container, mouse_area, space, text};
use iced::{Element, Fill};

pub(super) fn action<'a>(
    label: &'a str,
    message: Message,
    enabled: bool,
) -> iced::widget::Button<'a, Message> {
    button(text(label).size(14))
        .padding([8, 12])
        .style(button::text)
        .on_press_maybe(enabled.then_some(message))
}

pub(super) fn backdrop(message: Message, dim: bool) -> Element<'static, Message> {
    iced::widget::opaque(
        mouse_area(
            container(space())
                .width(Fill)
                .height(Fill)
                .style(move |_| container::Style {
                    background: dim.then(|| iced::Color::BLACK.scale_alpha(0.25).into()),
                    ..Default::default()
                }),
        )
        .on_press(message.clone())
        .on_right_press(message),
    )
}
