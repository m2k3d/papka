//! Минималистичный интерфейс Iced.
use crate::{
    app::{App, Dialog},
    files::{Command, Sort},
    operations::{Background, Event},
};
use iced::futures::{SinkExt, Stream};
use iced::widget::{
    button, checkbox, column, container, mouse_area, pick_list, row,
    rule::horizontal as horizontal_rule, scrollable, slider, space, stack, text, text_input,
};
use iced::{
    Element, Fill, Length, Subscription, Task, Theme,
    keyboard::{self, key::Named},
};
use std::path::PathBuf;

#[derive(Debug, Clone)]
enum Message {
    Context(Option<PathBuf>),
    SidebarContext(Option<PathBuf>),
    CloseContext,
    Pin(PathBuf),
    Unpin(PathBuf),
    MenuAction(Box<Message>),
    EditPath,
    ClosePath,
    CopyPath,
    CopySelection(bool),
    Paste,
    Ready(Background),
    Event(Event),
    Failed(String),
    Path(String),
    Go,
    Up,
    Back,
    Forward,
    Refresh,
    Select(PathBuf),
    Open(PathBuf),
    Create(bool),
    Rename,
    Transfer(bool),
    Trash,
    Undo(u64),
    Stop,
    Input(String),
    Confirm,
    Dismiss,
    ClearError,
    Settings,
    Palette,
    ClosePalette,
    PaletteCount(u8),
    PaletteColor(usize, String),
    Preset([u32; 3]),
    Navigate(PathBuf),
    History,
    Hidden(bool),
    Dark(bool),
    Reduced(bool),
    Descending(bool),
    Sort(Sort),
    Details(u64),
    Keyboard(iced::Event),
    Tick,
    Frame,
    FilesScrolled,
    SidebarScrolled,
    Page(bool),
    Acknowledge(u64),
    ResolveConflict,
}
pub fn run() -> iced::Result {
    iced::application(App::default, update, view)
        .title("papka")
        .window_size((1120., 760.))
        .default_font(iced::Font::with_name("DejaVu Sans"))
        .exit_on_close_request(false)
        .theme(|app: &App| theme(app.settings.dark))
        .subscription(subscription)
        .run()
}
fn update(app: &mut App, message: Message) -> Task<Message> {
    match message {
        Message::Context(target) => {
            if app.dialog.is_some() || app.conflict.is_some() || app.show_path || app.show_palette {
                return Task::none();
            }
            if let Some(path) = &target {
                app.select_context(path.clone());
            }
            app.sidebar_context = false;
            app.context_menu = Some((target, app.pointer));
        }
        Message::SidebarContext(target) => {
            if app.dialog.is_none() && app.conflict.is_none() && !app.show_path && !app.show_palette
            {
                app.sidebar_context = true;
                app.context_menu = Some((target, app.pointer));
            }
        }
        Message::CloseContext => app.context_menu = None,
        Message::Pin(path) => {
            if !app.settings.quick_access.contains(&path) {
                app.settings.quick_access.push(path);
                app.persist_settings();
            }
        }
        Message::Unpin(path) => {
            app.settings.quick_access.retain(|p| p != &path);
            app.persist_settings();
        }
        Message::MenuAction(action) => {
            app.context_menu = None;
            return update(app, *action);
        }
        Message::EditPath => {
            app.context_menu = None;
            app.show_path = true;
            app.path_input = app.directory.to_string_lossy().into_owned();
            app.path_dirty = false;
            return iced::widget::operation::focus("path");
        }
        Message::ClosePath => {
            app.show_path = false;
            app.path_dirty = false;
        }
        Message::CopyPath => {
            return iced::clipboard::write(app.directory.to_string_lossy().into_owned());
        }
        Message::CopySelection(moving) => {
            if app.dialog.is_none() {
                app.clipboard = Some((app.selected(), moving));
            }
        }
        Message::Paste => {
            if app.dialog.is_none() {
                if let Some((paths, moving)) = app.clipboard.clone() {
                    let directory = app.directory.clone();
                    app.execute(if moving {
                        Command::Move(paths, directory)
                    } else {
                        Command::Copy(paths, directory)
                    });
                }
            }
        }
        Message::Ready(background) => app.ready(background),
        Message::Event(event) => app.apply(event),
        Message::Failed(error) => app.error = Some(error),
        Message::Path(path) => {
            app.path_input = path;
            app.path_dirty = true;
        }
        Message::Navigate(path) => app.navigate(path),
        Message::Palette => {
            app.show_palette = true;
            app.palette_inputs = app.settings.palette.map(|color| format!("{color:06X}"));
        }
        Message::ClosePalette => app.show_palette = false,
        Message::PaletteCount(count) => {
            app.settings.palette_count = count.clamp(1, 3);
            app.save_settings();
        }
        Message::PaletteColor(index, value) => {
            if index < 3 {
                app.palette_inputs[index] = value.clone();
                let hex = value.trim().trim_start_matches('#');
                if hex.len() == 6 {
                    if let Ok(color) = u32::from_str_radix(hex, 16) {
                        app.settings.palette[index] = color;
                        app.save_settings();
                    }
                }
            }
        }
        Message::Preset(colors) => {
            app.settings.palette = colors;
            app.palette_inputs = colors.map(|color| format!("{color:06X}"));
            app.save_settings();
        }
        Message::Go => {
            app.show_path = false;
            app.navigate(PathBuf::from(&app.path_input));
        }
        Message::Up => {
            if let Some(path) = app.directory.parent() {
                app.navigate(path.to_path_buf());
            }
        }
        Message::Back => app.back(),
        Message::Forward => app.forward(),
        Message::Refresh => app.refresh(),
        Message::Select(path) => app.select(path),
        Message::Open(path) => app.open(path),
        Message::Create(directory) => {
            app.begin_dialog(Dialog::Create(directory));
            return iced::widget::operation::focus("operation-input");
        }
        Message::Rename => {
            if app.selection.len() != 1 {
                return Task::none();
            }
            if let Some(path) = app.selection.first() {
                let path = path.clone();
                app.begin_dialog(Dialog::Rename(path));
                return iced::widget::operation::focus("operation-input");
            }
        }
        Message::Transfer(moving) => {
            app.begin_dialog(Dialog::Transfer(moving));
            return iced::widget::operation::focus("operation-input");
        }
        Message::Trash => app.execute(Command::Trash(app.selected())),
        Message::Undo(id) => app.execute(Command::Undo(id)),
        Message::Stop => {
            if let Some(ticket) = &app.mutation {
                ticket.cancellation.request();
                app.status = "Запрошено прерывание…".into();
            }
        }
        Message::Input(input) => app.input = input,
        Message::Confirm => app.confirm(),
        Message::Dismiss => {
            app.dialog = None;
            app.show_palette = false;
            app.show_path = false;
            app.context_menu = None;
        }
        Message::ClearError => {
            app.error = None;
            app.conflict = None;
        }
        Message::ResolveConflict => {
            app.resolve_conflict();
            return iced::widget::operation::focus("operation-input");
        }
        Message::Settings => {
            app.show_settings = !app.show_settings;
            app.show_history = false;
        }
        Message::Details(id) => {
            app.expanded_history = if app.expanded_history == Some(id) {
                None
            } else {
                Some(id)
            }
        }
        Message::History => {
            app.show_history = !app.show_history;
            app.show_settings = false;
        }
        Message::Hidden(value) => {
            app.settings.hidden = value;
            app.selection.clear();
            app.save_settings();
        }
        Message::Dark(value) => {
            app.settings.dark = value;
            app.save_settings();
        }
        Message::Reduced(value) => {
            app.settings.reduced_motion = value;
            app.save_settings();
        }
        Message::Descending(value) => {
            app.settings.descending = value;
            app.save_settings();
        }
        Message::Sort(value) => {
            app.settings.sort = value;
            app.save_settings();
        }
        Message::Page(next) => {
            if next {
                app.page += 1;
            } else {
                app.page = app.page.saturating_sub(1);
            }
        }
        Message::Acknowledge(id) => app.execute(Command::Acknowledge(id)),
        Message::SidebarScrolled => app.last_sidebar_scroll = Some(std::time::Instant::now()),
        Message::FilesScrolled => app.last_file_scroll = Some(std::time::Instant::now()),
        Message::Frame => {
            if app
                .last_sidebar_scroll
                .is_some_and(|t| t.elapsed().as_millis() >= 1000)
            {
                app.last_sidebar_scroll = None;
            }
            if app
                .last_file_scroll
                .is_some_and(|t| t.elapsed().as_millis() >= 1000)
            {
                app.last_file_scroll = None;
            }
            if app
                .transition
                .is_some_and(|t| t.elapsed().as_millis() >= 180)
            {
                app.transition = None;
            }
        }
        Message::Tick => {
            if app.loading.is_none()
                && app.mutation.is_none()
                && app.dialog.is_none()
                && !app.closing
            {
                app.refresh();
            }
        }
        Message::Keyboard(event) => match event {
            iced::Event::Mouse(iced::mouse::Event::CursorMoved { position }) => {
                app.pointer = position
            }
            iced::Event::Mouse(iced::mouse::Event::ButtonPressed(button))
                if matches!(
                    button,
                    iced::mouse::Button::Back | iced::mouse::Button::Forward
                ) =>
            {
                if app.dialog.is_none()
                    && app.conflict.is_none()
                    && app.context_menu.is_none()
                    && !app.show_path
                    && !app.show_palette
                    && !app.closing
                {
                    return update(
                        app,
                        if button == iced::mouse::Button::Back {
                            Message::Back
                        } else {
                            Message::Forward
                        },
                    );
                }
            }
            iced::Event::Window(iced::window::Event::CloseRequested) => {
                app.closing = true;
                if let Some(ticket) = &app.mutation {
                    ticket.cancellation.request();
                    app.status = "Завершение операции перед закрытием…".into();
                }
            }
            iced::Event::Keyboard(keyboard::Event::ModifiersChanged(modifiers)) => {
                app.selection_control = modifiers.control();
                app.selection_shift = modifiers.shift();
            }
            iced::Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers, .. }) => {
                if app.context_menu.is_some() {
                    if key == keyboard::Key::Named(Named::Escape) {
                        app.context_menu = None;
                    }
                    return Task::none();
                }
                if app.show_path {
                    if key == keyboard::Key::Named(Named::Escape) {
                        return update(app, Message::ClosePath);
                    }
                    return Task::none();
                }
                if app.show_palette {
                    if key == keyboard::Key::Named(Named::Escape) {
                        app.show_palette = false;
                    }
                    return Task::none();
                }
                let ctrl = modifiers.control();
                let shortcut = match key {
                    keyboard::Key::Named(Named::F5) => Some(Message::Refresh),
                    keyboard::Key::Named(Named::Escape) => Some(Message::Dismiss),
                    keyboard::Key::Named(Named::F2) if app.dialog.is_none() => {
                        Some(Message::Rename)
                    }
                    keyboard::Key::Named(Named::Delete)
                        if app.dialog.is_none() && !app.selection.is_empty() =>
                    {
                        Some(Message::Trash)
                    }
                    keyboard::Key::Named(Named::Enter)
                        if app.dialog.is_none() && app.selection.len() == 1 =>
                    {
                        app.selection.first().cloned().map(Message::Open)
                    }
                    keyboard::Key::Named(Named::ArrowLeft) if modifiers.alt() => {
                        Some(Message::Back)
                    }
                    keyboard::Key::Named(Named::ArrowRight) if modifiers.alt() => {
                        Some(Message::Forward)
                    }
                    keyboard::Key::Named(Named::ArrowUp) if modifiers.alt() => Some(Message::Up),
                    keyboard::Key::Named(Named::ArrowDown) if app.dialog.is_none() => {
                        app.step_selection(true);
                        None
                    }
                    keyboard::Key::Named(Named::ArrowUp) if app.dialog.is_none() => {
                        app.step_selection(false);
                        None
                    }
                    keyboard::Key::Character(c) if ctrl => match c.as_str() {
                        "l" => return update(app, Message::EditPath),
                        "r" => Some(Message::Refresh),
                        "h" => Some(Message::Hidden(!app.settings.hidden)),
                        "a" if app.dialog.is_none() => {
                            app.selection = app.visible().iter().map(|e| e.path.clone()).collect();
                            None
                        }
                        "c" | "x" if app.dialog.is_none() => {
                            Some(Message::CopySelection(c.as_str() == "x"))
                        }
                        "v" if app.dialog.is_none() => Some(Message::Paste),
                        "z" if app.dialog.is_none() => app
                            .history
                            .iter()
                            .rev()
                            .find(|r| r.undo_available)
                            .map(|r| Message::Undo(r.id)),
                        _ => None,
                    },
                    _ => None,
                };
                if let Some(message) = shortcut {
                    return update(app, message);
                }
            }
            _ => {}
        },
    }
    if app.closing && app.can_close() {
        return iced::exit();
    }
    Task::none()
}
fn subscription(app: &App) -> Subscription<Message> {
    Subscription::batch([
        Subscription::run(background_events),
        iced::event::listen_with(|event, status, _| {
            if matches!(
                event,
                iced::Event::Mouse(iced::mouse::Event::CursorMoved { .. })
                    | iced::Event::Mouse(iced::mouse::Event::ButtonPressed(
                        iced::mouse::Button::Back | iced::mouse::Button::Forward
                    ))
                    | iced::Event::Keyboard(keyboard::Event::ModifiersChanged(_))
                    | iced::Event::Window(iced::window::Event::CloseRequested)
            ) || status == iced::event::Status::Ignored
            {
                Some(Message::Keyboard(event))
            } else {
                None
            }
        }),
        iced::time::every(std::time::Duration::from_secs(5)).map(|_| Message::Tick),
        if (app.transition.is_some() && !app.settings.reduced_motion)
            || app.last_file_scroll.is_some()
            || app.last_sidebar_scroll.is_some()
        {
            iced::time::every(std::time::Duration::from_millis(16)).map(|_| Message::Frame)
        } else {
            Subscription::none()
        },
    ])
}
fn background_events() -> impl Stream<Item = Message> {
    iced::stream::channel(1, async |mut output| match Background::start() {
        Ok((background, events)) => {
            if output
                .send(Message::Ready(background.clone()))
                .await
                .is_err()
            {
                return;
            }
            while let Ok(event) = events.recv().await {
                if output.send(Message::Event(event)).await.is_err() {
                    break;
                }
            }
            background.shutdown();
        }
        Err(_) => {
            let _ = output
                .send(Message::Failed(
                    "Не удалось запустить фоновые задачи.".into(),
                ))
                .await;
        }
    })
}
fn action<'a>(
    label: &'a str,
    message: Message,
    enabled: bool,
) -> iced::widget::Button<'a, Message> {
    button(text(label).size(14))
        .padding([8, 12])
        .style(button::text)
        .on_press_maybe(enabled.then_some(message))
}
fn view(app: &App) -> Element<'_, Message> {
    let ready = app.mutation.is_none() && !app.closing;
    let idle = ready && app.dialog.is_none() && app.conflict.is_none();
    let title = app
        .directory
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "Файловая система".into());
    let mut content = column![text(title).size(20), horizontal_rule(1)].spacing(18);
    if let Some(error) = &app.error {
        content = content.push(
            container(
                row![
                    text(error).size(14).width(Fill),
                    action("Закрыть", Message::ClearError, true)
                ]
                .spacing(12),
            )
            .padding(12)
            .style(container::bordered_box),
        );
    }
    if app.conflict.is_some() {
        content = content.push(
            row![
                text("Имя занято. Как продолжить?").size(14),
                action(
                    "Изменить имя / папку назначения",
                    Message::ResolveConflict,
                    ready
                ),
                action("Оставить как есть", Message::ClearError, true)
            ]
            .spacing(12),
        );
    }
    if let Some(dialog) = &app.dialog {
        let title = match dialog {
            Dialog::Create(true) => "Новая папка",
            Dialog::Create(false) => "Новый файл",
            Dialog::Rename(_) => "Новое имя",
            Dialog::Transfer(true) => "Переместить в папку",
            Dialog::Transfer(false) => "Копировать в папку",
        };
        content = content.push(
            container(
                column![
                    text(title).size(20),
                    row![
                        text_input("Имя или путь", &app.input)
                            .id("operation-input")
                            .on_input(Message::Input)
                            .on_submit(Message::Confirm)
                            .padding(10),
                        action("Выполнить", Message::Confirm, ready),
                        action("Отмена", Message::Dismiss, true)
                    ]
                    .spacing(8)
                ]
                .spacing(12),
            )
            .padding(20)
            .style(container::bordered_box),
        );
    }
    if app.show_settings {
        content = content.push(container(column![text("Настройки").size(22),
            checkbox(app.settings.hidden).label("Показывать скрытые файлы").on_toggle(Message::Hidden),
            checkbox(app.settings.dark).label("Тёмная тема").on_toggle(Message::Dark),
            checkbox(app.settings.reduced_motion).label("Уменьшить движение").on_toggle(Message::Reduced),
            row![text("Сортировка"), pick_list([Sort::Name, Sort::Size, Sort::Modified], Some(app.settings.sort), Message::Sort), checkbox(app.settings.descending).label("В обратном порядке").on_toggle(Message::Descending)].spacing(16).align_y(iced::Center),
            text("Папки показываются первыми. Ctrl / Shift — множественное выделение. F2 — имя, F5 — обновить. Alt + стрелки — переходы.").size(13),
        ].spacing(16)).padding(20).style(container::bordered_box));
    }
    if app.show_history {
        let mut history = column![text("История · до 100 действий за 30 дней").size(22), text("После сбоя незавершённые действия не повторяются. Отмена проверяет сохранность данных.").size(13)].spacing(12);
        for record in app.history.iter().rev() {
            history = history.push(
                row![
                    text(&record.label).size(14).width(Fill),
                    action(
                        "Отменить действие",
                        Message::Undo(record.id),
                        idle && record.undo_available
                    )
                ]
                .align_y(iced::Center),
            );
            if record.pending {
                history = history.push(action(
                    "Проверено вручную · закрыть запись",
                    Message::Acknowledge(record.id),
                    idle,
                ));
            }
            history = history.push(action("Пути и сведения", Message::Details(record.id), true));
            if app.expanded_history == Some(record.id) {
                for detail in &record.details {
                    history = history.push(text(detail).size(12));
                }
            }
        }
        if app.history.is_empty() {
            history = history.push(text("История пока пуста"));
        }
        content =
            content.push(scrollable(container(history).padding(16)).height(Length::Fixed(200.)));
    }
    let visible = app.visible();
    let page_size = 200;
    let pages = visible.len().div_ceil(page_size).max(1);
    let page = app.page.min(pages - 1);
    let grid = iced::widget::responsive(move |bounds| {
        let gap = 12.0;
        let columns = ((bounds.width + gap) / (132.0 + gap)).floor().max(1.0) as usize;
        let mut rows = column![].spacing(14).width(Fill);
        let mut cards = row![].spacing(gap).width(Fill);
        if visible.is_empty() {
            cards = cards.push(
                container(
                    text(if app.loading.is_some() {
                        "Чтение каталога…"
                    } else {
                        "Папка пуста"
                    })
                    .size(16),
                )
                .padding(32),
            );
        }
        let entries = visible.iter().skip(page * page_size).take(page_size);
        let count = entries.len();
        for (index, entry) in entries.enumerate() {
            let selected = app.selection.contains(&entry.path);
            let icon = file_icon(entry.directory, entry.link);
            let card = column![
                container(icon).center_x(Fill).height(60),
                text(&entry.name)
                    .size(13)
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
            .spacing(8)
            .align_x(iced::Center);
            cards = cards.push(
                mouse_area(
                    button(card)
                        .width(Fill)
                        .padding(12)
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
                cards = cards.push(space().width(Fill));
            }
            rows = rows.push(cards);
        }
        rows.into()
    })
    .height(Length::Shrink);
    content = content.push(
        panel_scrollable(
            grid.into(),
            app.last_file_scroll,
            app.settings.reduced_motion,
        )
        .id("files")
        .on_scroll(|_| Message::FilesScrolled),
    );
    let mut footer = row![
        text(if app.loading.is_some() {
            "Обновление…"
        } else {
            app.operation_notice.as_deref().unwrap_or(&app.status)
        })
        .size(13)
        .width(Fill),
        text(format!("Выделено: {}", app.selection.len())).size(13)
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
        footer = footer.push(action("Прервать", Message::Stop, true));
    }
    content = content.push(horizontal_rule(1)).push(footer);
    let surface = mouse_area(
        container(content)
            .padding(iced::Padding {
                top: 24.0,
                right: 24.0,
                bottom: 16.0,
                left: 24.0,
            })
            .width(Fill)
            .height(Fill)
            .style(|_: &Theme| container::Style {
                background: Some(
                    if app.settings.dark {
                        iced::color!(0x202127)
                    } else {
                        iced::color!(0xfafafd)
                    }
                    .into(),
                ),
                border: iced::Border {
                    radius: 6.into(),
                    ..Default::default()
                },
                ..Default::default()
            }),
    );
    let surface = surface.on_right_press(Message::Context(None));
    let shell = container(row![sidebar(app), surface].spacing(6))
        .padding(8)
        .height(Fill)
        .style(move |_: &Theme| container::Style {
            background: Some(palette_background(app)),
            ..Default::default()
        });
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
                    text("Текущий путь").size(20).width(Fill),
                    action("×", Message::ClosePath, true)
                ],
                text_input("Путь к папке", &app.path_input)
                    .id("path")
                    .on_input(Message::Path)
                    .on_submit(Message::Go)
                    .padding(12),
                row![
                    action("Копировать полный путь", Message::CopyPath, true),
                    action("Перейти", Message::Go, true)
                ]
                .spacing(8)
                .wrap(),
            ]
            .spacing(16),
        )
        .padding(20)
        .width(560)
        .style(container::rounded_box);
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
                    .style(container::rounded_box),
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

