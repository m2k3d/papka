//! Встроенные официальные SVG Lucide.
use super::Message;
use iced::widget::button;
use iced::{Element, Theme};

pub(super) fn sidebar_icon(name: &str, enabled: bool) -> Element<'_, Message> {
    use iced::widget::svg;
    // SVG включены в бинарник: внешний каталог ресурсов при запуске не нужен.
    let bytes: &[u8] = match name {
        "arrow-left" => include_bytes!("../../assets/icons/lucide/arrow-left.svg"),
        "arrow-right" => include_bytes!("../../assets/icons/lucide/arrow-right.svg"),
        "arrow-up" => include_bytes!("../../assets/icons/lucide/arrow-up.svg"),
        "house" => include_bytes!("../../assets/icons/lucide/house.svg"),
        "monitor" => include_bytes!("../../assets/icons/lucide/monitor.svg"),
        "file-text" => include_bytes!("../../assets/icons/lucide/file-text.svg"),
        "download" => include_bytes!("../../assets/icons/lucide/download.svg"),
        "music" => include_bytes!("../../assets/icons/lucide/music.svg"),
        "image" => include_bytes!("../../assets/icons/lucide/image.svg"),
        "clapperboard" => include_bytes!("../../assets/icons/lucide/clapperboard.svg"),
        "hard-drive" => include_bytes!("../../assets/icons/lucide/hard-drive.svg"),
        "history" => include_bytes!("../../assets/icons/lucide/history.svg"),
        "palette" => include_bytes!("../../assets/icons/lucide/palette.svg"),
        "folder" => include_bytes!("../../assets/icons/lucide/folder.svg"),
        "plus" => include_bytes!("../../assets/icons/lucide/plus.svg"),
        "x" => include_bytes!("../../assets/icons/lucide/x.svg"),
        _ => unreachable!("Неизвестная иконка боковой панели"),
    };
    svg(svg::Handle::from_memory(bytes))
        .width(20)
        .height(20)
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
    let bytes: &[u8] = match (directory, link) {
        (true, false) => include_bytes!("../../assets/icons/lucide/folder.svg"),
        (true, true) => include_bytes!("../../assets/icons/lucide/folder-symlink.svg"),
        (false, false) => include_bytes!("../../assets/icons/lucide/file.svg"),
        (false, true) => include_bytes!("../../assets/icons/lucide/file-symlink.svg"),
    };
    svg(svg::Handle::from_memory(bytes))
        .width(52)
        .height(52)
        .style(move |_: &Theme, _| svg::Style {
            color: Some(if directory {
                iced::color!(0xf2c45e)
            } else {
                iced::color!(0xb9c8e3)
            }),
        })
        .into()
}
