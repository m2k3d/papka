//! Общая тема, фон и оформление прокрутки.
use super::Message;
use crate::app::App;
use iced::widget::scrollable;
use iced::{Element, Fill, Theme};

pub(super) fn rgb(value: u32) -> iced::Color {
    iced::Color::from_rgb8((value >> 16) as u8, (value >> 8) as u8, value as u8)
}

pub(super) fn palette_background(app: &App) -> iced::Background {
    let count = app.settings.palette_count.clamp(1, 3) as usize;
    let tint = |color| {
        let c = rgb(color);
        let base = if app.settings.dark { 0.08 } else { 1.0 };
        let strength = if app.settings.dark { 0.28 } else { 0.6 };
        iced::Color::from_rgb(
            base + (c.r - base) * strength,
            base + (c.g - base) * strength,
            base + (c.b - base) * strength,
        )
    };
    if count == 1 {
        return tint(app.settings.palette[0]).into();
    }
    let mut gradient = iced::gradient::Linear::new(iced::Radians(2.2));
    for i in 0..count {
        gradient = gradient.add_stop(i as f32 / (count - 1) as f32, tint(app.settings.palette[i]));
    }
    iced::Background::Gradient(gradient.into())
}

pub(super) fn panel_scrollable<'a>(
    content: Element<'a, Message>,
    last_scroll: Option<std::time::Instant>,
    reduced_motion: bool,
) -> iced::widget::Scrollable<'a, Message> {
    let opacity = last_scroll.map_or(0.0, |start| {
        let elapsed = start.elapsed().as_secs_f32();
        if reduced_motion {
            if elapsed < 1.0 { 1.0 } else { 0.0 }
        } else {
            ((1.0 - elapsed) / 0.2).clamp(0.0, 1.0)
        }
    });
    scrollable(content)
        .width(Fill)
        .height(Fill)
        .direction(scrollable::Direction::Vertical(
            scrollable::Scrollbar::new()
                .width(5)
                .scroller_width(5)
                .margin(0),
        ))
        .style(move |theme: &Theme, status| {
            let mut style = scrollable::default(theme, status);
            let opacity = if matches!(
                status,
                scrollable::Status::Dragged {
                    is_vertical_scrollbar_dragged: true,
                    ..
                }
            ) {
                1.0
            } else {
                opacity
            };
            style.vertical_rail.background = None;
            style.vertical_rail.border = iced::Border::default();
            style.vertical_rail.scroller.background =
                theme.palette().text.scale_alpha(0.35 * opacity).into();
            style.vertical_rail.scroller.border = iced::Border {
                radius: 3.into(),
                ..Default::default()
            };
            style
        })
}

pub(super) fn theme(dark: bool) -> Theme {
    use std::sync::LazyLock;
    static LIGHT: LazyLock<Theme> = LazyLock::new(|| {
        Theme::custom(
            "papka",
            iced::theme::Palette {
                background: iced::color!(0xfafafd),
                text: iced::color!(0x30313a),
                primary: iced::color!(0x77739f),
                success: iced::color!(0x77739f),
                warning: iced::color!(0x947441),
                danger: iced::color!(0xa0524b),
            },
        )
    });
    static DARK: LazyLock<Theme> = LazyLock::new(|| {
        Theme::custom(
            "papka · тёмная",
            iced::theme::Palette {
                background: iced::color!(0x202127),
                text: iced::color!(0xe7e7ee),
                primary: iced::color!(0xb7afd8),
                success: iced::color!(0xb7afd8),
                warning: iced::color!(0xc4a575),
                danger: iced::color!(0xd38c81),
            },
        )
    });
    if dark { DARK.clone() } else { LIGHT.clone() }
}
