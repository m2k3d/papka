//! Пространственное смешивание цветов и перетаскиваемые точки редактора.
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
    cache: canvas::Cache,
    key: std::cell::RefCell<Vec<u32>>,
}
impl Field<'_> {
    fn center(&self, index: usize, bounds: Rectangle) -> Point {
        let p = position(self.settings.palette_positions[index]);
        Point::new(
            28.0 + p[0] * (bounds.width - 56.0).max(1.0),
            28.0 + p[1] * (bounds.height - 56.0).max(1.0),
        )
    }
    fn hit(&self, bounds: Rectangle, cursor: mouse::Cursor) -> Option<usize> {
        let p = cursor.position_in(bounds)?;
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
                let index = self.hit(bounds, cursor)?;
                state.dragging = Some(index);
                Some(Action::publish(Message::PaletteSelect(index)).and_capture())
            }
            canvas::Event::Mouse(mouse::Event::CursorMoved { .. }) if state.dragging.is_some() => {
                let index = state.dragging?;
                let p = cursor.position()? - iced::Vector::new(bounds.x, bounds.y);
                let value = position([
                    (p.x - 28.0) / (bounds.width - 56.0).max(1.0),
                    (p.y - 28.0) / (bounds.height - 56.0).max(1.0),
                ]);
                Some(Action::publish(Message::PalettePoint(index, value)).and_capture())
            }
            canvas::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left))
            | canvas::Event::Mouse(mouse::Event::CursorLeft)
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
        _: &Theme,
        bounds: Rectangle,
        _: mouse::Cursor,
    ) -> Vec<Geometry> {
        if self.editor {
            let mut frame = Frame::new(renderer, bounds.size());
            frame.fill(
                &Path::rounded_rectangle(Point::ORIGIN, bounds.size(), 12.0.into()),
                if self.settings.dark {
                    Color::from_rgb8(35, 35, 38)
                } else {
                    Color::from_rgb8(235, 235, 239)
                },
            );
            for index in (0..self.settings.palette_count.clamp(1, 3) as usize)
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
            return vec![frame.into_geometry()];
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
            self.settings.dark as u32,
            self.settings.palette_count as u32,
            self.settings.palette_intensity.to_bits(),
        ]);
        if *state.key.borrow() != key {
            state.cache.clear();
            *state.key.borrow_mut() = key;
        }
        vec![state.cache.draw(renderer, bounds.size(), |frame| {
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
        })]
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
