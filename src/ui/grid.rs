//! Сетка файлов и нижняя строка со страницами и прогрессом.
use super::{
    Message,
    icons::file_icon,
    style::{panel_scrollable, theme},
    widgets::action,
};
use crate::{app::App, files::Entry};
use iced::widget::{button, column, container, mouse_area, row, space, text};
use iced::{Element, Fill, Length, Theme};

pub(super) const PAGE_SIZE: usize = 200;

// Измеряем тем же движком и шрифтом, которыми Iced рисует подпись.
fn filename_label(name: &str, directory: bool, width: f32) -> String {
    use iced::advanced::{
        graphics::text::Paragraph,
        text::{Paragraph as _, Text},
    };
    use unicode_segmentation::UnicodeSegmentation;
    let name = name.replace(['\n', '\r', '\t'], " ");
    let fits = |value: &str| {
        let paragraph = Paragraph::with_text(Text {
            content: value,
            bounds: iced::Size::new(f32::INFINITY, f32::INFINITY),
            size: 13.into(),
            line_height: text::LineHeight::Absolute(16.into()),
            font: iced::Font::with_name("CaskaydiaCove Nerd Font"),
            align_x: iced::advanced::text::Alignment::Left,
            align_y: iced::alignment::Vertical::Top,
            shaping: text::Shaping::Advanced,
            wrapping: text::Wrapping::None,
        });
        paragraph.min_width() <= width.max(0.0)
    };
    let prefix = |value: &str, suffix: &str| {
        let boundaries: Vec<_> = value
            .grapheme_indices(true)
            .map(|(i, _)| i)
            .chain(std::iter::once(value.len()))
            .collect();
        let (mut low, mut high) = (0, boundaries.len() - 1);
        while low < high {
            let mid = (low + high).div_ceil(2);
            if fits(&format!("{}{}", &value[..boundaries[mid]], suffix)) {
                low = mid;
            } else {
                high = mid - 1;
            }
        }
        boundaries[low]
    };
    if fits(&name) {
        return name;
    }
    let mut split = prefix(&name, "");
    // Сначала переносим по словам / дефисам, длинное слово — по графемам.
    if let Some((index, ch)) = name[..split]
        .char_indices()
        .rev()
        .find(|(_, ch)| ch.is_whitespace() || *ch == '-' || *ch == '_')
    {
        let boundary = index + ch.len_utf8();
        if boundary >= split / 2 {
            split = boundary;
        }
    }
    let first = name[..split].trim_end();
    let rest = name[split..].trim_start();
    if fits(rest) {
        return format!("{first}\n{rest}");
    }
    let extension = if directory {
        ""
    } else {
        [".tar.gz", ".tar.bz2", ".tar.xz", ".tar.zst"]
            .into_iter()
            .find(|suffix| name.ends_with(suffix))
            .or_else(|| {
                name.rsplit_once('.')
                    .filter(|(stem, ext)| !stem.is_empty() && !ext.is_empty())
                    .map(|(_, ext)| &name[name.len() - ext.len() - 1..])
            })
            .unwrap_or("")
    };
    let suffix = format!("…{extension}");
    let suffix = if fits(&suffix) { suffix } else { "…".into() };
    let stem = rest.strip_suffix(extension).unwrap_or(rest);
    let end = prefix(stem, &suffix);
    format!("{first}\n{}{}", stem[..end].trim_end(), suffix)
}

