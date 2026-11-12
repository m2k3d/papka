//! Редактор цветовых точек, готовых оттенков и пространственных сочетаний.
use super::{Message, icons::sidebar_icon, palette_field::Field, style::rgb};
use crate::app::App;
use iced::widget::{
    button, canvas, column, container, row, scrollable, slider, space, text, text_input,
};
use iced::{Element, Fill};

pub(super) fn palette_editor(app: &App) -> Element<'_, Message> {
    let count = app.settings.palette_count.clamp(1, 3);
    let active = app.palette_active.min(count as usize - 1);
    let valid_hex = app.palette_input_valid(active);
    let combination = app.settings.harmony().to_string();
    let icon_button = |name, message, enabled| {
        button(sidebar_icon(name, enabled))
            .padding(8)
            .style(super::style::action_button)
            .on_press_maybe(enabled.then_some(message))
    };
    let mut points = row![].spacing(8);
    for index in 0..count as usize {
        let color = rgb(app.settings.palette[index]);
        let label_color = if 0.2126 * color.r + 0.7152 * color.g + 0.0722 * color.b > 0.5 {
            iced::Color::BLACK
        } else {
            iced::Color::WHITE
        };
        points = points.push(
            button(
                container(text((index + 1).to_string()).size(13))
                    .center(28)
                    .style(move |theme: &iced::Theme| container::Style {
                        background: Some(color.into()),
                        text_color: Some(label_color),
                        border: iced::Border {
                            color: if index == active {
                                theme.palette().text
                            } else {
                                iced::Color::TRANSPARENT
                            },
                            width: if index == active { 3.0 } else { 0.0 },
                            radius: 14.into(),
                        },
                        ..Default::default()
                    }),
            )
            .padding(3)
            .style(super::style::action_button)
            .on_press(Message::PaletteSelect(index)),
        );
    }
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
    let color_editor = column![
        row![
            text_input("RRGGBB", &app.palette_inputs[active])
                .padding(8)
                .style(move |theme, status| {
                    let mut style = text_input::default(theme, status);
                    if !valid_hex {
                        style.border.color = theme.palette().danger;
                        style.border.width = 2.0;
                    }
                    style
                })
                .on_input(move |v| Message::PaletteColor(active, v)),
            icon_button(
                "reset",
                Message::Preset(crate::files::Settings::DEFAULT_PALETTE),
                true
            )
        ]
        .spacing(8)
        .align_y(iced::Center)
    ]
    .spacing(4);
    let color_editor = if valid_hex {
        color_editor
    } else {
        color_editor.push(
            text("Enter 6 HEX digits (RRGGBB). Color not applied.")
                .size(12)
                .color(super::style::theme(app.settings.dark).palette().danger),
        )
    };
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
        canvas(Field {
            settings: &app.settings,
            active,
            editor: false
        })
        .width(Fill)
        .height(48),
        points,
        row![
            button(sidebar_icon("plus", count < 3))
                .padding(8)
                .style(super::style::action_button)
                .on_press_maybe((count < 3).then_some(Message::PaletteCount(count + 1))),
            button(sidebar_icon("minus", count > 1))
                .padding(8)
                .style(super::style::action_button)
                .on_press_maybe((count > 1).then_some(Message::PaletteCount(count - 1))),
            button(
                row![
                    sidebar_icon("combination", count > 1),
                    text(combination).size(13)
                ]
                .spacing(6)
                .align_y(iced::Center)
            )
            .padding(8)
            .style(super::style::action_button)
            .on_press_maybe((count > 1).then_some(Message::PaletteCombination))
        ]
        .spacing(8)
        .align_y(iced::Center),
        presets,
        color_editor,
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
