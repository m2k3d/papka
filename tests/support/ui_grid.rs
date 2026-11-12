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
        all_thumbs: Vec<(Rectangle, iced::Color, f32)>,
        svg_handles: Vec<iced::widget::svg::Handle>,
        has_overlay: bool,
        inputs: Vec<Rectangle>,
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
        fn text_input(
            &mut self,
            _: Option<&Id>,
            bounds: Rectangle,
            _: &mut dyn iced::advanced::widget::operation::TextInput,
        ) {
            self.inputs.push(bounds);
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
        iced_graphics::text::font_system()
            .write()
            .unwrap()
            .load_font(std::borrow::Cow::Borrowed(include_bytes!(
                "../assets/fonts/caskaydia-cove/CaskaydiaCoveNerdFont-Regular.ttf"
            )));
        let mut renderer = iced::Renderer::Secondary(iced_tiny_skia::Renderer::new(
            iced::Font::with_name("CaskaydiaCove Nerd Font"),
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
        geometry.has_overlay = element
            .as_widget_mut()
            .overlay(
                &mut tree,
                Layout::new(&node),
                &renderer,
                &Rectangle::with_size(size),
                iced::Vector::ZERO,
            )
            .is_some();
        let mut thumbs = Vec::new();
        if let iced::Renderer::Secondary(r) = &mut renderer {
            for layer in r.layers() {
                for image in &layer.images {
                    if let iced_graphics::Image::Vector { svg, bounds, .. } = image {
                        geometry.icons.push((*bounds, svg.color));
                        geometry.svg_handles.push(svg.handle.clone());
                    }
                }
                for (quad, background) in &layer.quads {
                    if quad.bounds.width > 300. && quad.bounds.height > 300. {
                        geometry
                            .surfaces
                            .push((quad.bounds, quad.border.radius.top_left));
                    }
                    if (quad.bounds.width - 5.).abs() < 0.1 {
                        if let iced::Background::Color(color) = background {
                            geometry.all_thumbs.push((
                                quad.bounds,
                                *color,
                                quad.border.radius.top_left,
                            ));
                        }
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
        let (_dir, app) = sample(10);
        let (g, _, _) = inspect(&app, Size::new(1120., 760.), &[]);
        let first = first_row(&g);
        assert!(first.len() > 1 && first.len() < 10);
        for pair in first.windows(2) {
            approx(pair[0].width, pair[1].width);
            approx(pair[1].x - pair[0].x, pair[0].width + 18.);
        }
        let last = g.names.last().unwrap().1;
        approx(last.x, first[(g.names.len() - 1) % first.len()].x);
        approx(last.width, first[0].width);
        let (scroll, content) = g.scrolls[0];
        approx(scroll.width, content.width);
        let left = first[0].x - 6.;
        let right = first.last().unwrap().x + first.last().unwrap().width + 6.;
        approx(left, scroll.x);
        approx(right, scroll.x + scroll.width);
        approx(scroll.x + scroll.width, 1120. - 26.);
    }
    #[test]
    fn compact_grid_fills_width_across_column_boundaries() {
        let (_dir, app) = sample(31);
        for width in (430..=1800).step_by(17).chain([
            507, 508, 509, 541, 542, 543, 637, 638, 639, 640, 641, 767, 768, 769, 897, 898, 899,
        ]) {
            let (g, _, _) = inspect(&app, Size::new(width as f32, 900.), &[]);
            let first = first_row(&g);
            let (scroll, _) = g.scrolls[0];
            let card_width = first[0].width + 12.;
            approx(first[0].x - 6., scroll.x);
            approx(
                first.last().unwrap().x + first.last().unwrap().width + 6.,
                scroll.x + scroll.width,
            );
            // После узкого перехода к трём колонкам карточки не шире 124.
            if scroll.width >= 288. {
                assert!(card_width <= 124.6, "window={width}, card={card_width}");
            }
            for pair in first.windows(2) {
                approx(pair[1].x - pair[0].x, card_width + 6.);
            }
            for (index, (_, rect)) in g.names.iter().enumerate() {
                approx(rect.x, first[index % first.len()].x);
                approx(rect.width, first[0].width);
                assert!(rect.x + rect.width <= scroll.x + scroll.width + 0.6);
            }
        }
    }

    #[test]
    fn material_ui_labels_are_english_and_font_is_loadable() {
        let (_dir, mut app) = sample(0);
        for panel in 0..4 {
            app.show_settings = panel == 1;
            app.show_history = panel == 2;
            app.show_palette = panel == 3;
            let (g, _, _) = inspect(&app, Size::new(1120., 760.), &[]);
            assert!(g.labels.iter().any(|s| s == "Quick access"));
            assert!(g.labels.iter().any(|s| s == "Devices"));
            for label in &g.labels {
                assert!(
                    !label
                        .chars()
                        .any(|c| ('\u{0400}'..='\u{04ff}').contains(&c)),
                    "{label}"
                );
            }
        }
        let mut fonts = iced_graphics::text::font_system().write().unwrap();
        assert!(fonts.raw().db().faces().any(|face| {
            face.families
                .iter()
                .any(|(family, _)| family == "CaskaydiaCove Nerd Font")
        }));
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
        assert!(g.labels.iter().any(|s| s == "This folder is empty"));
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
        assert!(!g.labels.iter().any(|s| s == "Settings"));
        app.context_menu = Some((None, Point::new(400., 200.)));
        let (g, _, _) = inspect(&app, size, &[]);
        assert!(g.labels.iter().any(|s| s == "Settings"));
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
                approx(*radius, 12.);
                let left = g
                    .texts
                    .iter()
                    .find(|(s, _)| s.starts_with("Items:"))
                    .unwrap()
                    .1;
                let right = g
                    .texts
                    .iter()
                    .find(|(s, _)| s.starts_with("Selected:"))
                    .unwrap()
                    .1;
                approx(left.x - surface.x, 18.);
                approx(surface.x + surface.width - right.x - right.width, 18.);
                let first = first_row(&g);
                approx(first[0].x - 6., left.x);
                let last = first.last().unwrap();
                approx(last.x + last.width + 6., right.x + right.width);
            }
        }
    }
    #[test]
    fn polish_sidebar_icons_use_fixed_slots_and_theme_color() {
        let (_dir, mut app) = sample(0);
        for dark in [false, true] {
            app.settings.dark = dark;
            let (g, _, _) = inspect(&app, Size::new(1120., 760.), &[]);
            assert!(g.icons.len() >= 6, "SVG не попали в draw: {:?}", g.icons);
            for (index, (bounds, color)) in g.icons.iter().enumerate() {
                approx(bounds.width, 24.);
                approx(bounds.height, 24.);
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
        let (_, _, messages) = inspect(&app, size, &click(at(icons[icons.len() - 1].0)));
        assert!(
            messages.iter().any(|m| matches!(m, Message::History)),
            "{messages:?}"
        );
        assert!(
            !g.svg_handles
                .contains(&iced::widget::svg::Handle::from_memory(include_bytes!(
                    "../assets/icons/material/palette.svg"
                )))
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
                    assert_eq!(app.status, "Items: 3");
                    assert!(app.operation_notice.is_none());
                }
                Outcome::Interrupted => assert!(
                    app.operation_notice
                        .as_ref()
                        .unwrap()
                        .contains("Operation stopped")
                ),
                Outcome::Failed(_) => {
                    assert_eq!(app.error.as_deref(), Some("Ошибка проверки"));
                    assert!(
                        app.operation_notice
                            .as_ref()
                            .unwrap()
                            .contains("Operation is incomplete")
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

    #[test]
    fn latest_quick_access_migrates_and_round_trips_empty_and_custom_paths() {
        use crate::files::Settings;
        let old: Settings = serde_json::from_str(r#"{"dark":true}"#).unwrap();
        assert!(old.dark);
        assert_eq!(old.quick_access, Settings::default().quick_access);
        let empty: Settings = serde_json::from_str(r#"{"quick_access":[]}"#).unwrap();
        assert!(empty.quick_access.is_empty());
        let dir = tempfile::tempdir().unwrap();
        for paths in [
            vec![],
            vec![
                dir.path().join("one/Downloads"),
                dir.path().join("two/Downloads"),
            ],
        ] {
            let settings = Settings {
                quick_access: paths.clone(),
                ..Settings::default()
            };
            let round: Settings =
                serde_json::from_slice(&serde_json::to_vec(&settings).unwrap()).unwrap();
            assert_eq!(round.quick_access, paths);
        }
    }

    #[test]
    fn latest_sidebar_names_and_context_routing_do_not_change_file_selection() {
        let (dir, mut app) = sample(3);
        let pinned = dir.path().join("Actual-name");
        std::fs::create_dir(&pinned).unwrap();
        app.settings.quick_access = vec![pinned.clone()];
        app.selection.insert(dir.path().join("grid-000"));
        let selection = app.selection.clone();
        let size = Size::new(1120., 760.);
        let (g, _, _) = inspect(&app, size, &[]);
        assert!(g.labels.contains(&"Quick access".into()));
        assert!(g.labels.contains(&"Actual-name".into()));
        assert!(
            !g.labels
                .iter()
                .any(|s| ["Главная", "Рабочий стол"].contains(&s.as_str()))
        );
        // Три навигационные иконки, одна папка, диск и история. Нижняя палитра удалена.
        assert_eq!(g.icons.iter().filter(|(r, _)| r.x < 210.).count(), 6);
        let r = g.texts.iter().find(|(s, _)| s == "Actual-name").unwrap().1;
        for (point, expected) in [
            (Point::new(r.center_x(), r.center_y()), Some(pinned.clone())),
            (Point::new(40., 500.), None),
        ] {
            let (_, _, messages) = inspect(
                &app,
                size,
                &[(
                    iced::Event::Mouse(iced::mouse::Event::ButtonPressed(
                        iced::mouse::Button::Right,
                    )),
                    point,
                )],
            );
            assert_eq!(
                messages.len(),
                1,
                "Правый клик должен открыть ровно одно меню: {messages:?}"
            );
            assert!(
                matches!(&messages[0],Message::SidebarContext(target) if target == &expected),
                "{messages:?}"
            );
            apply(&mut app, messages[0].clone());
            assert_eq!(app.selection, selection);
            let (menu, _, _) = inspect(&app, size, &[]);
            assert!(menu.labels.contains(&"Pin current folder".into()));
            assert!(
                !menu.labels.iter().any(|s| {
                    ["Move to Trash", "New file", "Cut", "Rename"].contains(&s.as_str())
                })
            );
            assert!(menu.labels.iter().any(|s| s.starts_with("Unpin")));
            apply(&mut app, Message::CloseContext);
        }
        app.page = 2;
        apply(&mut app, Message::Pin(pinned.clone()));
        assert_eq!(app.settings.quick_access, vec![pinned.clone()]);
        apply(&mut app, Message::Pin(dir.path().into()));
        assert_eq!(app.settings.quick_access.len(), 2);
        assert_eq!(app.page, 2);
        assert_eq!(app.selection, selection);
        apply(&mut app, Message::Unpin(pinned.clone()));
        assert_eq!(app.settings.quick_access, vec![dir.path().to_path_buf()]);
        assert!(pinned.is_dir());
        assert_eq!(std::fs::read(dir.path().join("grid-000")).unwrap(), b"test");
    }

    #[test]
    fn latest_sidebar_and_file_scrollbars_share_style_and_independent_timers() {
        let (dir, mut app) = sample(80);
        app.settings.quick_access = (0..30)
            .map(|n| dir.path().join(format!("pin-{n}")))
            .collect();
        let size = Size::new(1120., 760.);
        for dark in [false, true] {
            app.settings.dark = dark;
            app.last_file_scroll = None;
            app.last_sidebar_scroll = None;
            let (g, _, _) = inspect(&app, size, &[]);
            assert!(g.all_thumbs.iter().all(|(_, c, _)| c.a == 0.));
            app.last_sidebar_scroll = Some(Instant::now());
            let (g, _, _) = inspect(&app, size, &[]);
            assert!(g.all_thumbs.iter().any(|(r, c, _)| r.x < 230. && c.a > 0.3));
            assert!(
                g.all_thumbs
                    .iter()
                    .filter(|(r, _, _)| r.x > 230.)
                    .all(|(_, c, _)| c.a == 0.)
            );
            app.last_file_scroll = Some(Instant::now());
            let (g, _, _) = inspect(&app, size, &[]);
            let left = g
                .all_thumbs
                .iter()
                .find(|(r, c, _)| r.x < 230. && c.a > 0.3)
                .unwrap();
            let right = g
                .all_thumbs
                .iter()
                .find(|(r, c, _)| r.x > 230. && c.a > 0.3)
                .unwrap();
            approx(left.0.width, 5.);
            approx(right.0.width, 5.);
            approx(left.2, 3.);
            approx(right.2, 3.);
            assert_eq!(left.1, right.1);
            app.last_sidebar_scroll = Some(Instant::now() - Duration::from_millis(1100));
            app.last_file_scroll = Some(Instant::now());
            apply(&mut app, Message::Frame);
            assert!(app.last_sidebar_scroll.is_none());
            assert!(app.last_file_scroll.is_some());
            apply(&mut app, Message::SidebarScrolled);
            assert!(app.last_sidebar_scroll.is_some());
        }
        let (_, _, messages) = inspect(
            &app,
            size,
            &[(
                iced::Event::Mouse(iced::mouse::Event::WheelScrolled {
                    delta: iced::mouse::ScrollDelta::Lines { x: 0., y: -2. },
                }),
                Point::new(100., 300.),
            )],
        );
        assert!(
            messages
                .iter()
                .any(|m| matches!(m, Message::SidebarScrolled)),
            "{messages:?}"
        );
        // Первый кадр нового дерева может сообщить начальный viewport сетки,
        // поэтому сравниваем его с таким же кадром без колеса.
        let (_, _, baseline_messages) = inspect(&app, size, &[]);
        assert_eq!(
            messages
                .iter()
                .filter(|m| matches!(m, Message::FilesScrolled))
                .count(),
            baseline_messages
                .iter()
                .filter(|m| matches!(m, Message::FilesScrolled))
                .count(),
        );
    }

    #[test]
    fn latest_grid_uses_material_files_folders_and_link_overlays() {
        use std::os::unix::fs::symlink;
        let (dir, mut app) = sample(1);
        std::fs::create_dir(dir.path().join("folder")).unwrap();
        symlink("folder", dir.path().join("folder-link")).unwrap();
        symlink("grid-000", dir.path().join("file-link")).unwrap();
        let (background, rx) = Background::start().unwrap();
        app.background = Some(background.clone());
        app.refresh();
        let id = app.loading.unwrap();
        loop {
            let e = rx.recv_blocking().unwrap();
            let done = matches!(e,Event::Finished{id:finished,..}if finished==id);
            app.apply(e);
            if done {
                break;
            }
        }
        background.shutdown();
        app.background = None;
        let (g, _, _) = inspect(&app, Size::new(1120., 760.), &[]);
        let handles: Vec<_> = g
            .icons
            .iter()
            .zip(&g.svg_handles)
            .filter(|((r, _), _)| r.x > 200.)
            .collect();
        assert_eq!(handles.len(), 6);
        for (bytes, count, size) in [
            (
                include_bytes!("../assets/icons/material/folder.svg").as_slice(),
                2,
                68.,
            ),
            (
                include_bytes!("../assets/icons/material/file.svg").as_slice(),
                2,
                68.,
            ),
            (
                include_bytes!("../assets/icons/material/file-symlink.svg").as_slice(),
                2,
                26.,
            ),
        ] {
            let handle = iced::widget::svg::Handle::from_memory(bytes);
            let matching: Vec<_> = handles.iter().filter(|(_, h)| **h == handle).collect();
            assert_eq!(matching.len(), count);
            for ((r, _), _) in matching {
                approx(r.width, size);
                approx(r.height, size);
            }
        }
    }

    fn finish_read(app: &mut App, rx: &async_channel::Receiver<Event>) {
        let id = app.loading.expect("Ожидалось чтение каталога");
        loop {
            let e = rx.recv_blocking().unwrap();
            let done = matches!(e,Event::Finished{id:n,..}if n==id);
            app.apply(e);
            if done {
                break;
            }
        }
    }
    #[test]
    fn latest_mouse_back_forward_use_history_and_respect_modal_guards() {
        let dir = tempfile::tempdir().unwrap();
        let first = dir.path().join("first");
        let second = dir.path().join("second");
        std::fs::create_dir(&first).unwrap();
        std::fs::create_dir(&second).unwrap();
        let mut app = App::default();
        let (bg, rx) = Background::start().unwrap();
        app.background = Some(bg.clone());
        app.navigate(first.clone());
        finish_read(&mut app, &rx);
        app.navigate(second.clone());
        finish_read(&mut app, &rx);
        let back = || {
            Message::Keyboard(iced::Event::Mouse(iced::mouse::Event::ButtonPressed(
                iced::mouse::Button::Back,
            )))
        };
        for guard in 0..6 {
            match guard {
                0 => app.show_path = true,
                1 => app.show_palette = true,
                2 => app.context_menu = Some((None, Point::ORIGIN)),
                3 => app.dialog = Some(Dialog::Create(true)),
                4 => app.conflict = Some(Command::Create(dir.path().join("unused"), true)),
                _ => app.closing = true,
            }
            apply(&mut app, back());
            assert!(app.loading.is_none());
            assert_eq!(app.directory, second);
            app.show_path = false;
            app.show_palette = false;
            app.context_menu = None;
            app.dialog = None;
            app.conflict = None;
            app.closing = false;
        }
        apply(&mut app, back());
        finish_read(&mut app, &rx);
        assert_eq!(app.directory, first);
        apply(
            &mut app,
            Message::Keyboard(iced::Event::Mouse(iced::mouse::Event::ButtonReleased(
                iced::mouse::Button::Back,
            ))),
        );
        assert!(app.loading.is_none());
        apply(
            &mut app,
            Message::Keyboard(iced::Event::Mouse(iced::mouse::Event::ButtonPressed(
                iced::mouse::Button::Forward,
            ))),
        );
        finish_read(&mut app, &rx);
        assert_eq!(app.directory, second);
        apply(
            &mut app,
            Message::Keyboard(iced::Event::Mouse(iced::mouse::Event::ButtonPressed(
                iced::mouse::Button::Forward,
            ))),
        );
        assert!(app.loading.is_none());
        bg.shutdown();
    }
    #[test]
    fn latest_settings_storage_isolated_child() {
        let dir = tempfile::tempdir().unwrap();
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--ignored",
                "--exact",
                "ui::grid_tests::latest_storage_probe",
                "--nocapture",
            ])
            .env("PAPKA_STORAGE_TEST_ROOT", dir.path())
            .env("XDG_CONFIG_HOME", dir.path().join("config"))
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(String::from_utf8_lossy(&output.stdout).contains("1 passed"));
    }
    #[test]
    #[ignore]
    fn latest_storage_probe() {
        let root = PathBuf::from(std::env::var_os("PAPKA_STORAGE_TEST_ROOT").unwrap());
        let pin = root.join("Some-folder");
        std::fs::create_dir(&pin).unwrap();
        let mut app = App::default();
        app.settings.quick_access.clear();
        app.page = 2;
        let (bg, rx) = Background::start().unwrap();
        app.background = Some(bg.clone());
        for adding in [true, false] {
            if adding {
                apply(&mut app, Message::Pin(pin.clone()));
            } else {
                apply(&mut app, Message::Unpin(pin.clone()));
            }
            let barrier = bg
                .submit(|_| crate::operations::Completion {
                    outcome: crate::operations::Outcome::Success,
                    completed: None,
                    remaining: None,
                    undo_available: false,
                })
                .unwrap()
                .id;
            loop {
                let e = rx.recv_blocking().unwrap();
                let done = matches!(e,Event::Finished{id,..}if id==barrier);
                app.apply(e);
                if done {
                    break;
                }
            }
            assert!(app.error.is_none(), "{:?}", app.error);
            assert_eq!(
                crate::files::settings_load().unwrap().quick_access,
                app.settings.quick_access
            );
            assert_eq!(app.page, 2);
            assert!(app.loading.is_none());
            assert!(pin.is_dir());
        }
        apply(&mut app, Message::Palette);
        apply(&mut app, Message::PaletteSelect(1));
        apply(&mut app, Message::PaletteColor(1, "ABCDEF".into()));
        apply(&mut app, Message::PalettePoint(1, [0.1, 0.9]));
        apply(&mut app, Message::PaletteIntensity(0.73));
        apply(&mut app, Message::ClosePalette);
        let barrier = bg
            .submit(|_| crate::operations::Completion {
                outcome: crate::operations::Outcome::Success,
                completed: None,
                remaining: None,
                undo_available: false,
            })
            .unwrap()
            .id;
        loop {
            let e = rx.recv_blocking().unwrap();
            let done = matches!(e,Event::Finished{id,..}if id==barrier);
            app.apply(e);
            if done {
                break;
            }
        }
        let saved = crate::files::settings_load().unwrap();
        assert_eq!(saved.palette, app.settings.palette);
        assert_eq!(saved.palette_positions, app.settings.palette_positions);
        assert_eq!(saved.palette_intensity, 0.73);
        assert_eq!(saved.palette_harmony, app.settings.palette_harmony);
        assert_eq!(app.page, 2);
        assert!(app.loading.is_none());
        bg.shutdown();
    }

    #[test]
    fn latest_subscription_delivers_captured_side_buttons() {
        use iced::futures::{StreamExt, executor::block_on, stream};
        use iced_futures::subscription::{Event as Input, into_recipes};
        let app = App::default();
        // Вторая подписка приложения — фильтр событий; фоновый worker не запускается.
        let recipe = into_recipes(subscription(&app)).into_iter().nth(1).unwrap();
        let mut events = Vec::new();
        for status in [iced::event::Status::Captured, iced::event::Status::Ignored] {
            for button in [iced::mouse::Button::Back, iced::mouse::Button::Forward] {
                events.push(Input::Interaction {
                    window: iced::window::Id::unique(),
                    event: iced::Event::Mouse(iced::mouse::Event::ButtonPressed(button)),
                    status,
                });
            }
        }
        events.push(Input::Interaction {
            window: iced::window::Id::unique(),
            event: iced::Event::Mouse(iced::mouse::Event::ButtonPressed(iced::mouse::Button::Left)),
            status: iced::event::Status::Captured,
        });
        let result: Vec<_> = block_on(recipe.stream(Box::pin(stream::iter(events))).collect());
        assert_eq!(result.len(), 4);
        assert!(result.iter().all(|m| matches!(
            m,
            Message::Keyboard(iced::Event::Mouse(iced::mouse::Event::ButtonPressed(
                iced::mouse::Button::Back | iced::mouse::Button::Forward
            )))
        )));
    }
    #[test]
    fn refactor_appearance_messages_validate_hex_and_dispatch_all_preferences() {
        let mut app = App::default();
        let directory = app.directory.clone();
        app.settings.palette = [0x102030, 0x405060, 0x708090];
        apply(&mut app, Message::Palette);
        assert!(app.show_palette);
        assert_eq!(app.palette_inputs, ["102030", "405060", "708090"]);
        for (input, expected) in [
            ("#aBcDeF", 0xabcdef),
            ("bad", 0xabcdef),
            ("GGGGGG", 0xabcdef),
        ] {
            apply(&mut app, Message::PaletteColor(1, input.into()));
            assert_eq!(app.settings.palette[1], expected);
            assert_eq!(app.palette_inputs[1], input);
        }
        let colors = app.settings.palette;
        apply(&mut app, Message::PaletteColor(3, "FFFFFF".into()));
        assert_eq!(app.settings.palette, colors);
        for (input, expected) in [(0, 1), (2, 2), (255, 3)] {
            apply(&mut app, Message::PaletteCount(input));
            assert_eq!(app.settings.palette_count, expected);
        }
        apply(&mut app, Message::Preset([0, 0x123456, 0xffffff]));
        assert_eq!(app.palette_inputs, ["000000", "123456", "FFFFFF"]);
        apply(&mut app, Message::Dark(true));
        apply(&mut app, Message::Reduced(true));
        apply(&mut app, Message::Descending(true));
        apply(&mut app, Message::Sort(crate::files::Sort::Modified));
        app.selection.insert(directory.join("selected"));
        apply(&mut app, Message::Hidden(true));
        assert!(
            app.settings.dark
                && app.settings.reduced_motion
                && app.settings.descending
                && app.settings.hidden
        );
        assert_eq!(app.settings.sort, crate::files::Sort::Modified);
        assert!(app.selection.is_empty());
        assert_eq!(app.directory, directory);
        apply(&mut app, Message::ClosePalette);
        assert!(!app.show_palette);
    }

    #[test]
    fn refactor_frame_expires_timers_independently_without_sleep() {
        let mut app = App::default();
        let now = Instant::now();
        app.last_file_scroll = Some(now - Duration::from_secs(2));
        app.last_sidebar_scroll = Some(now);
        app.transition = Some(now - Duration::from_millis(500));
        apply(&mut app, Message::Frame);
        assert!(app.last_file_scroll.is_none());
        assert_eq!(app.last_sidebar_scroll, Some(now));
        assert!(app.transition.is_none());
        app.last_sidebar_scroll = Some(now - Duration::from_secs(2));
        apply(&mut app, Message::Frame);
        assert!(app.last_sidebar_scroll.is_none());
    }

    #[test]
    fn refactor_escape_reaches_each_editor_after_input_extraction() {
        use iced::keyboard::{Key, Location, Modifiers, key};
        let escape = || {
            Message::Keyboard(iced::Event::Keyboard(keyboard::Event::KeyPressed {
                key: Key::Named(Named::Escape),
                modified_key: Key::Named(Named::Escape),
                physical_key: key::Physical::Code(key::Code::Escape),
                location: Location::Standard,
                modifiers: Modifiers::default(),
                text: None,
                repeat: false,
            }))
        };
        let mut app = App::default();
        app.selection.insert(PathBuf::from("selected"));
        app.show_palette = true;
        apply(&mut app, escape());
        assert!(!app.show_palette);
        app.show_path = true;
        app.path_dirty = true;
        apply(&mut app, escape());
        assert!(!app.show_path && !app.path_dirty);
        app.dialog = Some(Dialog::Create(true));
        apply(&mut app, escape());
        assert!(app.dialog.is_none());
        assert_eq!(app.selection.len(), 1);
    }

    #[test]
    fn refactor_grid_and_footer_agree_at_page_boundary_and_stale_page() {
        let (_dir, mut app) = sample(201);
        let size = Size::new(1120., 760.);
        let (first, _, _) = inspect(&app, size, &[]);
        assert_eq!(first.names.len(), 200);
        assert!(first.labels.iter().any(|label| label == "1 / 2"));
        apply(&mut app, Message::Page(true));
        let (last, _, _) = inspect(&app, size, &[]);
        assert_eq!(last.names.len(), 1);
        assert_eq!(last.names[0].0, "grid-200");
        assert!(last.labels.iter().any(|label| label == "2 / 2"));
        app.page = usize::MAX;
        let (stale, _, _) = inspect(&app, size, &[]);
        assert_eq!(stale.names.len(), 1);
        assert!(stale.labels.iter().any(|label| label == "2 / 2"));
        app.page = 0;
        apply(&mut app, Message::Settings);
        let (settings, _, _) = inspect(&app, size, &[]);
        assert!(
            settings
                .labels
                .iter()
                .any(|label| label == "Show hidden files")
        );
        apply(&mut app, Message::History);
        assert!(!app.show_settings && app.show_history);
        let (history, _, _) = inspect(&app, size, &[]);
        assert!(history.labels.iter().any(|label| label == "No history yet"));
    }

    #[test]
    fn latest_two_line_names_keep_equal_height_and_extensions() {
        let (_dir, mut app) = sample(18);
        let names = [
            "grid-short",
            "grid-zapret-discord-youtube-linux",
            "grid-ОченьДлинноеНазваниеБезПробеловОченьДлинноеНазвание.tar.gz",
            "grid-e\u{301}e\u{301}e\u{301}👩‍💻👩‍💻👩‍💻longlonglonglong.txt",
            "grid-строка\nдругая\tстрокаОченьДлинная",
        ];
        for (i, e) in app.entries.iter_mut().enumerate() {
            e.name = names[i % names.len()].into();
        }
        for width in [650., 900., 1120.] {
            app.window_width = width;
            let (g, _, _) = inspect(&app, Size::new(width, 900.), &[]);
            let labels: Vec<_> = g
                .texts
                .iter()
                .filter(|(s, _)| s.starts_with("grid-"))
                .collect();
            assert_eq!(labels.len(), 18);
            for (label, r) in &labels {
                approx(r.height, 32.);
                assert!(label.lines().count() <= 2, "{label:?}");
                assert!(!label.contains('\t'));
                if label.contains("Очень") {
                    assert!(label.ends_with(".tar.gz"), "{label}");
                }
                assert!(
                    !label.contains("e…"),
                    "составная графема разрезана: {label}"
                );
            }
            let mut ys: Vec<_> = labels.iter().map(|(_, r)| r.y).collect();
            ys.sort_by(f32::total_cmp);
            ys.dedup_by(|a, b| (*a - *b).abs() < 0.1);
            for pair in ys.windows(2) {
                approx(pair[1] - pair[0], 122.);
            }
        }
    }

    #[test]
    fn latest_sidebar_resize_limits_release_and_modal_guards() {
        let mut app = App::default();
        app.pointer = Point::new(208., 200.);
        apply(&mut app, Message::SidebarResizeStart);
        assert!(app.sidebar_drag_offset.is_some());
        apply(
            &mut app,
            Message::Keyboard(iced::Event::Mouse(iced::mouse::Event::CursorMoved {
                position: Point::new(308., 200.),
            })),
        );
        approx(app.sidebar_width, 296.);
        for x in [-300., 2000.] {
            apply(
                &mut app,
                Message::Keyboard(iced::Event::Mouse(iced::mouse::Event::CursorMoved {
                    position: Point::new(x, 200.),
                })),
            );
            assert!((160.0..=480.0).contains(&app.sidebar_width));
        }
        for end in [
            iced::Event::Mouse(iced::mouse::Event::ButtonReleased(
                iced::mouse::Button::Left,
            )),
            iced::Event::Mouse(iced::mouse::Event::CursorLeft),
            iced::Event::Window(iced::window::Event::Unfocused),
        ] {
            app.pointer = Point::new(200., 200.);
            apply(&mut app, Message::SidebarResizeStart);
            apply(&mut app, Message::Keyboard(end));
            assert!(app.sidebar_drag_offset.is_none());
        }
        app.show_palette = true;
        apply(&mut app, Message::SidebarResizeStart);
        assert!(app.sidebar_drag_offset.is_none());
        app.show_palette = false;
        app.window_width = 400.;
        assert!(sidebar::sidebar_width(&app) <= 136.);
    }

    #[test]
    fn latest_palette_points_counts_layouts_and_page_are_preserved() {
        let mut app = App::default();
        app.page = 2;
        let selected = app.directory.join("selection");
        app.selection.insert(selected);
        let selection = app.selection.clone();
        app.settings.palette = [0x112233, 0x445566, 0x778899];
        // Независимые пользовательские цвета проверяются в свободной схеме.
        app.settings.palette_harmony = None;
        apply(&mut app, Message::Palette);
        apply(&mut app, Message::PaletteSelect(1));
        apply(&mut app, Message::PaletteCount(2));
        assert_eq!(app.settings.palette[..2], [0x112233, 0x778899]);
        apply(&mut app, Message::PaletteCount(1));
        apply(&mut app, Message::PaletteCount(2));
        assert_eq!(app.palette_active, 1);
        apply(&mut app, Message::PalettePoint(1, [-1., 2.]));
        let p = app.settings.wheel_position(1);
        assert!(((p[0] - 0.5).hypot(p[1] - 0.5) - 0.5).abs() < 0.00001);
        assert!(p[0] < 0.5 && p[1] > 0.5);
        let original = app.settings.palette_positions;
        for _ in 0..4 {
            apply(&mut app, Message::PaletteCombination);
        }
        assert_ne!(app.settings.palette_positions, original);
        apply(&mut app, Message::PaletteColor(1, "ABCDEF".into()));
        apply(&mut app, Message::PaletteIntensity(0.8));
        apply(&mut app, Message::PaletteCommit);
        assert_eq!(app.page, 2);
        assert_eq!(app.selection, selection);
        assert!(app.loading.is_none());
        let old: crate::files::Settings =
            serde_json::from_str(r#"{"palette":[1,2,3],"palette_count":2}"#).unwrap();
        assert_eq!(
            old.palette_positions,
            crate::files::Settings::default().palette_positions
        );
        let restored: crate::files::Settings =
            serde_json::from_slice(&serde_json::to_vec(&app.settings).unwrap()).unwrap();
        assert_eq!(restored.palette_positions, app.settings.palette_positions);
        assert_eq!(restored.palette_intensity, 0.8);
    }

    #[test]
    fn latest_palette_canvas_drag_release_and_unfocus() {
        use iced::widget::canvas::Program;
        let settings = crate::files::Settings::default();
        let field = palette_field::Field {
            settings: &settings,
            active: 0,
            editor: true,
        };
        let mut state = palette_field::State::default();
        let bounds = Rectangle::new(Point::ORIGIN, Size::new(330., 300.));
        let p = Point::new(
            165. + (settings.wheel_position(0)[0] - 0.5) * 244.,
            150. + (settings.wheel_position(0)[1] - 0.5) * 244.,
        );
        let press =
            iced::Event::Mouse(iced::mouse::Event::ButtonPressed(iced::mouse::Button::Left));
        let a = field
            .update(
                &mut state,
                &press,
                bounds,
                iced::mouse::Cursor::Available(p),
            )
            .unwrap()
            .into_inner();
        assert!(matches!(a.0, Some(Message::PaletteSelect(0))));
        assert_eq!(a.2, iced::event::Status::Captured);
        let moved = Point::new(300., 250.);
        let a = field
            .update(
                &mut state,
                &iced::Event::Mouse(iced::mouse::Event::CursorMoved { position: moved }),
                bounds,
                iced::mouse::Cursor::Available(moved),
            )
            .unwrap()
            .into_inner();
        assert!(matches!(a.0, Some(Message::PalettePoint(0, _))));
        let release = iced::Event::Mouse(iced::mouse::Event::ButtonReleased(
            iced::mouse::Button::Left,
        ));
        let a = field
            .update(
                &mut state,
                &release,
                bounds,
                iced::mouse::Cursor::Available(moved),
            )
            .unwrap()
            .into_inner();
        assert!(matches!(a.0, Some(Message::PaletteCommit)));
        assert_ne!(
            field.mouse_interaction(&state, bounds, iced::mouse::Cursor::Unavailable),
            iced::mouse::Interaction::Grabbing
        );
        field.update(
            &mut state,
            &press,
            bounds,
            iced::mouse::Cursor::Available(p),
        );
        field.update(
            &mut state,
            &iced::Event::Window(iced::window::Event::Unfocused),
            bounds,
            iced::mouse::Cursor::Unavailable,
        );
        assert_ne!(
            field.mouse_interaction(&state, bounds, iced::mouse::Cursor::Unavailable),
            iced::mouse::Interaction::Grabbing,
            "после потери фокуса жест должен завершаться"
        );
        assert!(
            field
                .update(
                    &mut state,
                    &iced::Event::Mouse(iced::mouse::Event::CursorMoved { position: moved }),
                    bounds,
                    iced::mouse::Cursor::Available(moved)
                )
                .is_none()
        );
        assert!(
            field
                .update(
                    &mut state,
                    &press,
                    bounds,
                    iced::mouse::Cursor::Available(p)
                )
                .is_some()
        );
        assert_eq!(
            field.mouse_interaction(&state, bounds, iced::mouse::Cursor::Available(p)),
            iced::mouse::Interaction::Grabbing
        );
    }

    #[test]
    fn latest_context_palette_action_and_path_editor_without_go() {
        let (_dir, mut app) = sample(1);
        let size = Size::new(1120., 760.);
        app.pointer = Point::new(40., 300.);
        apply(&mut app, Message::SidebarContext(None));
        let (g, _, _) = inspect(&app, size, &[]);
        let change = g
            .texts
            .iter()
            .find(|(s, _)| s == "Change colors")
            .unwrap()
            .1;
        let p = Point::new(change.center_x(), change.center_y());
        let (_, _, msgs) = inspect(
            &app,
            size,
            &[
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
            ],
        );
        for message in msgs {
            apply(&mut app, message);
        }
        assert!(app.show_palette);
        assert!(app.context_menu.is_none());
        let (g, _, _) = inspect(&app, size, &[]);
        for text in [
            "Appearance colors",
            "Color intensity",
            "Reset colors",
            "Light",
            "Dark",
            "Scattered",
        ] {
            assert!(!g.labels.iter().any(|s| s == text));
        }
        apply(&mut app, Message::ClosePalette);
        apply(&mut app, Message::EditPath);
        let (g, _, _) = inspect(&app, size, &[]);
        assert!(!g.labels.iter().any(|s| s == "Go"));
        assert!(g.labels.iter().any(|s| s == "Copy full path"));
        let r = g.inputs[0];
        let p = Point::new(r.center_x(), r.center_y());
        let key = iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
            key: iced::keyboard::Key::Named(iced::keyboard::key::Named::Enter),
            modified_key: iced::keyboard::Key::Named(iced::keyboard::key::Named::Enter),
            physical_key: iced::keyboard::key::Physical::Code(iced::keyboard::key::Code::Enter),
            location: iced::keyboard::Location::Standard,
            modifiers: iced::keyboard::Modifiers::default(),
            text: None,
            repeat: false,
        });
        let (_, _, msgs) = inspect(
            &app,
            size,
            &[
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
                (key, p),
            ],
        );
        assert!(
            msgs.iter().any(|m| matches!(m, Message::Go)),
            "Enter в поле пути: {msgs:?}"
        );
    }

    #[test]
    fn latest_resize_subscription_delivers_captured_and_ignored_events() {
        use iced::futures::{StreamExt, executor::block_on, stream};
        use iced_futures::subscription::{Event as Input, into_recipes};
        let app = App::default();
        let recipe = into_recipes(subscription(&app)).into_iter().nth(1).unwrap();
        let mut inputs = Vec::new();
        for status in [iced::event::Status::Captured, iced::event::Status::Ignored] {
            for event in [
                iced::Event::Mouse(iced::mouse::Event::ButtonReleased(
                    iced::mouse::Button::Left,
                )),
                iced::Event::Mouse(iced::mouse::Event::CursorLeft),
                iced::Event::Window(iced::window::Event::Unfocused),
                iced::Event::Window(iced::window::Event::Resized(Size::new(600., 500.))),
            ] {
                inputs.push(Input::Interaction {
                    window: iced::window::Id::unique(),
                    event,
                    status,
                });
            }
        }
        let result: Vec<_> = block_on(recipe.stream(Box::pin(stream::iter(inputs))).collect());
        assert_eq!(result.len(), 8);
        assert!(result.iter().all(|m| matches!(m, Message::Keyboard(_))));
    }
    #[test]
    fn latest_file_hover_does_not_create_name_tooltip() {
        let (_dir, app) = sample(1);
        let size = Size::new(1120., 760.);
        let (g, _, _) = inspect(&app, size, &[]);
        let r = g.names[0].1;
        let p = Point::new(r.center_x(), r.center_y());
        let (g, _, _) = inspect(
            &app,
            size,
            &[(
                iced::Event::Mouse(iced::mouse::Event::CursorMoved { position: p }),
                p,
            )],
        );
        assert!(
            !g.has_overlay,
            "наведение на имя файла не должно создавать tooltip overlay"
        );
    }
    #[test]
    fn harmony_hex_validation_explains_unapplied_input_and_recovers() {
        let mut app = App::default();
        apply(&mut app, Message::Palette);
        let before = app.settings.palette;
        for value in [
            "",
            "12345",
            "1234567",
            "GG0000",
            "##123456",
            "+12345",
            "１２３４５６",
        ] {
            apply(&mut app, Message::PaletteColor(0, value.into()));
            assert_eq!(
                app.settings.palette, before,
                "невалидный {value:?} изменил палитру"
            );
            assert!(!app.palette_input_valid(0));
        }
        let (g, _, _) = inspect(&app, Size::new(1120., 900.), &[]);
        assert!(
            g.labels
                .iter()
                .any(|s| s == "Enter 6 HEX digits (RRGGBB). Color not applied.")
        );
        apply(&mut app, Message::PaletteColor(0, " #aBcDeF ".into()));
        assert!(app.palette_input_valid(0));
        assert_eq!(app.settings.palette[0], 0xabcdef);
        let (g, _, _) = inspect(&app, Size::new(1120., 900.), &[]);
        assert!(!g.labels.iter().any(|s| s.contains("Color not applied")));
    }

    #[test]
    fn harmony_theme_and_all_palette_messages_preserve_page_and_selection() {
        let mut app = App::default();
        app.page = 3;
        app.selection
            .insert(app.directory.join("temporary-selection-marker"));
        let selection = app.selection.clone();
        for message in [
            Message::Palette,
            Message::Dark(true),
            Message::Dark(false),
            Message::PaletteSelect(1),
            Message::PaletteColor(1, "ABCDEF".into()),
            Message::PalettePoint(1, [0.4, 0.3]),
            Message::PaletteCombination,
            Message::PaletteCount(2),
            Message::PaletteIntensity(0.7),
            Message::PaletteCommit,
            Message::Preset(crate::files::Settings::DEFAULT_PALETTE),
            Message::ClosePalette,
        ] {
            apply(&mut app, message);
            assert_eq!(app.page, 3);
            assert_eq!(app.selection, selection);
            assert!(app.loading.is_none());
        }
    }

    #[test]
    fn harmony_overlapping_points_can_be_selected_without_moving_another_point() {
        let mut app = App::default();
        app.settings
            .apply_palette_harmony(crate::files::PaletteHarmony::Custom);
        app.settings.palette_positions = [[0.6, 0.5]; 3];
        app.settings.palette = [0xabcdef; 3];
        apply(&mut app, Message::Palette);
        let colors = app.settings.palette;
        let positions = app.settings.palette_positions;
        for index in 0..3 {
            let (g, _, _) = inspect(&app, Size::new(1120., 900.), &[]);
            let label = (index + 1).to_string();
            let r = g.texts.iter().find(|(s, _)| s == &label).unwrap().1;
            let point = Point::new(r.center_x(), r.center_y());
            let (_, _, messages) = inspect(
                &app,
                Size::new(1120., 900.),
                &[
                    (
                        iced::Event::Mouse(iced::mouse::Event::ButtonPressed(
                            iced::mouse::Button::Left,
                        )),
                        point,
                    ),
                    (
                        iced::Event::Mouse(iced::mouse::Event::ButtonReleased(
                            iced::mouse::Button::Left,
                        )),
                        point,
                    ),
                ],
            );
            assert!(
                messages
                    .iter()
                    .any(|m| matches!(m, Message::PaletteSelect(i) if *i == index)),
                "не выбран индекс {index}: {messages:?}"
            );
            for message in messages {
                apply(&mut app, message);
            }
            assert_eq!(app.palette_active, index);
            assert_eq!(app.settings.palette, colors);
            assert_eq!(app.settings.palette_positions, positions);
        }
    }

    #[test]
    fn harmony_canvas_drag_preserves_grab_offset_and_clamps_in_non_square_bounds() {
        use iced::widget::canvas::Program;
        let settings = crate::files::Settings::default();
        let field = palette_field::Field {
            settings: &settings,
            active: 0,
            editor: true,
        };
        let mut state = palette_field::State::default();
        let bounds = Rectangle::new(Point::new(70., 90.), Size::new(330., 300.));
        let p = settings.wheel_position(0);
        let center = Point::new(
            bounds.center_x() + (p[0] - 0.5) * 244.,
            bounds.center_y() + (p[1] - 0.5) * 244.,
        );
        let grab = center + iced::Vector::new(8., -4.);
        let press =
            iced::Event::Mouse(iced::mouse::Event::ButtonPressed(iced::mouse::Button::Left));
        field
            .update(
                &mut state,
                &press,
                bounds,
                iced::mouse::Cursor::Available(grab),
            )
            .unwrap();
        let movement = iced::Event::Mouse(iced::mouse::Event::CursorMoved { position: grab });
        let action = field
            .update(
                &mut state,
                &movement,
                bounds,
                iced::mouse::Cursor::Available(grab),
            )
            .unwrap()
            .into_inner();
        let Some(Message::PalettePoint(0, actual)) = action.0 else {
            panic!("{:?}", action.0);
        };
        for axis in 0..2 {
            assert!((p[axis] - actual[axis]).abs() < 0.00001);
        }
        let outside = Point::new(1000., -1000.);
        let action = field
            .update(
                &mut state,
                &movement,
                bounds,
                iced::mouse::Cursor::Available(outside),
            )
            .unwrap()
            .into_inner();
        let Some(Message::PalettePoint(0, actual)) = action.0 else {
            panic!("{:?}", action.0);
        };
        assert!(((actual[0] - 0.5).hypot(actual[1] - 0.5) - 0.5).abs() < 0.00001);
        let release = iced::Event::Mouse(iced::mouse::Event::CursorLeft);
        assert!(matches!(
            field
                .update(
                    &mut state,
                    &release,
                    bounds,
                    iced::mouse::Cursor::Unavailable
                )
                .unwrap()
                .into_inner()
                .0,
            Some(Message::PaletteCommit)
        ));
        assert!(
            field
                .update(
                    &mut state,
                    &movement,
                    bounds,
                    iced::mouse::Cursor::Available(grab)
                )
                .is_none()
        );
    }

    #[test]
    fn harmony_scheme_label_tracks_count_and_drag_keeps_it() {
        let mut app = App::default();
        apply(&mut app, Message::Palette);
        for count in [3, 2, 1, 2, 3] {
            apply(&mut app, Message::PaletteCount(count));
            let choices = crate::files::PaletteHarmony::choices(count);
            for _ in 0..choices.len() {
                let scheme = app.settings.harmony();
                apply(&mut app, Message::PalettePoint(0, [0.7, 0.4]));
                assert_eq!(app.settings.harmony(), scheme);
                let (g, _, _) = inspect(&app, Size::new(1120., 900.), &[]);
                assert!(g.labels.contains(&scheme.to_string()));
                apply(&mut app, Message::PaletteCombination);
            }
        }
    }
    #[test]
    fn harmony_theme_keeps_shift_selection_anchor() {
        let (_dir, mut app) = sample(3);
        let paths: Vec<_> = app
            .visible()
            .iter()
            .map(|entry| entry.path.clone())
            .collect();
        app.select_context(paths[0].clone());
        app.page = 2;
        apply(&mut app, Message::Dark(true));
        app.selection_modifiers(false, true);
        app.select(paths[2].clone());
        assert_eq!(
            app.selection.len(),
            3,
            "смена темы сбросила якорь выделения"
        );
        assert_eq!(app.page, 2);
    }
}
