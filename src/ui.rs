//! Окно и представление состояния средствами Iced.

use iced::widget::{column, container, text};
use iced::{Element, Fill, Subscription};
use iced::futures::{SinkExt, Stream};

use crate::app::App;
use crate::operations::{Background, Event};

#[derive(Debug, Clone)]
enum Message {
    BackgroundReady(Background),
    BackgroundEvent(Event),
    BackgroundFailed(String),
}

pub fn run() -> iced::Result {
    iced::application(App::default, update, view)
        .title("papka")
        .subscription(subscription)
        .run()
}

fn update(app: &mut App, message: Message) {
    match message {
        Message::BackgroundReady(background) => app.background = Some(background),
        Message::BackgroundEvent(event) => app.apply(event),
        Message::BackgroundFailed(error) => app.error = Some(error),
    }
}

fn subscription(_app: &App) -> Subscription<Message> {
    Subscription::run(background_events)
}

fn background_events() -> impl Stream<Item = Message> {
    iced::stream::channel(1, async |mut output| {
        match Background::start() {
            Ok((background, events)) => {
                if output.send(Message::BackgroundReady(background.clone())).await.is_err() {
                    return;
                }
                while let Ok(event) = events.recv().await {
                    if output.send(Message::BackgroundEvent(event)).await.is_err() {
                        break;
                    }
                }
                background.shutdown();
            }
            Err(_) => {
                let _ = output.send(Message::BackgroundFailed(
                    "Не удалось запустить фоновые задачи. Перезапустите приложение.".into(),
                )).await;
            }
        }
    })
}

fn view(app: &App) -> Element<'_, Message> {
    let mut content = column![
        text("papka").size(32),
        text("Файловый менеджер"),
        text("Просмотр файлов пока не реализован."),
    ].spacing(12);
    if let Some(error) = &app.error {
        content = content.push(text(error));
    }
    if app.is_busy() {
        content = content.push(text("Выполняется операция…"));
    }
    container(
        content,
    )
    .padding(24)
    .center_x(Fill)
    .center_y(Fill)
    .into()
}
