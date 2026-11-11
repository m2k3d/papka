// Этот модуль добавляется к временной копии ui.rs тестовым runner, src проекта не меняется.
#[cfg(test)]
mod navigation_tests {
    use super::*;
    use iced::futures::{StreamExt, executor::block_on};
    use iced::keyboard::{Key, Location, Modifiers, key};
    use std::path::Path;

    fn press(key: Key, modifiers: Modifiers) -> Message {
        Message::Keyboard(iced::Event::Keyboard(keyboard::Event::KeyPressed {
            modified_key: key.clone(),
            key,
            physical_key: key::Physical::Code(key::Code::F5),
            location: Location::Standard,
            modifiers,
            text: None,
            repeat: false,
        }))
    }
    fn apply(app: &mut App, message: Message) {
        let _ = update(app, message);
    }
    fn fresh() -> App {
        let mut app = App::default();
        app.directory = PathBuf::from("/papka-test-only/каталог/вложенная-папка");
        app.path_input = app.directory.to_string_lossy().into_owned();
        app
    }
    fn mouse_messages(
        app: &App,
        position: iced::Point,
        button: iced::mouse::Button,
        size: iced::Size,
    ) -> Vec<Message> {
        use iced::advanced::{Layout, Shell, layout, widget::Tree};
        let renderer = iced::Renderer::Secondary(iced_tiny_skia::Renderer::new(
            iced::Font::default(),
            16.into(),
        ));
        let mut element = view(app);
        let mut tree = Tree::new(element.as_widget());
        let node = element.as_widget_mut().layout(
            &mut tree,
            &renderer,
            &layout::Limits::new(iced::Size::ZERO, size),
        );
        let mut messages = Vec::new();
        let mut clipboard = iced::advanced::clipboard::Null;
        element.as_widget_mut().update(
            &mut tree,
            &iced::Event::Mouse(iced::mouse::Event::ButtonPressed(button)),
            Layout::new(&node),
            iced::mouse::Cursor::Available(position),
            &renderer,
            &mut clipboard,
            &mut Shell::new(&mut messages),
            &iced::Rectangle::with_size(size),
        );
        // Button публикует сообщение при отпускании, MouseArea — при нажатии.
        element.as_widget_mut().update(
            &mut tree,
            &iced::Event::Mouse(iced::mouse::Event::ButtonReleased(button)),
            Layout::new(&node),
            iced::mouse::Cursor::Available(position),
            &renderer,
            &mut clipboard,
            &mut Shell::new(&mut messages),
            &iced::Rectangle::with_size(size),
        );
        messages
    }

