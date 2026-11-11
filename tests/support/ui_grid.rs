// Модуль подключается только к временной копии ui.rs.
#[cfg(test)]
mod grid_tests {
    use super::*;
    use iced::advanced::{
        Layout, Shell, layout, renderer,
        widget::{Id, Operation, Tree, operation::Scrollable},
    };
    use iced::{Point, Rectangle, Size};
    use std::time::{Duration, Instant};

    #[derive(Default)]
    struct Geometry {
        names: Vec<(String, Rectangle)>,
        scrolls: Vec<(Rectangle, Rectangle)>,
        labels: Vec<String>,
        texts: Vec<(String, Rectangle)>,
        icons: Vec<(Rectangle, Option<iced::Color>)>,
        surfaces: Vec<(Rectangle, f32)>,
    }
    impl Operation for Geometry {
        fn traverse(&mut self, f: &mut dyn FnMut(&mut dyn Operation)) {
            f(self);
        }
        fn text(&mut self, _: Option<&Id>, bounds: Rectangle, value: &str) {
            self.labels.push(value.into());
            self.texts.push((value.into(), bounds));
            if value.starts_with("grid-") {
                self.names.push((value.into(), bounds));
            }
        }
        fn scrollable(
            &mut self,
            _: Option<&Id>,
            bounds: Rectangle,
            content: Rectangle,
            _: iced::Vector,
            _: &mut dyn Scrollable,
        ) {
            if bounds.x > 200. {
                self.scrolls.push((bounds, content));
            }
        }
    }
    fn apply(app: &mut App, message: Message) {
        let _ = update(app, message);
    }
    fn sample(count: usize) -> (tempfile::TempDir, App) {
        let dir = tempfile::tempdir().unwrap();
        for n in 0..count {
            std::fs::write(dir.path().join(format!("grid-{n:03}")), b"test").unwrap();
        }
        let mut app = App::default();
        app.directory = dir.path().into();
        let (background, receiver) = Background::start().unwrap();
        app.background = Some(background.clone());
        app.refresh();
        let expected = app.loading.unwrap();
        loop {
            let event = receiver.recv_blocking().unwrap();
            let done = matches!(event,Event::Finished{id,..} if id==expected);
            app.apply(event);
            if done {
                break;
            }
        }
        background.shutdown();
        app.background = None;
        app.transition = None;
        (dir, app)
    }
    fn inspect(
        app: &App,
        size: Size,
        events: &[(iced::Event, Point)],
    ) -> (Geometry, Vec<(Rectangle, f32)>, Vec<Message>) {
        let mut renderer = iced::Renderer::Secondary(iced_tiny_skia::Renderer::new(
            iced::Font::default(),
            16.into(),
        ));
        iced::advanced::Renderer::reset(&mut renderer, Rectangle::with_size(size));
        let mut element = view(app);
        let mut tree = Tree::new(element.as_widget());
        let node = element.as_widget_mut().layout(
            &mut tree,
            &renderer,
            &layout::Limits::new(Size::ZERO, size),
        );
        let mut geometry = Geometry::default();
        element
            .as_widget_mut()
            .operate(&mut tree, Layout::new(&node), &renderer, &mut geometry);
        let mut messages = Vec::new();
        let mut clipboard = iced::advanced::clipboard::Null;
        for (event, position) in events {
            element.as_widget_mut().update(
                &mut tree,
                event,
                Layout::new(&node),
                iced::mouse::Cursor::Available(*position),
                &renderer,
                &mut clipboard,
                &mut Shell::new(&mut messages),
                &Rectangle::with_size(size),
            );
        }
        // Iced фиксирует Status::Dragged только на RedrawRequested, как перед реальным кадром.
        let cursor = events
            .last()
            .map_or(iced::mouse::Cursor::Unavailable, |(_, p)| {
                iced::mouse::Cursor::Available(*p)
            });
        element.as_widget_mut().update(
            &mut tree,
            &iced::Event::Window(iced::window::Event::RedrawRequested(Instant::now())),
            Layout::new(&node),
            cursor,
            &renderer,
            &mut clipboard,
            &mut Shell::new(&mut messages),
            &Rectangle::with_size(size),
        );
        element.as_widget().draw(
            &tree,
            &mut renderer,
            &theme(app.settings.dark),
            &renderer::Style {
                text_color: theme(app.settings.dark).palette().text,
            },
            Layout::new(&node),
            iced::mouse::Cursor::Unavailable,
            &Rectangle::with_size(size),
        );
        let mut thumbs = Vec::new();
        if let iced::Renderer::Secondary(r) = &mut renderer {
            for layer in r.layers() {
                for image in &layer.images {
                    if let iced_graphics::Image::Vector { svg, bounds, .. } = image {
                        geometry.icons.push((*bounds, svg.color));
                    }
                }
                for (quad, background) in &layer.quads {
                    if quad.bounds.width > 300. && quad.bounds.height > 300. {
                        geometry
                            .surfaces
                            .push((quad.bounds, quad.border.radius.top_left));
                    }
                    if quad.bounds.x > 200. && (quad.bounds.width - 5.).abs() < 0.1 {
                        if let iced::Background::Color(color) = background {
                            thumbs.push((quad.bounds, color.a));
                        }
                    }
                }
            }
        }
        (geometry, thumbs, messages)
    }
    fn approx(a: f32, b: f32) {
        assert!((a - b).abs() < 0.6, "{a} != {b}");
    }
    fn first_row(g: &Geometry) -> Vec<Rectangle> {
        let y = g.names[0].1.y;
        g.names
            .iter()
            .filter(|(_, r)| (r.y - y).abs() < 0.1)
            .map(|(_, r)| *r)
            .collect()
    }
    #[test]
    fn grid_fills_width_and_last_row_keeps_column_alignment() {
        let (_dir, app) = sample(7);
        let (g, _, _) = inspect(&app, Size::new(1120., 760.), &[]);
        let first = first_row(&g);
        assert!(first.len() > 1 && first.len() < 7);
        for pair in first.windows(2) {
            approx(pair[0].width, pair[1].width);
            approx(pair[1].x - pair[0].x, pair[0].width + 36.);
        }
        let last = g.names.last().unwrap().1;
        approx(last.x, first[(g.names.len() - 1) % first.len()].x);
        approx(last.width, first[0].width);
        let (scroll, content) = g.scrolls[0];
        approx(scroll.width, content.width);
        let left = first[0].x - 12.;
        let right = first.last().unwrap().x + first.last().unwrap().width + 12.;
        approx(left, scroll.x);
        approx(right, scroll.x + scroll.width);
        approx(scroll.x + scroll.width, 1120. - 32.);
    }
    #[test]
    fn resizing_changes_spacing_and_wraps_columns() {
        let (_dir, app) = sample(12);
        let (a, _, _) = inspect(&app, Size::new(1120., 760.), &[]);
        let (b, _, _) = inspect(&app, Size::new(1130., 760.), &[]);
        let (c, _, _) = inspect(&app, Size::new(950., 760.), &[]);
        let a = first_row(&a);
        let b = first_row(&b);
        let c = first_row(&c);
        assert_eq!(a.len(), b.len());
        assert!(b[0].width > a[0].width);
        assert!((b[1].x - b[0].x) > (a[1].x - a[0].x));
        assert!(c.len() < a.len());
    }
    #[test]
    fn single_column_and_empty_directory_layout_are_valid() {
        let (_dir, app) = sample(3);
        let (g, _, _) = inspect(&app, Size::new(430., 760.), &[]);
        assert_eq!(first_row(&g).len(), 1);
        for (_, r) in &g.names {
            approx(r.x, g.names[0].1.x);
        }
        let (_dir, app) = sample(0);
        let (g, _, _) = inspect(&app, Size::new(1120., 760.), &[]);
        assert!(g.labels.iter().any(|s| s == "Папка пуста"));
    }
    #[test]
    fn scrollbar_is_hidden_idle_visible_after_scroll_and_fades_without_moving_grid() {
        let (_dir, mut app) = sample(80);
        let size = Size::new(1120., 760.);
        let (idle, thumbs, _) = inspect(&app, size, &[]);
        assert!(thumbs.iter().all(|(_, a)| *a == 0.));
        apply(&mut app, Message::FilesScrolled);
        let (active, thumbs, _) = inspect(&app, size, &[]);
        assert!(thumbs.iter().any(|(_, a)| *a > 0.3));
        approx(idle.scrolls[0].0.width, active.scrolls[0].0.width);
        approx(idle.names[0].1.width, active.names[0].1.width);
        app.last_file_scroll = Some(Instant::now() - Duration::from_millis(900));
        let (_, thumbs, _) = inspect(&app, size, &[]);
        assert!(
            thumbs.iter().any(|(_, a)| *a > 0.05 && *a < 0.30),
            "{thumbs:?}"
        );
        app.last_file_scroll = Some(Instant::now() - Duration::from_millis(1100));
        let (_, thumbs, _) = inspect(&app, size, &[]);
        assert!(thumbs.iter().all(|(_, a)| *a == 0.));
    }
    #[test]
    fn reduced_motion_skips_fade_and_frame_timer_finishes() {
        let (_dir, mut app) = sample(80);
        app.settings.reduced_motion = true;
        let size = Size::new(1120., 760.);
        let baseline = iced_futures::subscription::into_recipes(subscription(&app)).len();
        app.last_file_scroll = Some(Instant::now() - Duration::from_millis(900));
        let (_, thumbs, _) = inspect(&app, size, &[]);
        assert!(thumbs.iter().any(|(_, a)| *a > 0.3));
        assert_eq!(
            iced_futures::subscription::into_recipes(subscription(&app)).len(),
            baseline + 1
        );
        // Время отрисовки не должно переводить проверку через границу таймера.
        app.last_file_scroll = Some(Instant::now());
        apply(&mut app, Message::Frame);
        assert!(app.last_file_scroll.is_some());
        app.last_file_scroll = Some(Instant::now() - Duration::from_millis(1100));
        apply(&mut app, Message::Frame);
        assert!(app.last_file_scroll.is_none());
        assert_eq!(
            iced_futures::subscription::into_recipes(subscription(&app)).len(),
            baseline
        );
        let (_, thumbs, _) = inspect(&app, size, &[]);
        assert!(thumbs.iter().all(|(_, a)| *a == 0.));
    }
    #[test]
    fn wheel_emits_activity_and_drag_keeps_thumb_visible_after_timeout() {
        let (_dir, mut app) = sample(80);
        let size = Size::new(1120., 760.);
        let (_, _, messages) = inspect(
            &app,
            size,
            &[(
                iced::Event::Mouse(iced::mouse::Event::WheelScrolled {
                    delta: iced::mouse::ScrollDelta::Lines { x: 0., y: -2. },
                }),
                Point::new(700., 300.),
            )],
        );
        assert!(
            messages.iter().any(|m| matches!(m, Message::FilesScrolled)),
            "{messages:?}"
        );
        apply(&mut app, Message::FilesScrolled);
        let (_, thumbs, _) = inspect(&app, size, &[]);
        let thumb = thumbs.iter().find(|(_, a)| *a > 0.3).unwrap().0;
        app.last_file_scroll = None;
        let start = Point::new(thumb.center_x(), thumb.center_y());
        let moved = Point::new(start.x, start.y + 40.);
        let (_, thumbs, messages) = inspect(
            &app,
            size,
            &[
                (
                    iced::Event::Mouse(iced::mouse::Event::ButtonPressed(
                        iced::mouse::Button::Left,
                    )),
                    start,
                ),
                (
                    iced::Event::Mouse(iced::mouse::Event::CursorMoved { position: moved }),
                    moved,
                ),
            ],
        );
        assert!(
            messages.iter().any(|m| matches!(m, Message::FilesScrolled)),
            "{messages:?}"
        );
        assert!(
            thumbs.iter().any(|(_, a)| *a > 0.3),
            "Перетаскиваемый thumb должен оставаться видимым без таймера: {thumbs:?}"
        );
    }
    #[test]
    fn settings_absent_from_sidebar_but_available_via_context_menu() {
        let (_dir, mut app) = sample(0);
        let size = Size::new(1120., 760.);
        let (g, _, _) = inspect(&app, size, &[]);
        assert!(!g.labels.iter().any(|s| s == "Настройки"));
        app.context_menu = Some((None, Point::new(400., 200.)));
        let (g, _, _) = inspect(&app, size, &[]);
        assert!(g.labels.iter().any(|s| s == "Настройки"));
        apply(&mut app, Message::MenuAction(Box::new(Message::Settings)));
        assert!(app.show_settings && app.context_menu.is_none());
    }
    #[test]
    fn polish_footer_and_grid_have_equal_panel_insets() {
        let (_dir, mut app) = sample(80);
        for size in [
            Size::new(1120., 760.),
            Size::new(1000., 600.),
            Size::new(650., 500.),
        ] {
            for dark in [false, true] {
                app.settings.dark = dark;
                let (g, _, _) = inspect(&app, size, &[]);
                let (surface, radius) = g.surfaces.iter().find(|(r, _)| r.x > 200.).unwrap();
                approx(*radius, 6.);
                let left = g
                    .texts
                    .iter()
                    .find(|(s, _)| s.starts_with("Элементов:"))
                    .unwrap()
                    .1;
                let right = g
                    .texts
                    .iter()
                    .find(|(s, _)| s.starts_with("Выделено:"))
                    .unwrap()
                    .1;
                approx(left.x - surface.x, 24.);
                approx(surface.x + surface.width - right.x - right.width, 24.);
                let first = first_row(&g);
                approx(first[0].x - 12., left.x);
                let last = first.last().unwrap();
                approx(last.x + last.width + 12., right.x + right.width);
            }
        }
    }
    #[test]
    fn polish_sidebar_icons_use_fixed_slots_and_theme_color() {
        let (_dir, mut app) = sample(0);
        for dark in [false, true] {
            app.settings.dark = dark;
            let (g, _, _) = inspect(&app, Size::new(1120., 760.), &[]);
            assert!(g.icons.len() >= 7, "SVG не попали в draw: {:?}", g.icons);
            for (index, (bounds, color)) in g.icons.iter().enumerate() {
                approx(bounds.width, 20.);
                approx(bounds.height, 20.);
                if index >= 3 {
                    assert_eq!(*color, Some(theme(dark).palette().text));
                }
            }
        }
    }
    #[test]
    fn polish_sidebar_tools_dispatch_and_disabled_back_is_inert() {
        let (_dir, app) = sample(0);
        let size = Size::new(1120., 760.);
        let click = |p| {
            vec![
                (
                    iced::Event::Mouse(iced::mouse::Event::ButtonPressed(
                        iced::mouse::Button::Left,
                    )),
                    p,
                ),
                (
                    iced::Event::Mouse(iced::mouse::Event::ButtonReleased(
                        iced::mouse::Button::Left,
                    )),
                    p,
                ),
            ]
        };
        let (g, _, _) = inspect(&app, size, &[]);
        let at = |r: Rectangle| Point::new(r.center_x(), r.center_y());
        let icons = &g.icons;
        let (_, _, messages) = inspect(&app, size, &click(at(icons[0].0)));
        assert!(!messages.iter().any(|m| matches!(m, Message::Back)));
        let (_, _, messages) = inspect(&app, size, &click(at(icons[icons.len() - 2].0)));
        assert!(
            messages.iter().any(|m| matches!(m, Message::History)),
            "{messages:?}"
        );
        let (_, _, messages) = inspect(&app, size, &click(at(icons[icons.len() - 1].0)));
        assert!(
            messages.iter().any(|m| matches!(m, Message::Palette)),
            "{messages:?}"
        );
        let path = g
            .texts
            .iter()
            .find(|(_, r)| r.x < 200. && r.y < 120.)
            .unwrap();
        let (_, _, messages) = inspect(&app, size, &click(at(path.1)));
        assert!(
            messages.iter().any(|m| matches!(m, Message::EditPath)),
            "{messages:?}"
        );
    }
    #[test]
    fn polish_completion_clears_success_but_preserves_failure_and_interruption() {
        use crate::operations::{Completion, Outcome};
        for outcome in [
            Outcome::Success,
            Outcome::Interrupted,
            Outcome::Failed("Ошибка проверки".into()),
        ] {
            let (_dir, mut app) = sample(3);
            let (background, receiver) = Background::start().unwrap();
            let result = Completion {
                outcome: outcome.clone(),
                completed: Some(2),
                remaining: Some(1),
                undo_available: true,
            };
            let ticket = background.submit(move |_| result).unwrap();
            app.mutation = Some(ticket);
            app.status = "Выполняется…".into();
            app.operation_notice = Some("Старое уведомление".into());
            loop {
                let event = receiver.recv_blocking().unwrap();
                let finished = matches!(event, Event::Finished { .. });
                app.apply(event);
                if finished {
                    break;
                }
            }
            background.shutdown();
            match outcome {
                Outcome::Success => {
                    assert_eq!(app.status, "Элементов: 3");
                    assert!(app.operation_notice.is_none());
                }
                Outcome::Interrupted => assert!(
                    app.operation_notice
                        .as_ref()
                        .unwrap()
                        .contains("Прерывание завершено")
                ),
                Outcome::Failed(_) => {
                    assert_eq!(app.error.as_deref(), Some("Ошибка проверки"));
                    assert!(
                        app.operation_notice
                            .as_ref()
                            .unwrap()
                            .contains("не завершена полностью")
                    );
                }
            }
        }
    }

    #[test]
    fn polish_disabled_navigation_has_visually_dimmed_icon() {
        let (_dir, app) = sample(0);
        assert!(!app.can_back());
        assert!(app.directory.parent().is_some());
        let (g, _, _) = inspect(&app, Size::new(1120., 760.), &[]);
        let disabled = g.icons[0].1.unwrap();
        let enabled = g.icons[2].1.unwrap();
        assert!(
            disabled.a < enabled.a,
            "Недоступная «Назад» должна быть приглушена относительно доступной «Вверх»: {disabled:?} / {enabled:?}"
        );
    }
}
