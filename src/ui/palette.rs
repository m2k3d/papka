//! Редактор цветов оформления.
use super::{
    Message,
    style::{palette_background, rgb},
    widgets::action,
};
use crate::app::App;
use iced::widget::{
    button, checkbox, column, container, row, scrollable, slider, space, text, text_input,
};
use iced::{Element, Fill};

pub(super) fn palette_editor(app: &App) -> Element<'_, Message> {
    let mut editor = column![
        row![
            text("Цвета оформления").size(20).width(Fill),
            action("×", Message::ClosePalette, true)
        ]
        .align_y(iced::Center),
        text("Один цвет или плавный градиент").size(12),
        container(space())
            .height(64)
            .width(Fill)
            .style(move |_| container::Style {
                background: Some(palette_background(app)),
                border: iced::Border {
                    radius: 10.into(),
                    ..Default::default()
                },
                ..Default::default()
            }),
        row![
            action("1 цвет", Message::PaletteCount(1), true),
            action("2 цвета", Message::PaletteCount(2), true),
            action("3 цвета", Message::PaletteCount(3), true)
        ]
        .spacing(4)
    ]
    .spacing(14);
    for index in 0..app.settings.palette_count.clamp(1, 3) as usize {
        let swatch = rgb(app.settings.palette[index]);
        editor = editor.push(
            row![
                container(space())
                    .width(28)
                    .height(28)
                    .style(move |_| container::Style {
                        background: Some(swatch.into()),
                        border: iced::Border {
                            radius: 7.into(),
                            ..Default::default()
                        },
                        ..Default::default()
                    }),
                text_input("RRGGBB", &app.palette_inputs[index])
                    .on_input(move |value| Message::PaletteColor(index, value))
                    .padding(8)
            ]
            .spacing(10),
        );
        let color = app.settings.palette[index];
        for (label, shift) in [("R", 16), ("G", 8), ("B", 0)] {
            editor = editor.push(
                row![
                    text(label).size(11).width(14),
                    slider(0..=255, ((color >> shift) & 255) as u8, move |value| {
                        let next = (color & !(255 << shift)) | ((value as u32) << shift);
                        Message::PaletteColor(index, format!("{next:06X}"))
                    })
                ]
                .spacing(8)
                .align_y(iced::Center),
            );
        }
    }
    editor = editor.push(text("Готовые сочетания").size(12));
    let mut presets = row![].spacing(8);
    for colors in [
        [0xc7c2df, 0xe7c4b5, 0xb8cddd],
        [0xb6d5c6, 0xdbe1b4, 0xb4ced1],
        [0xe5b8c8, 0xb7b7e3, 0xa8cadf],
        [0xb9bec8, 0xd8dce3, 0xc6ccd6],
    ] {
        presets = presets.push(
            button(container(space()).width(48).height(24).style(move |_| {
                container::Style {
                    background: Some(iced::Background::Gradient(
                        iced::gradient::Linear::new(iced::Radians(0.7))
                            .add_stop(0.0, rgb(colors[0]))
                            .add_stop(0.5, rgb(colors[1]))
                            .add_stop(1.0, rgb(colors[2]))
                            .into(),
                    )),
                    border: iced::Border {
                        radius: 6.into(),
                        ..Default::default()
                    },
                    ..Default::default()
                }
            }))
            .padding(2)
            .style(button::text)
            .on_press(Message::Preset(colors)),
        );
    }
    editor = editor
        .push(presets)
        .push(
            checkbox(app.settings.dark)
                .label("Тёмная тема")
                .on_toggle(Message::Dark),
        )
        .push(
            text(
                "HEX: 6 символов. Цвет применяется сразу.
Esc или щелчок снаружи — закрыть.",
            )
            .size(11),
        );
    container(scrollable(editor).height(Fill))
        .padding(22)
        .width(350)
        .height(Fill)
        .max_height(640)
        .style(container::rounded_box)
        .into()
}
