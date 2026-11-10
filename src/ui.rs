//! Минималистичный интерфейс Iced.
use crate::{
    app::{App, Dialog},
    files::{Command, Sort},
    operations::{Background, Event},
};
use iced::futures::{SinkExt, Stream};
use iced::widget::{
    button, checkbox, column, container, pick_list, row, rule::horizontal as horizontal_rule,
    scrollable, space, text, text_input,
};
use iced::{
    Element, Fill, Length, Subscription, Task, Theme,
    keyboard::{self, key::Named},
};
use std::path::PathBuf;

#[derive(Debug, Clone)]
enum Message {
    Ready(Background),
    Event(Event),
    Failed(String),
    Path(String),
    Go,
    Up,
    Back,
    Forward,
    Home,
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
    Page(bool),
    Acknowledge(u64),
    ResolveConflict,
}
pub fn run() -> iced::Result {
    iced::application(App::default, update, view)
        .title("papka")
        .window_size((1080., 740.))
        .exit_on_close_request(false)
        .theme(|app: &App| theme(app.settings.dark))
        .subscription(subscription)
        .run()
}
fn update(app: &mut App, message: Message) -> Task<Message> {
    match message {
        Message::Ready(background) => app.ready(background),
        Message::Event(event) => app.apply(event),
        Message::Failed(error) => app.error = Some(error),
        Message::Path(path) => {
            app.path_input = path;
            app.path_dirty = true;
        }
        Message::Go => app.navigate(PathBuf::from(&app.path_input)),
        Message::Home => {
            if let Some(path) = dirs::home_dir() {
                app.navigate(path);
            }
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
        Message::Dismiss => app.dialog = None,
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
        Message::Frame => {
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
                        "l" => return iced::widget::operation::focus("path"),
                        "h" => Some(Message::Hidden(!app.settings.hidden)),
                        "a" if app.dialog.is_none() => {
                            app.selection = app.visible().iter().map(|e| e.path.clone()).collect();
                            None
                        }
                        "c" | "x" if app.dialog.is_none() => {
                            app.clipboard = Some((app.selected(), c.as_str() == "x"));
                            None
                        }
                        "v" if app.dialog.is_none() => {
                            if let Some((paths, moving)) = app.clipboard.clone() {
                                let directory = app.directory.clone();
                                app.execute(if moving {
                                    Command::Move(paths, directory)
                                } else {
                                    Command::Copy(paths, directory)
                                });
                            }
                            None
                        }
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
                iced::Event::Keyboard(keyboard::Event::ModifiersChanged(_))
                    | iced::Event::Window(iced::window::Event::CloseRequested)
            ) || status == iced::event::Status::Ignored
            {
                Some(Message::Keyboard(event))
            } else {
                None
            }
        }),
        iced::time::every(std::time::Duration::from_secs(5)).map(|_| Message::Tick),
        if app.transition.is_some() && !app.settings.reduced_motion {
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
    let selected = !app.selection.is_empty();
    let navigation = row![
        text("papka").size(26),
        space().width(12),
        action("←", Message::Back, app.can_back()),
        action("→", Message::Forward, app.can_forward()),
        action("↑", Message::Up, app.directory.parent().is_some()),
        action("Домой", Message::Home, true),
        text_input("Путь к папке", &app.path_input)
            .id("path")
            .on_input(Message::Path)
            .on_submit(Message::Go)
            .padding(10),
        action("Обновить", Message::Refresh, true),
        action("История", Message::History, true),
        action("Настройки", Message::Settings, true),
    ]
    .spacing(4)
    .align_y(iced::Center);
    let toolbar = row![
        action("+ Файл", Message::Create(false), idle),
        action("+ Папка", Message::Create(true), idle),
        action(
            "Открыть",
            app.selection
                .first()
                .map_or(Message::Refresh, |p| Message::Open(p.clone())),
            selected && idle
        ),
        action("Копировать…", Message::Transfer(false), selected && idle),
        action("Переместить…", Message::Transfer(true), selected && idle),
        action(
            "Переименовать",
            Message::Rename,
            app.selection.len() == 1 && idle
        ),
        action("В корзину", Message::Trash, selected && idle),
    ]
    .spacing(2)
    .align_y(iced::Center);
    let mut content = column![navigation, toolbar, horizontal_rule(1)].spacing(12);
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
    let mut list = column![
        row![
            text("Имя").width(Fill),
            text("Размер").width(110),
            text("Действие").width(100)
        ]
        .padding([8, 12]),
        horizontal_rule(1)
    ]
    .spacing(3);
    if visible.is_empty() {
        list = list.push(
            container(
                text(if app.loading.is_some() {
                    "Чтение каталога…"
                } else {
                    "Папка пуста"
                })
                .size(18),
            )
            .padding(32),
        );
    }
    for entry in visible.iter().skip(page * page_size).take(page_size) {
        let kind = if entry.link {
            "↗"
        } else if entry.directory {
            "▸"
        } else {
            "·"
        };
        let label = format!("{kind}  {}", entry.name);
        let selected = app.selection.contains(&entry.path);
        list = list.push(
            row![
                button(text(label).size(16).color({
                    let mut color = theme(app.settings.dark).palette().text;
                    if !app.settings.reduced_motion {
                        if let Some(start) = app.transition {
                            color.a = 0.65 + 0.35 * (start.elapsed().as_secs_f32() / 0.18).min(1.);
                        }
                    }
                    color
                }))
                .width(Fill)
                .padding([10, 12])
                .style(move |theme: &Theme, status| {
                    let mut style = button::text(theme, status);
                    if selected {
                        let mut color = theme.palette().primary;
                        color.a = 0.15;
                        style.background = Some(color.into());
                        style.border.radius = 6.into();
                    }
                    style
                })
                .on_press(Message::Select(entry.path.clone())),
                text(if entry.directory {
                    "—".into()
                } else {
                    size(entry.size)
                })
                .size(13)
                .width(110),
                action(
                    if entry.directory {
                        "Войти"
                    } else {
                        "Открыть"
                    },
                    Message::Open(entry.path.clone()),
                    idle
                )
                .width(100),
            ]
            .spacing(8)
            .align_y(iced::Center),
        );
    }
    content = content.push(scrollable(list).id("files").height(Fill));
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
    container(content)
        .padding(24)
        .width(Fill)
        .height(Fill)
        .into()
}
fn size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{bytes} Б")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} КиБ", bytes as f64 / 1024.)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.1} МиБ", bytes as f64 / 1048576.)
    } else {
        format!("{:.1} ГиБ", bytes as f64 / 1073741824.)
    }
}

fn theme(dark: bool) -> Theme {
    use std::sync::LazyLock;
    static LIGHT: LazyLock<Theme> = LazyLock::new(|| {
        Theme::custom(
            "papka",
            iced::theme::Palette {
                background: iced::color!(0xf5f5f0),
                text: iced::color!(0x303a35),
                primary: iced::color!(0x526d5b),
                success: iced::color!(0x526d5b),
                warning: iced::color!(0x947441),
                danger: iced::color!(0xa0524b),
            },
        )
    });
    static DARK: LazyLock<Theme> = LazyLock::new(|| {
        Theme::custom(
            "papka · тёмная",
            iced::theme::Palette {
                background: iced::color!(0x222824),
                text: iced::color!(0xe1e6de),
                primary: iced::color!(0x9bb59e),
                success: iced::color!(0x9bb59e),
                warning: iced::color!(0xc4a575),
                danger: iced::color!(0xd38c81),
            },
        )
    });
    if dark { DARK.clone() } else { LIGHT.clone() }
}
