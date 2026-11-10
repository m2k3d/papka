//! Окно и представление состояния средствами Iced.

use iced::widget::{column, container, text};
use iced::{Element, Fill};

use crate::app::App;

#[derive(Debug, Clone)]
enum Message {}

pub fn run() -> iced::Result {
    iced::application(App::default, update, view)
        .title("papka")
        .run()
}

fn update(_app: &mut App, message: Message) {
    match message {}
}

fn view(_app: &App) -> Element<'_, Message> {
    container(
        column![
            text("papka").size(32),
            text("Файловый менеджер"),
            text("Просмотр файлов пока не реализован."),
        ]
        .spacing(12),
    )
    .padding(24)
    .center_x(Fill)
    .center_y(Fill)
    .into()
}
