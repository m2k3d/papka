//! Подключение DND к живому окну Wayland; файловые действия остаются в движке.
use guth_wayland_dnd::FileDnd;
use std::{
    hash::{Hash, Hasher},
    sync::Arc,
};

#[derive(Clone)]
pub struct NativeDnd {
    pub handle: FileDnd,
    pub wake: Wake,
}
impl std::fmt::Debug for NativeDnd {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("NativeDnd")
    }
}
#[derive(Clone)]
pub struct Wake(Arc<async_channel::Receiver<()>>);
impl Hash for Wake {
    fn hash<H: Hasher>(&self, state: &mut H) {
        Arc::as_ptr(&self.0).hash(state);
    }
}
impl NativeDnd {
    // Вызывается в window::run; до закрытия окна host обязан освободить адаптер.
    pub fn attach(
        window: &(
             impl iced::window::raw_window_handle::HasWindowHandle
             + iced::window::raw_window_handle::HasDisplayHandle
         ),
    ) -> Option<Self> {
        let (sender, receiver) = async_channel::bounded(1);
        let handle = FileDnd::attach(window, move || {
            let _ = sender.try_send(());
        })?;
        handle.set_incoming_move_allowed(false);
        Some(Self {
            handle,
            wake: Wake(Arc::new(receiver)),
        })
    }
}
impl Wake {
    pub async fn notified(&self) -> bool {
        self.0.recv().await.is_ok()
    }
}