    #[test]
    fn context_selection_preserves_group_and_replaces_other_target() {
        let mut app = fresh();
        let a = app.directory.join("a");
        let b = app.directory.join("b");
        let c = app.directory.join("c");
        app.selection.extend([a.clone(), b.clone()]);
        app.selection_control = true;
        app.selection_shift = true;
        app.select_context(a.clone());
        assert_eq!(app.selected(), vec![a, b]);
        app.select_context(c.clone());
        assert_eq!(app.selected(), vec![c]);
    }
    #[test]
    fn context_click_breaks_double_click_sequence() {
        let mut app = fresh();
        let a = app.directory.join("a");
        let b = app.directory.join("b");
        app.select(a.clone());
        app.selection.insert(b);
        app.select_context(a.clone());
        app.select(a.clone());
        assert_eq!(
            app.selected(),
            vec![a],
            "Следующий левый клик должен выделить, а не открыть элемент"
        );
    }
    #[test]
    fn context_target_pointer_and_empty_space_are_distinct() {
        let mut app = fresh();
        let a = app.directory.join("a");
        app.selection.insert(a.clone());
        app.pointer = iced::Point::new(900., 700.);
        apply(&mut app, Message::Context(Some(a.clone())));
        assert_eq!(app.context_menu, Some((Some(a.clone()), app.pointer)));
        apply(&mut app, Message::Context(None));
        assert_eq!(app.context_menu, Some((None, app.pointer)));
        assert_eq!(app.selected(), vec![a]);
    }
    #[test]
    fn menu_copy_preserves_multiple_paths_and_closes_menu() {
        let mut app = fresh();
        app.selection
            .extend([app.directory.join("a"), app.directory.join("b")]);
        let target = app.directory.join("a");
        apply(&mut app, Message::Context(Some(target)));
        apply(
            &mut app,
            Message::MenuAction(Box::new(Message::CopySelection(true))),
        );
        assert!(app.context_menu.is_none());
        assert_eq!(app.clipboard, Some((app.selected(), true)));
    }
    #[test]
    fn escape_and_outside_close_context_without_selection_changes() {
        let mut app = fresh();
        let target = app.directory.join("a");
        apply(&mut app, Message::Context(Some(target.clone())));
        apply(
            &mut app,
            press(Key::Named(Named::Escape), Modifiers::empty()),
        );
        assert!(app.context_menu.is_none());
        assert_eq!(app.selected(), vec![target.clone()]);
        apply(&mut app, Message::Context(Some(target)));
        apply(&mut app, Message::CloseContext);
        assert!(app.context_menu.is_none());
        assert!(app.clipboard.is_none());
    }
    #[test]
    fn control_l_opens_complete_path_and_escape_cancels_input() {
        let mut app = fresh();
        let directory = app.directory.clone();
        apply(&mut app, press(Key::Character("l".into()), Modifiers::CTRL));
        assert!(app.show_path);
        assert_eq!(app.path_input, directory.to_string_lossy());
        apply(&mut app, Message::Path("/papka-test-only/другой".into()));
        assert!(app.path_dirty);
        apply(
            &mut app,
            press(Key::Named(Named::Escape), Modifiers::empty()),
        );
        assert!(!app.show_path && !app.path_dirty);
        assert_eq!(app.directory, directory);
        apply(&mut app, Message::EditPath);
        assert_eq!(app.path_input, directory.to_string_lossy());
    }
    #[test]
    fn copy_path_task_contains_full_current_directory() {
        let mut app = fresh();
        app.path_input = "/несохранённый-ввод".into();
        let task = update(&mut app, Message::CopyPath);
        let mut stream = iced_runtime::task::into_stream(task).unwrap();
        match block_on(stream.next()).unwrap() {
            iced_runtime::Action::Clipboard(iced_runtime::clipboard::Action::Write {
                contents,
                ..
            }) => assert_eq!(contents, app.directory.to_string_lossy()),
            other => panic!("Ожидалась запись пути в буфер: {other:?}"),
        }
    }
    #[test]
    fn shortened_paths_keep_ends_and_handle_unicode_and_home() {
        assert_eq!(short_path(Path::new("/"), 20), "/");
        assert_eq!(short_path(Path::new("/a/b"), 20), "/a/b");
        if let Some(home) = dirs::home_dir() {
            assert_eq!(short_path(&home, 20), "~");
            assert_eq!(short_path(&home.join("Документы"), 30), "~/Документы");
            assert!(
                !short_path(&PathBuf::from(format!("{}-other", home.display())), 300)
                    .starts_with('~')
            );
        }
        let path = Path::new("/искусственный/очень-длинный/путь/最後のフォルダ");
        for budget in 0..50 {
            let result = short_path(path, budget);
            assert!(result.chars().count() <= budget.max(3));
            if result.contains('…') {
                assert!(result.starts_with('/'));
                assert!(result.ends_with('ダ'));
            }
        }
    }
    fn finish_read(app: &mut App, receiver: &async_channel::Receiver<Event>) {
        let id = app
            .loading
            .expect("Горячая клавиша должна запустить чтение");
        loop {
            let event = receiver.recv_blocking().unwrap();
            let done = matches!(event, Event::Finished { id: finished, .. } if finished == id);
            app.apply(event);
            if done {
                break;
            }
        }
        assert!(app.loading.is_none());
    }
    #[test]
    fn f5_and_control_r_refresh_only_test_directory() {
        let directory = tempfile::tempdir().unwrap();
        let mut app = fresh();
        app.directory = directory.path().to_path_buf();
        let (background, receiver) = Background::start().unwrap();
        app.background = Some(background.clone());
        apply(&mut app, press(Key::Named(Named::F5), Modifiers::empty()));
        finish_read(&mut app, &receiver);
        std::fs::write(directory.path().join("создан-для-проверки"), b"test").unwrap();
        apply(&mut app, press(Key::Character("r".into()), Modifiers::CTRL));
        finish_read(&mut app, &receiver);
        assert_eq!(app.entries.len(), 1);
        background.shutdown();
    }
    #[test]
    fn go_navigates_to_full_edited_path_in_test_directory() {
        let directory = tempfile::tempdir().unwrap();
        let child = directory.path().join("подкаталог");
        std::fs::create_dir(&child).unwrap();
        let mut app = fresh();
        app.directory = directory.path().to_path_buf();
        let (background, receiver) = Background::start().unwrap();
        app.background = Some(background.clone());
        apply(&mut app, Message::EditPath);
        apply(
            &mut app,
            Message::Path(child.to_string_lossy().into_owned()),
        );
        apply(&mut app, Message::Go);
        assert!(!app.show_path);
        finish_read(&mut app, &receiver);
        assert_eq!(app.directory, child);
        background.shutdown();
    }
    #[test]
    fn menu_blocks_global_file_shortcuts() {
        let mut app = fresh();
        let target = app.directory.join("a");
        apply(&mut app, Message::Context(Some(target)));
        apply(&mut app, press(Key::Character("x".into()), Modifiers::CTRL));
        assert!(app.clipboard.is_none());
        assert!(app.context_menu.is_some());
    }
    #[test]
    fn actual_blank_menu_offers_create_and_outside_click_only_dismisses() {
        let mut app = fresh();
        app.pointer = iced::Point::new(400., 200.);
        apply(&mut app, Message::Context(None));
        let messages = mouse_messages(
            &app,
            iced::Point::new(430., 225.),
            iced::mouse::Button::Left,
            iced::Size::new(1120., 760.),
        );
        assert!(messages.iter().any(|m| matches!(m, Message::MenuAction(action) if matches!(**action, Message::Create(true)))), "{messages:?}");
        let messages = mouse_messages(
            &app,
            iced::Point::new(900., 600.),
            iced::mouse::Button::Left,
            iced::Size::new(1120., 760.),
        );
        assert!(
            messages.iter().any(|m| matches!(m, Message::CloseContext)),
            "{messages:?}"
        );
        assert!(!messages.iter().any(|m| matches!(m, Message::MenuAction(_))));
    }
    #[test]
    fn actual_file_menu_opens_clicked_target_and_fits_bottom_right() {
        let mut app = fresh();
        let target = app.directory.join("a");
        app.pointer = iced::Point::new(1119., 759.);
        apply(&mut app, Message::Context(Some(target.clone())));
        let messages = mouse_messages(
            &app,
            iced::Point::new(900., 445.),
            iced::mouse::Button::Left,
            iced::Size::new(1120., 760.),
        );
        assert!(messages.iter().any(|m| matches!(m, Message::MenuAction(action) if matches!(&**action, Message::Open(path) if path == &target))), "{messages:?}");
    }
    #[test]
    fn actual_card_right_click_emits_only_file_context() {
        let directory = tempfile::tempdir().unwrap();
        let file = directory.path().join("файл-для-проверки");
        std::fs::write(&file, b"test").unwrap();
        let mut app = fresh();
        app.directory = directory.path().to_path_buf();
        let (background, receiver) = Background::start().unwrap();
        app.background = Some(background.clone());
        apply(&mut app, Message::Refresh);
        finish_read(&mut app, &receiver);
        let messages = mouse_messages(
            &app,
            iced::Point::new(300., 120.),
            iced::mouse::Button::Right,
            iced::Size::new(1120., 760.),
        );
        assert!(
            messages
                .iter()
                .any(|m| matches!(m, Message::Context(Some(path)) if path == &file)),
            "{messages:?}"
        );
        assert!(
            !messages.iter().any(|m| matches!(m, Message::Context(None))),
            "Свободная область не должна перехватить клик карточки: {messages:?}"
        );
        background.shutdown();
    }
    #[test]
    fn context_menu_remains_clickable_in_small_window() {
        let mut app = fresh();
        app.pointer = iced::Point::new(319., 199.);
        apply(&mut app, Message::Context(None));
        let messages = mouse_messages(
            &app,
            iced::Point::new(100., 30.),
            iced::mouse::Button::Left,
            iced::Size::new(320., 200.),
        );
        assert!(messages.iter().any(|m| matches!(m, Message::MenuAction(action) if matches!(**action, Message::Create(true)))), "{messages:?}");
    }
}