fn backdrop(message: Message, dim: bool) -> Element<'static, Message> {
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
fn context_menu<'a>(
    app: &'a App,
    target: Option<&PathBuf>,
    idle: bool,
) -> iced::widget::Column<'a, Message> {
    let item = |label, message, enabled| {
        action(label, Message::MenuAction(Box::new(message)), enabled).width(Fill)
    };
    let mut menu = column![].spacing(2);
    if app.sidebar_context {
        menu = menu.push(item(
            "Закрепить текущую папку",
            Message::Pin(app.directory.clone()),
            !app.settings.quick_access.contains(&app.directory),
        ));
        if let Some(path) = target {
            menu = menu.push(item(
                "Убрать из быстрого доступа",
                Message::Unpin(path.clone()),
                true,
            ));
        } else if !app.settings.quick_access.is_empty() {
            menu = menu.push(horizontal_rule(1));
            for path in &app.settings.quick_access {
                let label = format!(
                    "Убрать: {}",
                    path.file_name()
                        .unwrap_or(path.as_os_str())
                        .to_string_lossy()
                );
                menu = menu.push(
                    button(text(label).size(14))
                        .padding([8, 12])
                        .width(Fill)
                        .style(button::text)
                        .on_press(Message::MenuAction(Box::new(Message::Unpin(path.clone())))),
                );
            }
        }
        return menu;
    }
    if let Some(path) = target {
        menu = menu
            .push(item("Открыть", Message::Open(path.clone()), idle))
            .push(horizontal_rule(1))
            .push(item("Копировать", Message::CopySelection(false), idle))
            .push(item("Вырезать", Message::CopySelection(true), idle))
            .push(item("Копировать в…", Message::Transfer(false), idle))
            .push(item("Переместить в…", Message::Transfer(true), idle))
            .push(item(
                "Переименовать",
                Message::Rename,
                idle && app.selection.len() == 1,
            ))
            .push(horizontal_rule(1))
            .push(item("В корзину", Message::Trash, idle));
        if app
            .entries
            .iter()
            .any(|entry| &entry.path == path && entry.directory)
        {
            let pinned = app.settings.quick_access.contains(path);
            menu = menu.push(item(
                if pinned {
                    "Убрать из быстрого доступа"
                } else {
                    "Закрепить в быстром доступе"
                },
                if pinned {
                    Message::Unpin(path.clone())
                } else {
                    Message::Pin(path.clone())
                },
                true,
            ));
        }
    } else {
        menu = menu
            .push(item("Создать папку", Message::Create(true), idle))
            .push(item("Создать файл", Message::Create(false), idle))
            .push(item(
                "Вставить",
                Message::Paste,
                idle && app
                    .clipboard
                    .as_ref()
                    .is_some_and(|(paths, _)| !paths.is_empty()),
            ))
            .push(horizontal_rule(1));
    }
    menu.push(item("Настройки", Message::Settings, true))
        .push(item("Цвета оформления", Message::Palette, true))
}
fn short_path(path: &std::path::Path, budget: usize) -> String {
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

fn rgb(value: u32) -> iced::Color {
    iced::Color::from_rgb8((value >> 16) as u8, (value >> 8) as u8, value as u8)
}
fn palette_background(app: &App) -> iced::Background {
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
// Размер места под будущие иконки не зависит от временного символа.
fn panel_scrollable<'a>(
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
fn sidebar_icon(name: &str, enabled: bool) -> Element<'_, Message> {
    use iced::widget::svg;
    // SVG включены в бинарник: внешний каталог ресурсов при запуске не нужен.
    let bytes: &[u8] = match name {
        "arrow-left" => include_bytes!("../assets/icons/lucide/arrow-left.svg"),
        "arrow-right" => include_bytes!("../assets/icons/lucide/arrow-right.svg"),
        "arrow-up" => include_bytes!("../assets/icons/lucide/arrow-up.svg"),
        "house" => include_bytes!("../assets/icons/lucide/house.svg"),
        "monitor" => include_bytes!("../assets/icons/lucide/monitor.svg"),
        "file-text" => include_bytes!("../assets/icons/lucide/file-text.svg"),
        "download" => include_bytes!("../assets/icons/lucide/download.svg"),
        "music" => include_bytes!("../assets/icons/lucide/music.svg"),
        "image" => include_bytes!("../assets/icons/lucide/image.svg"),
        "clapperboard" => include_bytes!("../assets/icons/lucide/clapperboard.svg"),
        "hard-drive" => include_bytes!("../assets/icons/lucide/hard-drive.svg"),
        "history" => include_bytes!("../assets/icons/lucide/history.svg"),
        "palette" => include_bytes!("../assets/icons/lucide/palette.svg"),
        "folder" => include_bytes!("../assets/icons/lucide/folder.svg"),
        "plus" => include_bytes!("../assets/icons/lucide/plus.svg"),
        "x" => include_bytes!("../assets/icons/lucide/x.svg"),
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
            .padding(8)
            .style(|theme, status| sidebar_style(theme, status, false))
            .on_press_maybe(enabled.then_some(message)),
        container(text(label).size(12))
            .padding(8)
            .style(container::rounded_box),
        iced::widget::tooltip::Position::Top,
    )
    .into()
}
fn sidebar(app: &App) -> Element<'_, Message> {
    let navigation = row![
        sidebar_tool("arrow-left", "Назад", Message::Back, app.can_back()),
        sidebar_tool("arrow-right", "Вперёд", Message::Forward, app.can_forward()),
        sidebar_tool(
            "arrow-up",
            "Вверх",
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
        .padding([12, 12])
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
    .height(44);
    let section = |label| {
        text(label).size(11).style(|theme: &Theme| text::Style {
            color: Some(theme.palette().text.scale_alpha(0.6)),
        })
    };
    let mut links = column![container(section("Быстрый доступ")).padding([8, 12])].spacing(4);
    for path in &app.settings.quick_access {
        let active = path == &app.directory;
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
            .spacing(10)
            .align_y(iced::Center),
        )
        .width(Fill)
        .padding([10, 12])
        .style(move |theme, status| sidebar_style(theme, status, active))
        .on_press(Message::Navigate(path.clone()));
        links = links.push(
            mouse_area(
                iced::widget::tooltip(
                    destination,
                    container(text(path.to_string_lossy().into_owned()).size(12))
                        .padding(8)
                        .style(container::rounded_box),
                    iced::widget::tooltip::Position::Right,
                )
                .gap(4),
            )
            .on_right_press(Message::SidebarContext(Some(path.clone()))),
        );
    }
    links = links
        .push(space().height(8))
        .push(container(section("Устройства")).padding([8, 12]))
        .push(
            button(row![sidebar_icon("hard-drive", true), text("/").size(13)].spacing(10))
                .width(Fill)
                .padding([10, 12])
                .style(move |theme, status| {
                    sidebar_style(theme, status, app.directory == PathBuf::from("/"))
                })
                .on_press(Message::Navigate(PathBuf::from("/"))),
        );
    let tools = row![
        sidebar_tool("history", "История", Message::History, true),
        space().width(Fill),
        sidebar_tool("palette", "Цвета оформления", Message::Palette, true)
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
    mouse_area(container(panel).padding([12, 10]).width(210).height(Fill))
        .on_right_press(Message::SidebarContext(None))
        .into()
}
fn file_icon(directory: bool, link: bool) -> Element<'static, Message> {
    use iced::widget::svg;
    let bytes: &[u8] = match (directory, link) {
        (true, false) => include_bytes!("../assets/icons/lucide/folder.svg"),
        (true, true) => include_bytes!("../assets/icons/lucide/folder-symlink.svg"),
        (false, false) => include_bytes!("../assets/icons/lucide/file.svg"),
        (false, true) => include_bytes!("../assets/icons/lucide/file-symlink.svg"),
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
fn palette_editor(app: &App) -> Element<'_, Message> {
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

fn theme(dark: bool) -> Theme {
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
