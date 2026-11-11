//! Обработка команд интерфейса и завершение обновления.
use super::Message;
use crate::{
    app::{App, Dialog},
    files::Command,
};
use iced::Task;
use std::path::PathBuf;

pub(super) fn update(app: &mut App, message: Message) -> Task<Message> {
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
        Message::Palette
        | Message::ClosePalette
        | Message::PaletteCount(_)
        | Message::PaletteColor(_, _)
        | Message::Preset(_)
        | Message::Hidden(_)
        | Message::Dark(_)
        | Message::Reduced(_)
        | Message::Descending(_)
        | Message::Sort(_) => appearance(app, message),
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
        Message::Keyboard(event) => return super::input::handle(app, event),
    }
    finish(app)
}

pub(super) fn finish(app: &App) -> Task<Message> {
    if app.closing && app.can_close() {
        return iced::exit();
    }
    Task::none()
}

fn appearance(app: &mut App, message: Message) {
    match message {
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
        _ => unreachable!("Сообщение не относится к оформлению"),
    }
}
