//! Круг оттенков со связанными точками и пространственный фон палитры.
use super::{Message, style::rgb};
use crate::files::Settings;
use iced::widget::{
    Action,
    canvas::{self, Frame, Geometry, Path, Stroke},
};
use iced::{Color, Point, Rectangle, Renderer, Size, Theme, mouse};

pub(super) fn position(value: [f32; 2]) -> [f32; 2] {
    value.map(|v| {
        if v.is_finite() {
            v.clamp(0.0, 1.0)
        } else {
            0.5
        }
    })
}

pub(super) struct Field<'a> {
    pub settings: &'a Settings,
    pub active: usize,
    pub editor: bool,
}
#[derive(Default)]
pub(super) struct State {
    dragging: Option<usize>,
    drag_offset: iced::Vector,
    cache: canvas::Cache,
    key: std::cell::RefCell<Vec<u32>>,
}
impl Field<'_> {
    fn center(&self, index: usize, bounds: Rectangle) -> Point {
        let p = self.settings.wheel_position(index);
        let radius = Self::radius(bounds);
        Point::new(
            bounds.width * 0.5 + (p[0] - 0.5) * 2.0 * radius,
            bounds.height * 0.5 + (p[1] - 0.5) * 2.0 * radius,
        )
    }
    fn radius(bounds: Rectangle) -> f32 {
        (bounds.width.min(bounds.height) * 0.5 - 28.0).max(1.0)
    }
    fn point(
        &self,
        bounds: Rectangle,
        cursor: mouse::Cursor,
        offset: iced::Vector,
    ) -> Option<[f32; 2]> {
        let p =
            cursor.position()? - iced::Vector::new(bounds.center_x(), bounds.center_y()) - offset;
        let diameter = 2.0 * Self::radius(bounds);
        Some(crate::files::circle_position([
            0.5 + p.x / diameter,
            0.5 + p.y / diameter,
        ]))
    }
    fn hit(&self, bounds: Rectangle, cursor: mouse::Cursor) -> Option<usize> {
        let p = cursor.position_in(bounds)?;
        // Отдельные переключатели позволяют выбрать скрытую точку;
        // при начале жеста приоритет остаётся у явно выбранной точки.
        std::iter::once(self.active)
            .chain(
                (0..self.settings.palette_count.clamp(1, 3) as usize)
                    .rev()
                    .filter(|&i| i != self.active),
            )
            .find(|&i| {
                p.distance(self.center(i, bounds)) <= if i == self.active { 26.0 } else { 16.0 }
            })
    }
}
impl canvas::Program<Message> for Field<'_> {
    type State = State;
    fn update(
        &self,
        state: &mut State,
        event: &canvas::Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<Action<Message>> {
        if !self.editor {
            return None;
        }
        match event {
            canvas::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                if let Some(index) = self.hit(bounds, cursor) {
                    state.dragging = Some(index);
                    state.drag_offset = cursor.position_in(bounds)? - self.center(index, bounds);
                    Some(Action::publish(Message::PaletteSelect(index)).and_capture())
                } else {
                    let p = cursor.position_in(bounds)?;
                    if p.distance(Point::new(bounds.width * 0.5, bounds.height * 0.5))
                        > Self::radius(bounds)
                    {
                        return None;
                    }
                    state.dragging = Some(self.active);
                    state.drag_offset = iced::Vector::ZERO;
                    Some(
                        Action::publish(Message::PalettePoint(
                            self.active,
                            self.point(bounds, cursor, state.drag_offset)?,
                        ))
                        .and_capture(),
                    )
                }
            }
            canvas::Event::Mouse(mouse::Event::CursorMoved { .. }) if state.dragging.is_some() => {
                let index = state.dragging?;
                let value = self.point(bounds, cursor, state.drag_offset)?;
                Some(Action::publish(Message::PalettePoint(index, value)).and_capture())
            }
            canvas::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left))
            | canvas::Event::Mouse(mouse::Event::CursorLeft)
            | canvas::Event::Window(iced::window::Event::Unfocused)
                if state.dragging.take().is_some() =>
            {
                Some(Action::publish(Message::PaletteCommit).and_capture())
            }
            _ => None,
        }
    }
    fn draw(
        &self,
        state: &State,
        renderer: &Renderer,
        theme: &Theme,
        bounds: Rectangle,
        _: mouse::Cursor,
    ) -> Vec<Geometry> {
        if self.editor {
            if state.key.borrow().as_slice() != [u32::MAX] {
                state.cache.clear();
                *state.key.borrow_mut() = vec![u32::MAX];
            }
            let wheel = state.cache.draw(renderer, bounds.size(), |frame| {
                let center = Point::new(bounds.width * 0.5, bounds.height * 0.5);
                let radius = Self::radius(bounds);
                let (sectors, rings) = (180, 64);
                // Круг кешируется один раз на размер. Перетаскивание меняет лишь линии и точки.
                for ring in 0..rings {
                    let inner = radius * ring as f32 / rings as f32;
                    let outer = (radius * (ring + 1) as f32 / rings as f32 + 0.25).min(radius);
                    let lightness = (ring as f32 + 0.5) / rings as f32;
                    for sector in 0..sectors {
                        let angle = sector as f32 * std::f32::consts::TAU / sectors as f32;
                        let next = (sector as f32 + 1.05) * std::f32::consts::TAU / sectors as f32;
                        let polar = |r: f32, a: f32| {
                            Point::new(center.x + r * a.cos(), center.y + r * a.sin())
                        };
                        let cell = Path::new(|path| {
                            path.move_to(polar(inner, angle));
                            path.line_to(polar(outer, angle));
                            path.line_to(polar(outer, next));
                            path.line_to(polar(inner, next));
                            path.close();
                        });
                        frame.fill(
                            &cell,
                            rgb(crate::files::hsl_color(
                                angle.to_degrees() + 180.0 / sectors as f32,
                                0.9 + 0.1 * lightness,
                                lightness,
                            )),
                        );
                    }
                }
            });
            let mut frame = Frame::new(renderer, bounds.size());
            let count = self.settings.palette_count.clamp(1, 3) as usize;
            if count > 1 && self.settings.harmony() != crate::files::PaletteHarmony::Custom {
                let links = Path::new(|path| {
                    path.move_to(self.center(0, bounds));
                    for index in 1..count {
                        path.line_to(self.center(index, bounds));
                    }
                    if count == 3 {
                        path.close();
                    }
                });
                frame.stroke(
                    &links,
                    Stroke::default().with_width(1.5).with_color(Color {
                        a: 0.5,
                        ..theme.palette().text
                    }),
                );
            }
            for index in (0..count)
                .filter(|&i| i != self.active)
                .chain(std::iter::once(self.active))
            {
                let circle = Path::circle(
                    self.center(index, bounds),
                    if index == self.active { 24.0 } else { 13.0 },
                );
                frame.fill(&circle, rgb(self.settings.palette[index]));
                frame.stroke(
                    &circle,
                    Stroke::default()
                        .with_width(if index == self.active { 5.0 } else { 3.0 })
                        .with_color(Color::WHITE),
                );
            }
            return vec![wheel, frame.into_geometry()];
        }
        let mut key = self.settings.palette.to_vec();
        key.extend(
            self.settings
                .palette_positions
                .iter()
                .flatten()
                .map(|v| v.to_bits()),
        );
        key.extend([
            self.editor as u32,
            self.settings.dark as u32,
            self.settings.palette_count as u32,
            self.settings.palette_intensity.to_bits(),
        ]);
        if *state.key.borrow() != key {
            state.cache.clear();
            *state.key.borrow_mut() = key;
        }
        let background = state.cache.draw(renderer, bounds.size(), |frame| {
            let base = if self.settings.dark { 0.09 } else { 0.97 };
            let strength = if self.settings.palette_intensity.is_finite() {
                self.settings.palette_intensity.clamp(0.0, 1.0)
            } else {
                0.35
            };
            // Мелкая кешируемая сетка; без шума, внешних текстур и файлов.
            let (nx, ny) = (128, 96);
            for y in 0..ny {
                for x in 0..nx {
                    let (u, v) = ((x as f32 + 0.5) / nx as f32, (y as f32 + 0.5) / ny as f32);
                    let mut sum = [0.0; 3];
                    let mut weight = 0.0;
                    for i in 0..self.settings.palette_count.clamp(1, 3) as usize {
                        let p = position(self.settings.palette_positions[i]);
                        let w = (-5.0 * ((u - p[0]).powi(2) + (v - p[1]).powi(2))).exp();
                        let c = rgb(self.settings.palette[i]);
                        for (s, c) in sum.iter_mut().zip([c.r, c.g, c.b]) {
                            *s += w * c;
                        }
                        weight += w;
                    }
                    let c =
                        sum.map(|s| base * (1.0 - strength) + (s / weight.max(0.0001)) * strength);
                    let cell = Size::new(bounds.width / nx as f32, bounds.height / ny as f32);
                    frame.fill_rectangle(
                        Point::new(x as f32 * cell.width, y as f32 * cell.height),
                        Size::new(cell.width + 0.5, cell.height + 0.5),
                        Color::from_rgb(c[0], c[1], c[2]),
                    );
                }
            }
        });
        vec![background]
    }

    fn mouse_interaction(
        &self,
        state: &State,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        if state.dragging.is_some() {
            mouse::Interaction::Grabbing
        } else if self.editor && self.hit(bounds, cursor).is_some() {
            mouse::Interaction::Grab
        } else {
            mouse::Interaction::default()
        }
    }
}
