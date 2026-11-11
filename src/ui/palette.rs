//! Редактор цветовых точек, готовых оттенков и пространственных сочетаний.
use super::{Message, icons::sidebar_icon, palette_field::Field, style::rgb};
use crate::app::App;
use iced::widget::{button, canvas, column, container, row, scrollable, slider, space, text_input};
use iced::{Element, Fill};

pub(super) fn palette_editor(app: &App) -> Element<'_, Message> {
    let count = app.settings.palette_count.clamp(1, 3);
    let active = app.palette_active.min(count as usize - 1);
    let icon_button = |name, message, enabled| {
        button(sidebar_icon(name, enabled))
            .padding(8)
            .style(super::style::action_button)
            .on_press_maybe(enabled.then_some(message))
    };
    let mut presets = row![].spacing(6);
    for color in [
        0xf5efdc, 0xefb5cd, 0xe7b4e4, 0xd77388, 0xef896f, 0xe4d47f, 0x50dfad, 0x929fb9,
    ] {
        presets = presets.push(
            button(
                container(space())
                    .width(26)
                    .height(26)
                    .style(move |_| container::Style {
                        background: Some(rgb(color).into()),
                        border: iced::Border {
                            radius: 13.into(),
                            ..Default::default()
                        },
                        ..Default::default()
                    }),
            )
            .padding(3)
            .style(super::style::action_button)
            .on_press(Message::PaletteColor(active, format!("{color:06X}"))),
        );
    }
    let editor = column![
        row![
            icon_button("sun", Message::Dark(false), app.settings.dark),
            icon_button("moon", Message::Dark(true), !app.settings.dark),
            space().width(Fill),
            icon_button("x", Message::ClosePalette, true)
        ]
        .spacing(6)
        .align_y(iced::Center),
        canvas(Field {
            settings: &app.settings,
            active,
            editor: true
        })
        .width(Fill)
        .height(300),
        row![
            button(sidebar_icon("plus", count < 3))
                .padding(8)
                .style(super::style::action_button)
                .on_press_maybe((count < 3).then_some(Message::PaletteCount(count + 1))),
            button(sidebar_icon("minus", count > 1))
                .padding(8)
                .style(super::style::action_button)
                .on_press_maybe((count > 1).then_some(Message::PaletteCount(count - 1))),
            icon_button("combination", Message::PaletteCombination, count > 1)
        ]
        .spacing(8)
        .align_y(iced::Center),
        presets,
        row![
            text_input("RRGGBB", &app.palette_inputs[active])
                .padding(8)
                .on_input(move |v| Message::PaletteColor(active, v)),
            icon_button(
                "reset",
                Message::Preset([0xc7c2df, 0xe7c4b5, 0xb8cddd]),
                true
            )
        ]
        .spacing(8)
        .align_y(iced::Center),
        slider(
            0.0..=1.0,
            app.settings.palette_intensity.clamp(0.0, 1.0),
            Message::PaletteIntensity
        )
        .step(0.01_f32)
        .on_release(Message::PaletteCommit)
    ]
    .spacing(12);
    container(scrollable(editor))
        .padding(20)
        .width(370)
        .max_height(660)
        .style(super::style::panel)
        .into()
}
