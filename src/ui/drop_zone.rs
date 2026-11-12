//! Геометрия целей DND и порог начала жеста на карточке.
use super::Message;
use iced::advanced::{Clipboard, Layout, Shell, Widget, layout, overlay, renderer, widget};
use iced::{Element, Event, Length, Rectangle, Renderer, Size, Theme, Vector, mouse};
use std::path::PathBuf;

pub(super) fn zone<'a>(
    content: Element<'a, Message>,
    source: Option<PathBuf>,
    target: Option<PathBuf>,
) -> Element<'a, Message> {
    Element::new(Zone {
        content,
        source,
        target,
    })
}
struct Zone<'a> {
    content: Element<'a, Message>,
    source: Option<PathBuf>,
    target: Option<PathBuf>,
}
#[derive(Default)]
struct Gesture {
    press: Option<iced::Point>,
    dragged: bool,
    source: Option<PathBuf>,
}
struct Target(PathBuf);

#[derive(Default)]
pub(super) struct Collect {
    zones: Vec<(PathBuf, Rectangle)>,
    offset: Vector,
    clip: Option<Rectangle>,
    pending_scroll: Option<(Rectangle, Vector)>,
}
impl widget::Operation<Vec<(PathBuf, Rectangle)>> for Collect {
    fn traverse(
        &mut self,
        operate: &mut dyn FnMut(&mut dyn widget::Operation<Vec<(PathBuf, Rectangle)>>),
    ) {
        let previous = (self.offset, self.clip);
        if let Some((bounds, translation)) = self.pending_scroll.take() {
            let bounds = bounds - self.offset;
            self.clip = self
                .clip
                .map_or(Some(bounds), |clip| clip.intersection(&bounds));
            self.offset += translation;
        }
        operate(self);
        (self.offset, self.clip) = previous;
    }
    fn scrollable(
        &mut self,
        _: Option<&widget::Id>,
        bounds: Rectangle,
        _: Rectangle,
        translation: Vector,
        _: &mut dyn widget::operation::Scrollable,
    ) {
        self.pending_scroll = Some((bounds, translation));
    }
    fn custom(&mut self, _: Option<&widget::Id>, bounds: Rectangle, state: &mut dyn std::any::Any) {
        if let Some(Target(path)) = state.downcast_ref::<Target>() {
            let bounds = bounds - self.offset;
            if let Some(bounds) = self
                .clip
                .map_or(Some(bounds), |clip| clip.intersection(&bounds))
            {
                self.zones.push((path.clone(), bounds));
            }
        }
    }
    fn finish(&self) -> widget::operation::Outcome<Vec<(PathBuf, Rectangle)>> {
        widget::operation::Outcome::Some(self.zones.clone())
    }
}

impl Widget<Message, Theme, Renderer> for Zone<'_> {
    fn tag(&self) -> widget::tree::Tag {
        widget::tree::Tag::of::<Gesture>()
    }

    fn state(&self) -> widget::tree::State {
        widget::tree::State::new(Gesture::default())
    }

    fn children(&self) -> Vec<widget::Tree> {
        vec![widget::Tree::new(&self.content)]
    }

    fn diff(&self, tree: &mut widget::Tree) {
        let gesture = tree.state.downcast_mut::<Gesture>();
        if gesture.source.as_ref() != self.source.as_ref() {
            *gesture = Gesture::default();
        }
        tree.diff_children(std::slice::from_ref(&self.content));
    }

    fn size(&self) -> Size<Length> {
        self.content.as_widget().size()
    }

    fn size_hint(&self) -> Size<Length> {
        self.content.as_widget().size_hint()
    }

    fn layout(
        &mut self,
        tree: &mut widget::Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        self.content
            .as_widget_mut()
            .layout(&mut tree.children[0], renderer, limits)
    }

    fn update(
        &mut self,
        tree: &mut widget::Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        let gesture = tree.state.downcast_mut::<Gesture>();
        if let Some(source) = &self.source {
            match event {
                Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
                    if cursor.is_over(layout.bounds()) && cursor.is_over(*viewport) =>
                {
                    gesture.press = cursor.position();
                    gesture.source = Some(source.clone());
                    gesture.dragged = false;
                    shell.capture_event();
                }
                Event::Mouse(mouse::Event::CursorMoved { position }) => {
                    if let Some(press) = gesture.press
                        && !gesture.dragged
                        && press.distance(*position) >= 6.0
                    {
                        gesture.dragged = true;
                        shell.publish(Message::DragStart(source.clone()));
                        shell.capture_event();
                    }
                }
                Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                    if gesture.press.take().is_some() {
                        if !gesture.dragged
                            && cursor.is_over(layout.bounds())
                            && cursor.is_over(*viewport)
                        {
                            shell.publish(Message::Select(source.clone()));
                        }
                        gesture.dragged = false;
                        shell.capture_event();
                    }
                }
                Event::Mouse(mouse::Event::CursorLeft)
                | Event::Window(iced::window::Event::Unfocused)
                | Event::Keyboard(iced::keyboard::Event::KeyPressed {
                    key: iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape),
                    ..
                }) => {
                    gesture.press = None;
                    gesture.dragged = false;
                }
                _ => {}
            }
        }
        self.content.as_widget_mut().update(
            &mut tree.children[0],
            event,
            layout,
            cursor,
            renderer,
            clipboard,
            shell,
            viewport,
        );
    }

    fn draw(
        &self,
        tree: &widget::Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        self.content.as_widget().draw(
            &tree.children[0],
            renderer,
            theme,
            style,
            layout,
            cursor,
            viewport,
        );
    }

    fn operate(
        &mut self,
        tree: &mut widget::Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn widget::Operation,
    ) {
        if let Some(path) = &self.target {
            operation.custom(None, layout.bounds(), &mut Target(path.clone()));
        }
        self.content
            .as_widget_mut()
            .operate(&mut tree.children[0], layout, renderer, operation);
    }

    fn mouse_interaction(
        &self,
        tree: &widget::Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        if self.source.is_some() && cursor.is_over(layout.bounds()) && cursor.is_over(*viewport) {
            mouse::Interaction::Pointer
        } else {
            self.content.as_widget().mouse_interaction(
                &tree.children[0],
                layout,
                cursor,
                viewport,
                renderer,
            )
        }
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut widget::Tree,
        layout: Layout<'b>,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, Message, Theme, Renderer>> {
        self.content.as_widget_mut().overlay(
            &mut tree.children[0],
            layout,
            renderer,
            viewport,
            translation,
        )
    }
}
