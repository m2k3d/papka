//! Встроенные официальные SVG Google Material Icons Round.
use super::Message;
use iced::widget::button;
use iced::{Element, Theme};

pub(super) fn sidebar_icon(name: &str, enabled: bool) -> Element<'_, Message> {
    use iced::widget::svg;
    // SVG включены в бинарник: внешний каталог ресурсов при запуске не нужен.
    let bytes: &[u8] = match name {
        "arrow-left" => include_bytes!("../../assets/icons/material/arrow-left.svg"),
        "arrow-right" => include_bytes!("../../assets/icons/material/arrow-right.svg"),
        "arrow-up" => include_bytes!("../../assets/icons/material/arrow-up.svg"),
        "house" => include_bytes!("../../assets/icons/material/house.svg"),
        "monitor" => include_bytes!("../../assets/icons/material/monitor.svg"),
        "file-text" => include_bytes!("../../assets/icons/material/file-text.svg"),
        "download" => include_bytes!("../../assets/icons/material/download.svg"),
        "music" => include_bytes!("../../assets/icons/material/music.svg"),
        "image" => include_bytes!("../../assets/icons/material/image.svg"),
        "clapperboard" => include_bytes!("../../assets/icons/material/clapperboard.svg"),
        "hard-drive" => include_bytes!("../../assets/icons/material/hard-drive.svg"),
        "history" => include_bytes!("../../assets/icons/material/history.svg"),
        "palette" => include_bytes!("../../assets/icons/material/palette.svg"),
        "folder" => include_bytes!("../../assets/icons/material/folder.svg"),
        "plus" => include_bytes!("../../assets/icons/material/plus.svg"),
        "x" => include_bytes!("../../assets/icons/material/x.svg"),
        _ => unreachable!("Unknown sidebar icon"),
    };
    svg(svg::Handle::from_memory(bytes))
        .width(24)
        .height(24)
        .style(move |theme: &Theme, _| svg::Style {
            color: Some(
                button::text(
                    theme,
                    if enabled {
                        button::Status::Active
                    } else {
                        button::Status::Disabled
                    },
                )
                .text_color,
            ),
        })
        .into()
}

pub(super) fn file_icon(directory: bool, link: bool) -> Element<'static, Message> {
    use iced::widget::svg;
    let bytes: &[u8] = if directory {
        include_bytes!("../../assets/icons/material/folder.svg")
    } else {
        include_bytes!("../../assets/icons/material/file.svg")
    };
    let icon = svg(svg::Handle::from_memory(bytes))
        .width(68)
        .height(68)
        .style(move |_: &Theme, _| svg::Style {
            color: Some(if directory {
                iced::color!(0xf2c45e)
            } else {
                iced::color!(0xb9c8e3)
            }),
        });
    if link {
        let badge = svg(svg::Handle::from_memory(include_bytes!(
            "../../assets/icons/material/file-symlink.svg"
        )))
        .width(26)
        .height(26)
        .style(|_: &Theme, _| svg::Style {
            color: Some(iced::color!(0xffffff)),
        });
        iced::widget::stack![
            icon,
            iced::widget::container(badge)
                .width(68)
                .height(68)
                .align_x(iced::Right)
                .align_y(iced::Bottom)
        ]
        .into()
    } else {
        icon.into()
    }
}
