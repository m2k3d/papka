// Модуль добавляется только во временную копию ui.rs; реализация не меняется.
#[cfg(test)]
mod dnd_tests {
    use super::*;
    use guth_wayland_dnd::{DndEvent, DropAction};
    use iced::advanced::{
        Layout, Shell, layout,
        widget::{Operation, Tree},
    };
    use iced::{Element, Event as UiEvent, Point, Rectangle, Size};
    use std::path::{Path, PathBuf};

    fn renderer() -> iced::Renderer {
        iced::Renderer::Secondary(iced_tiny_skia::Renderer::new(
            iced::Font::default(),
            16.into(),
        ))
    }
    fn messages(events: Vec<(UiEvent, Point)>) -> Vec<Message> {
        let mut element = super::drop_zone::zone(
            iced::widget::space().width(100).height(100).into(),
            Some(PathBuf::from("/test-only/source")),
            None,
        );
        let renderer = renderer();
        let mut tree = Tree::new(element.as_widget());
        let node = element.as_widget_mut().layout(
            &mut tree,
            &renderer,
            &layout::Limits::new(Size::ZERO, Size::new(100., 100.)),
        );
        let mut messages = vec![];
        let mut clipboard = iced::advanced::clipboard::Null;
        for (event, point) in events {
            element.as_widget_mut().update(
                &mut tree,
                &event,
                Layout::new(&node),
                iced::mouse::Cursor::Available(point),
                &renderer,
                &mut clipboard,
                &mut Shell::new(&mut messages),
                &Rectangle::with_size(Size::new(100., 100.)),
            );
        }
        messages
    }
    fn pressed() -> UiEvent {
        UiEvent::Mouse(iced::mouse::Event::ButtonPressed(iced::mouse::Button::Left))
    }
    fn released() -> UiEvent {
        UiEvent::Mouse(iced::mouse::Event::ButtonReleased(
            iced::mouse::Button::Left,
        ))
    }
    fn moved(point: Point) -> UiEvent {
        UiEvent::Mouse(iced::mouse::Event::CursorMoved { position: point })
    }

