//! Подписки на системный ввод, фоновые события и таймеры.
use super::Message;
use crate::{app::App, operations::Background};
use iced::futures::{SinkExt, Stream};
use iced::{Subscription, keyboard};

pub(super) fn subscription(app: &App) -> Subscription<Message> {
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
                    "Could not start background tasks.".into(),
                ))
                .await;
        }
    })
}
