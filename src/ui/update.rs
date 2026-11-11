//! Преобразование сообщений интерфейса в действия приложения и задачи Iced.
use super::Message;
use crate::{
    app::{App, Dialog},
    files::Command,
};
use iced::Task;

pub(super) fn update(app: &mut App, message: Message) -> Task<Message> {
    match message {
        Message::Context(target) => {
            if !app.open_context(target, false) {
                return Task::none();
            }
        }
        Message::SidebarContext(target) => {
            app.open_context(target, true);
        }
        Message::CloseContext => app.close_context(),
        Message::Pin(path) => app.pin(path),
        Message::Unpin(path) => app.unpin(path),
        Message::MenuAction(action) => {
            app.close_context();
            return update(app, *action);
        }
        Message::EditPath => {
            app.edit_path();
            return iced::widget::operation::focus("path");
        }
        Message::ClosePath => app.close_path(),
        Message::CopyPath => {
            return iced::clipboard::write(app.directory.to_string_lossy().into_owned());
        }
        Message::CopySelection(moving) => app.copy_selection(moving),
        Message::Paste => app.paste(),
        Message::Ready(background) => app.ready(background),
        Message::Event(event) => app.apply(event),
        Message::Failed(error) => app.set_error(error),
        Message::Path(path) => app.set_path_input(path),
        Message::Navigate(path) => app.navigate(path),
        Message::Palette => app.open_palette(),
        Message::ClosePalette => app.close_palette(),
        Message::PaletteCount(count) => app.set_palette_count(count),
        Message::PaletteColor(index, value) => app.set_palette_color(index, value),
        Message::Preset(colors) => app.set_palette(colors),
        Message::Hidden(value) => app.set_hidden(value),
        Message::Dark(value) => app.set_dark(value),
        Message::Reduced(value) => app.set_reduced_motion(value),
        Message::Descending(value) => app.set_descending(value),
        Message::Sort(value) => app.set_sort(value),
        Message::Go => app.go_to_input(),
        Message::Up => app.up(),
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
            if app.rename_selected() {
                return iced::widget::operation::focus("operation-input");
            }
            return Task::none();
        }
        Message::Transfer(moving) => {
            app.begin_dialog(Dialog::Transfer(moving));
            return iced::widget::operation::focus("operation-input");
        }
        Message::Trash => app.execute(Command::Trash(app.selected())),
        Message::Undo(id) => app.execute(Command::Undo(id)),
        Message::Stop => app.stop(),
        Message::Input(input) => app.set_input(input),
        Message::Confirm => app.confirm(),
        Message::Dismiss => app.dismiss(),
        Message::ClearError => app.clear_error(),
        Message::ResolveConflict => {
            app.resolve_conflict();
            return iced::widget::operation::focus("operation-input");
        }
        Message::Settings => app.toggle_settings(),
        Message::Details(id) => app.toggle_history_details(id),
        Message::History => app.toggle_history(),
        Message::Page(next) => app.change_page(next),
        Message::Acknowledge(id) => app.execute(Command::Acknowledge(id)),
        Message::SidebarScrolled => app.sidebar_scrolled(),
        Message::FilesScrolled => app.files_scrolled(),
        Message::Frame => app.advance_frame(),
        Message::Tick => app.refresh_if_idle(),
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