    #[test]
    fn dnd_click_and_threshold_publish_distinct_actions() {
        let p = Point::new(10., 10.);
        let q = Point::new(15.9, 10.);
        let click = messages(vec![(pressed(), p), (moved(q), q), (released(), q)]);
        assert!(
            matches!(&click[..], [Message::Select(path)] if path == Path::new("/test-only/source"))
        );
        let q = Point::new(16., 10.);
        let drag = messages(vec![
            (pressed(), p),
            (moved(q), q),
            (moved(Point::new(30., 10.)), Point::new(30., 10.)),
            (released(), q),
        ]);
        assert!(
            matches!(&drag[..], [Message::DragStart(path)] if path == Path::new("/test-only/source"))
        );
    }
    #[test]
    fn dnd_unfocus_and_outside_release_do_not_click_or_restart() {
        let p = Point::new(10., 10.);
        let q = Point::new(30., 10.);
        assert!(
            messages(vec![
                (pressed(), p),
                (UiEvent::Window(iced::window::Event::Unfocused), p),
                (moved(q), q),
                (released(), q)
            ])
            .is_empty()
        );
        let outside = Point::new(150., 10.);
        assert!(messages(vec![(pressed(), p), (released(), outside), (moved(q), q)]).is_empty());
    }
    #[test]
    fn dnd_left_surface_cancels_unstarted_widget_gesture() {
        let p = Point::new(10., 10.);
        let q = Point::new(30., 10.);
        let result = messages(vec![
            (pressed(), p),
            (UiEvent::Mouse(iced::mouse::Event::CursorLeft), p),
            (moved(q), q),
        ]);
        assert!(
            result.is_empty(),
            "После ухода указателя до начала drag возвращение без нового press не должно начинать drag: {result:?}"
        );
    }
    #[test]
    fn dnd_escape_cancels_widget_gesture() {
        let p = Point::new(10., 10.);
        let q = Point::new(30., 10.);
        let event = UiEvent::Keyboard(iced::keyboard::Event::KeyPressed {
            key: iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape),
            modified_key: iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape),
            physical_key: iced::keyboard::key::Physical::Code(iced::keyboard::key::Code::Escape),
            location: iced::keyboard::Location::Standard,
            modifiers: iced::keyboard::Modifiers::default(),
            text: None,
            repeat: false,
        });
        assert!(
            messages(vec![
                (pressed(), p),
                (event, p),
                (moved(q), q),
                (released(), q)
            ])
            .is_empty()
        );
    }
    fn zones(
        element: &mut Element<'_, Message>,
        tree: &mut Tree,
        node: &layout::Node,
        renderer: &iced::Renderer,
    ) -> Vec<(PathBuf, Rectangle)> {
        let mut collect = super::drop_zone::Collect::default();
        {
            let mut operation = iced::advanced::widget::operation::black_box(&mut collect);
            element
                .as_widget_mut()
                .operate(tree, Layout::new(node), renderer, &mut operation);
        }
        match collect.finish() {
            iced::advanced::widget::operation::Outcome::Some(zones) => zones,
            _ => panic!("Нет результата геометрии"),
        }
    }
    #[test]
    fn dnd_scroll_geometry_translates_and_clips_targets() {
        use iced::widget::{column, scrollable, space};
        let child = |name: &str| {
            super::drop_zone::zone(
                space().width(100).height(80).into(),
                None,
                Some(PathBuf::from(name)),
            )
        };
        let mut element: Element<'_, Message> =
            scrollable(column![child("/a"), child("/b"), child("/c")])
                .id("dnd-scroll")
                .height(100)
                .width(100)
                .into();
        let renderer = renderer();
        let mut tree = Tree::new(element.as_widget());
        let node = element.as_widget_mut().layout(
            &mut tree,
            &renderer,
            &layout::Limits::new(Size::ZERO, Size::new(100., 100.)),
        );
        let before = zones(&mut element, &mut tree, &node, &renderer);
        assert_eq!(before.len(), 2);
        let mut scroll = iced::advanced::widget::operation::scrollable::scroll_to::<()>(
            iced::advanced::widget::Id::new("dnd-scroll"),
            iced::advanced::widget::operation::scrollable::AbsoluteOffset {
                x: None,
                y: Some(80.),
            },
        );
        element
            .as_widget_mut()
            .operate(&mut tree, Layout::new(&node), &renderer, &mut scroll);
        let after = zones(&mut element, &mut tree, &node, &renderer);
        assert!(
            after.iter().any(|(path, bounds)| path == Path::new("/b")
                && bounds.y.abs() < 0.1
                && (bounds.height - 80.).abs() < 0.1),
            "{after:?}"
        );
        assert!(
            after
                .iter()
                .all(|(_, bounds)| bounds.y >= 0. && bounds.y + bounds.height <= 100.)
        );
    }
    #[test]
    fn dnd_open_panels_reject_targets() {
        let (background, _) = Background::start().unwrap();
        let mut app = App::default();
        app.background = Some(background);
        let zones = vec![(
            PathBuf::from("/test-only/target"),
            Rectangle::with_size(Size::new(100., 100.)),
        )];
        for panel in 0..5 {
            app.show_settings = panel == 0;
            app.show_history = panel == 1;
            app.show_palette = panel == 2;
            app.show_path = panel == 3;
            app.dialog = (panel == 4).then_some(Dialog::Transfer(false));
            super::dnd::events(
                &mut app,
                zones.clone(),
                vec![DndEvent::Hovered { x: 10., y: 10. }],
            );
            assert!(app.dnd.hover.is_none());
        }
        app.dialog = None;
        super::dnd::events(&mut app, zones, vec![DndEvent::Hovered { x: 10., y: 10. }]);
        assert_eq!(app.dnd.hover, Some(PathBuf::from("/test-only/target")));
        super::dnd::events(&mut app, vec![], vec![DndEvent::Left]);
        assert!(app.dnd.hover.is_none());
    }
    fn isolated(scenario: &str) {
        let root = tempfile::tempdir().unwrap();
        for name in ["data", "state", "config", "work"] {
            std::fs::create_dir(root.path().join(name)).unwrap();
        }
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "ui::dnd_tests::isolated_ui_probe",
                "--ignored",
                "--nocapture",
            ])
            .env("PAPKA_DND_UI_SCENARIO", scenario)
            .env("PAPKA_DND_UI_ROOT", root.path())
            .env("XDG_DATA_HOME", root.path().join("data"))
            .env("XDG_CONFIG_HOME", root.path().join("config"))
            .env("XDG_STATE_HOME", root.path().join("state"))
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        print!("{}", String::from_utf8_lossy(&output.stdout));
    }
    #[test]
    fn dnd_own_drop_before_end_moves_and_undoes() {
        isolated("own_drop_first");
    }
    #[test]
    fn dnd_own_end_before_drop_moves_and_undoes() {
        isolated("own_end_first");
    }
    #[test]
    fn dnd_own_ctrl_drop_copies() {
        isolated("own_ctrl");
    }
    #[test]
    fn dnd_external_copy_and_move_remain_distinct() {
        isolated("external");
    }
    #[test]
    fn dnd_stale_preparation_after_release_does_not_restart() {
        isolated("stale");
    }
    #[test]
    fn dnd_worker_failure_clears_native_gesture_state() {
        isolated("worker");
    }
    #[test]
    #[ignore]
    fn isolated_ui_probe() {
        use crate::files::{Data, DragSnapshot};
        let root = PathBuf::from(std::env::var_os("PAPKA_DND_UI_ROOT").unwrap());
        let work = root.join("work");
        let source = work.join("source");
        let dest = work.join("destination");
        std::fs::write(&source, b"original").unwrap();
        std::fs::create_dir(&dest).unwrap();
        assert_eq!(dirs::data_dir().unwrap(), root.join("data"));
        assert_eq!(
            crate::platform::trash_directory(&source).unwrap(),
            root.join("data/Trash/files")
        );
        let (background, receiver) = Background::start().unwrap();
        let mut app = App::default();
        app.directory = work.clone();
        app.background = Some(background.clone());
        let paths = vec![source.clone()];
        let ticket = background
            .submit(move |ctx| {
                ctx.data(Data::DragPrepared(
                    1,
                    DragSnapshot::capture(&paths, ctx).map_err(|e| e.to_string()),
                ));
                crate::app::completion(Ok(()))
            })
            .unwrap();
        let mut snapshot = None;
        loop {
            match receiver.recv_blocking().unwrap() {
                Event::Data {
                    id,
                    data: Data::DragPrepared(_, result),
                } if id == ticket.id => snapshot = Some(result.unwrap()),
                Event::Finished { id, .. } if id == ticket.id => break,
                _ => {}
            }
        }
        let scenario = std::env::var("PAPKA_DND_UI_SCENARIO").unwrap();
        let zones = vec![(dest.clone(), Rectangle::with_size(Size::new(100., 100.)))];
        if scenario.starts_with("own_") {
            app.dnd.snapshot = snapshot;
            app.dnd.active = true;
            app.selection.insert(source.clone());
            app.selection_control = scenario == "own_ctrl";
            let hover = DndEvent::Hovered { x: 10., y: 10. };
            let drop = DndEvent::Dropped {
                x: 10.,
                y: 10.,
                paths: vec![source.clone()],
                action: DropAction::Copy,
                completion_id: None,
            };
            let ended = DndEvent::DragEnded {
                moved_paths: vec![],
            };
            super::dnd::events(
                &mut app,
                zones,
                if scenario == "own_end_first" {
                    vec![hover, ended, drop]
                } else {
                    vec![hover, drop, ended]
                },
            );
            let id = app.mutation.as_ref().unwrap().id;
            finish(&mut app, &receiver, id);
            assert_eq!(std::fs::read(dest.join("source")).unwrap(), b"original");
            assert_eq!(source.exists(), scenario == "own_ctrl");
            let record = app.history.last().unwrap().id;
            let _ = update(&mut app, Message::Undo(record));
            let id = app.mutation.as_ref().unwrap().id;
            finish(&mut app, &receiver, id);
            assert_eq!(std::fs::read(&source).unwrap(), b"original");
            assert!(!dest.join("source").exists());
        } else if scenario == "external" {
            for action in [DropAction::Copy, DropAction::Move] {
                let path = if action == DropAction::Copy {
                    source.clone()
                } else {
                    let p = work.join("move-source");
                    std::fs::write(&p, b"original").unwrap();
                    p
                };
                super::dnd::events(
                    &mut app,
                    zones.clone(),
                    vec![DndEvent::Dropped {
                        x: 10.,
                        y: 10.,
                        paths: vec![path.clone()],
                        action,
                        completion_id: (action == DropAction::Move).then_some(50),
                    }],
                );
                let id = app.mutation.as_ref().unwrap().id;
                assert_eq!(
                    app.dnd.incoming,
                    if action == DropAction::Move {
                        Some((id, 50))
                    } else {
                        None
                    }
                );
                finish(&mut app, &receiver, id);
                assert!(path.exists());
                assert!(app.dnd.incoming.is_none());
                assert!(app.history.last().unwrap().label.contains(
                    if action == DropAction::Move {
                        "Move"
                    } else {
                        "Copy"
                    }
                ));
            }
        } else if scenario == "stale" {
            app.dnd.generation = 1;
            app.dnd.held = true;
            let _ = update(&mut app, Message::Keyboard(released()));
            assert!(!app.dnd.held);
            assert_ne!(app.dnd.generation, 1);
            let _ = update(
                &mut app,
                Message::Event(Event::Data {
                    id: ticket.id,
                    data: Data::DragPrepared(1, Ok(snapshot.unwrap())),
                }),
            );
            assert!(!app.dnd.active);
            assert!(app.dnd.snapshot.is_none());
        } else if scenario == "worker" {
            app.dnd.active = true;
            app.dnd.held = true;
            app.dnd.snapshot = snapshot;
            app.dnd.incoming = Some((20, 30));
            super::dnd::events(
                &mut app,
                zones,
                vec![DndEvent::WorkerUnavailable {
                    reason: "test failure".into(),
                }],
            );
            assert!(!app.dnd.active);
            assert!(!app.dnd.held);
            assert!(app.dnd.snapshot.is_none());
            assert!(app.dnd.incoming.is_none());
            assert!(app.error.as_ref().unwrap().contains("test failure"));
        } else {
            panic!("Неизвестный сценарий");
        }
    }
    fn finish(app: &mut App, receiver: &async_channel::Receiver<Event>, id: u64) {
        loop {
            let event = receiver.recv_blocking().unwrap();
            let done = matches!(&event,Event::Finished{id:operation,..} if *operation==id);
            if let Event::Finished {
                id: operation,
                completion,
            } = &event
            {
                if *operation == id {
                    assert!(
                        matches!(completion.outcome, crate::operations::Outcome::Success),
                        "{completion:?}"
                    );
                }
            }
            let _ = update(app, Message::Event(event));
            if done {
                break;
            }
        }
    }
}