pub(super) fn file_grid<'a>(
    app: &'a App,
    idle: bool,
    visible: Vec<&'a Entry>,
    page: usize,
) -> Element<'a, Message> {
    let grid = iced::widget::responsive(move |bounds| {
        let gap = 6.0;
        // Добавляем колонку до того, как карточки станут шире компактного размера.
        let columns = ((bounds.width + gap) / (124.0 + gap)).ceil().max(1.0) as usize;
        // В узком окне оставляем место для иконки и внутренних отступов.
        let columns = columns.min(((bounds.width + gap) / (92.0 + gap)).floor().max(1.0) as usize);
        // Распределяем всю доступную ширину поровну, включая неполный ряд.
        let card_width = ((bounds.width - gap * (columns - 1) as f32) / columns as f32).max(0.0);
        let mut rows = column![].spacing(6).width(Fill);
        let mut cards = row![].spacing(gap).width(Fill);
        if visible.is_empty() {
            cards = cards.push(
                container(
                    text(if app.loading.is_some() {
                        "Reading directory…"
                    } else {
                        "This folder is empty"
                    })
                    .size(16),
                )
                .padding(32),
            );
        }
        let entries = visible.iter().skip(page * PAGE_SIZE).take(PAGE_SIZE);
        let count = entries.len();
        for (index, entry) in entries.enumerate() {
            let selected = app.selection.contains(&entry.path);
            let icon = file_icon(entry.directory, entry.link);
            let card = column![
                container(icon).center_x(Fill).height(68),
                text(filename_label(
                    &entry.name,
                    entry.directory,
                    card_width - 12.0
                ))
                .size(13)
                .height(32)
                .line_height(text::LineHeight::Absolute(16.into()))
                .shaping(text::Shaping::Advanced)
                .wrapping(text::Wrapping::None)
                .width(Fill)
                .align_x(iced::Center)
                .color({
                    let opacity = if app.settings.reduced_motion {
                        1.0
                    } else {
                        app.transition.map_or(1.0, |start| {
                            0.65 + 0.35 * (start.elapsed().as_secs_f32() / 0.18).min(1.0)
                        })
                    };
                    theme(app.settings.dark).palette().text.scale_alpha(opacity)
                }),
            ]
            .spacing(4)
            .align_x(iced::Center);
            cards = cards.push(
                mouse_area(
                    button(card)
                        .width(card_width)
                        .padding(6)
                        .style(move |theme: &Theme, status| {
                            let mut style = button::text(theme, status);
                            style.border.radius = 12.into();
                            if selected {
                                style.background =
                                    Some(theme.palette().primary.scale_alpha(0.18).into());
                            }
                            style
                        })
                        .on_press_maybe(idle.then(|| Message::Select(entry.path.clone()))),
                )
                .on_right_press(Message::Context(Some(entry.path.clone()))),
            );
            if (index + 1) % columns == 0 {
                rows = rows.push(cards);
                cards = row![].spacing(gap).width(Fill);
            }
        }
        if count == 0 {
            rows = rows.push(cards);
        } else if count % columns != 0 {
            // Неполный ряд сохраняет ширину столбцов остальных рядов.
            for _ in count % columns..columns {
                cards = cards.push(space().width(card_width));
            }
            rows = rows.push(cards);
        }
        rows.into()
    })
    .height(Length::Shrink);
    let files = panel_scrollable(
        grid.into(),
        app.last_file_scroll,
        app.settings.reduced_motion,
    )
    .id("files")
    .on_scroll(|_| Message::FilesScrolled);
    files.into()
}

pub(super) fn footer(app: &App, ready: bool, page: usize, pages: usize) -> Element<'_, Message> {
    let mut footer = row![
        text(if app.loading.is_some() {
            "Refreshing…"
        } else {
            app.operation_notice.as_deref().unwrap_or(&app.status)
        })
        .size(13)
        .width(Fill),
        text(format!("Selected: {}", app.selection.len())).size(13)
    ]
    .spacing(12)
    .align_y(iced::Center);
    if pages > 1 {
        footer = footer
            .push(action("‹", Message::Page(false), page > 0))
            .push(text(format!("{} / {}", page + 1, pages)))
            .push(action("›", Message::Page(true), page + 1 < pages));
    }
    if !ready {
        footer = footer.push(action("Stop", Message::Stop, true));
    }
    footer.into()
}
