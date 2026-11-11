//! Интерфейс Iced: запуск и границы компонентов.
mod context_menu;
mod grid;
mod icons;
mod input;
mod message;
mod overlays;
mod palette;
mod palette_field;
mod panels;
mod sidebar;
mod style;
mod subscriptions;
mod update;
mod view;
mod widgets;

use crate::app::App;
use message::Message;
use style::theme;
use subscriptions::subscription;
use update::update;
use view::view;

// Существующий внешний harness подключает свои модули к копии этого файла.
#[cfg(test)]
use crate::{
    app::Dialog,
    files::Command,
    operations::{Background, Event},
};
#[cfg(test)]
use iced::keyboard::{self, key::Named};
#[cfg(test)]
use sidebar::short_path;
#[cfg(test)]
use std::path::PathBuf;

pub fn run() -> iced::Result {
    iced::application(App::default, update, view)
        .title("papka")
        .window_size((1120., 760.))
        .font(
            include_bytes!("../assets/fonts/caskaydia-cove/CaskaydiaCoveNerdFont-Regular.ttf")
                .as_slice(),
        )
        .default_font(iced::Font::with_name("CaskaydiaCove Nerd Font"))
        .exit_on_close_request(false)
        .theme(|app: &App| theme(app.settings.dark))
        .subscription(subscription)
        .run()
}
