//! Боковая панель и отображение текущего пути.
use super::{Message, icons::sidebar_icon, style::panel_scrollable};
use crate::app::App;
use iced::widget::{button, column, container, mouse_area, row, space, text};
use iced::{Element, Fill, Theme};
use std::path::PathBuf;

pub(super) fn short_path(path: &std::path::Path, budget: usize) -> String {
    let full = path.to_string_lossy();
    let display = dirs::home_dir()
        .and_then(|home| path.strip_prefix(home).ok())
        .map(|relative| {
            if relative.as_os_str().is_empty() {
                "~".into()
            } else {
                format!("~/{}", relative.display())
            }
        })
        .unwrap_or_else(|| full.into_owned());
    let chars: Vec<_> = display.chars().collect();
    let budget = budget.max(3);
    if chars.len() <= budget {
        display
    } else {
        format!(
            "{}…{}",
            chars[..(budget / 3)].iter().collect::<String>(),
            chars[chars.len() - (budget - budget / 3 - 1)..]
                .iter()
                .collect::<String>()
        )
    }
}

fn sidebar_style(theme: &Theme, status: button::Status, active: bool) -> button::Style {
    let mut style = button::text(theme, status);
    style.border.radius = 8.into();
    let opacity = match status {
        button::Status::Hovered => 0.12,
        button::Status::Pressed => 0.16,
        _ if active => 0.09,
        _ => 0.0,
    };
    style.background = Some(theme.palette().text.scale_alpha(opacity).into());
    style
}
fn sidebar_tool<'a>(
    symbol: &'a str,
    label: &'a str,
    message: Message,
    enabled: bool,
) -> Element<'a, Message> {
    iced::widget::tooltip(
        button(sidebar_icon(symbol, enabled))
            .padding(6)
            .style(|theme, status| sidebar_style(theme, status, false))
            .on_press_maybe(enabled.then_some(message)),
        container(text(label).size(12))
            .padding(8)
            .style(super::style::panel),
        iced::widget::tooltip::Position::Top,
    )
    .into()
}

pub(super) fn sidebar(app: &App) -> Element<'_, Message> {
    let navigation = row![
        sidebar_tool("arrow-left", "Back", Message::Back, app.can_back()),
        sidebar_tool(
            "arrow-right",
            "Forward",
            Message::Forward,
            app.can_forward()
        ),
        sidebar_tool(
            "arrow-up",
            "Up",
            Message::Up,
            app.directory.parent().is_some()
        )
    ]
    .spacing(4);
    let path = iced::widget::responsive(move |size| {
        let budget = ((size.width - 24.0) / 13.0).floor().max(3.0) as usize;
        button(
            text(short_path(&app.directory, budget))
                .size(13)
                .wrapping(text::Wrapping::None),
        )
        .padding([8, 10])
        .width(Fill)
        .style(|theme: &Theme, status| {
            let mut style = sidebar_style(theme, status, false);
            if matches!(status, button::Status::Active) {
                style.background = Some(theme.palette().text.scale_alpha(0.06).into());
            }
            style
        })
        .on_press(Message::EditPath)
        .into()
    })
    .height(36);
    let section = |label| {
        text(label).size(11).style(|theme: &Theme| text::Style {
            color: Some(theme.palette().text.scale_alpha(0.6)),
        })
    };
    let mut links = column![container(section("Quick access")).padding([6, 10])].spacing(2);
    for path in &app.settings.quick_access {
        let active = path == &app.directory || app.dnd.hover.as_ref() == Some(path);
        let symbol = [
            ("house", dirs::home_dir()),
            ("monitor", dirs::desktop_dir()),
            ("file-text", dirs::document_dir()),
            ("download", dirs::download_dir()),
            ("music", dirs::audio_dir()),
            ("image", dirs::picture_dir()),
            ("clapperboard", dirs::video_dir()),
        ]
        .into_iter()
        .find(|(_, candidate)| candidate.as_ref() == Some(path))
        .map_or("folder", |(icon, _)| icon);
        let label = path
            .file_name()
            .unwrap_or(path.as_os_str())
            .to_string_lossy()
            .into_owned();
        let destination = button(
            row![
                sidebar_icon(symbol, true),
                text(label).size(13).wrapping(text::Wrapping::None)
            ]
            .spacing(8)
            .align_y(iced::Center),
        )
        .width(Fill)
        .padding([6, 10])
        .style(move |theme, status| sidebar_style(theme, status, active))
        .on_press(Message::Navigate(path.clone()));
        links = links.push(super::drop_zone::zone(
            mouse_area(
                iced::widget::tooltip(
                    destination,
                    container(text(path.to_string_lossy().into_owned()).size(12))
                        .padding(8)
                        .style(super::style::panel),
                    iced::widget::tooltip::Position::Right,
                )
                .gap(4),
            )
            .on_right_press(Message::SidebarContext(Some(path.clone())))
            .into(),
            None,
            Some(path.clone()),
        ));
    }
    links = links
        .push(space().height(6))
        .push(container(section("Devices")).padding([6, 10]))
        .push(
            button(
                row![sidebar_icon("hard-drive", true), text("/").size(13)]
                    .spacing(8)
                    .align_y(iced::Center),
            )
            .width(Fill)
            .padding([6, 10])
            .style(move |theme, status| {
                sidebar_style(theme, status, app.directory == PathBuf::from("/"))
            })
            .on_press(Message::Navigate(PathBuf::from("/"))),
        );
    let tools = row![
        sidebar_tool("history", "History", Message::History, true),
        space().width(Fill)
    ]
    .spacing(4)
    .align_y(iced::Center);
    let panel = column![
        navigation,
        path,
        space().height(4),
        panel_scrollable(
            links.into(),
            app.last_sidebar_scroll,
            app.settings.reduced_motion
        )
        .on_scroll(|_| Message::SidebarScrolled),
        tools
    ]
    .spacing(8);
    mouse_area(
        container(panel)
            .padding([10, 8])
            .width(sidebar_width(app))
            .height(Fill),
    )
    .on_right_press(Message::SidebarContext(None))
    .into()
}

// Основной области оставляем минимум 240 пикселей, панели — до 480.
pub(super) fn sidebar_width(app: &App) -> f32 {
    let available = (app.window_width - 16.0 - 8.0 - 240.0).max(0.0);
    app.sidebar_width
        .clamp(160.0_f32.min(available), 480.0_f32.min(available))
}
