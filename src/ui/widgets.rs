//! Общие кнопки, подложки и обработка клика вне панели.
use super::Message;
use iced::widget::{button, container, mouse_area, space, text};
use iced::advanced::{Clipboard, Layout, Shell, Widget, layout, overlay, renderer, widget};
use iced::{Element, Event, Fill, Length, Rectangle, Renderer, Size, Theme, Vector, mouse};

pub(super) fn action<'a>(
    label: &'a str,
    message: Message,
    enabled: bool,
) -> iced::widget::Button<'a, Message> {
    button(text(label).size(14))
        .padding([8, 12])
        .style(super::style::action_button)
        .on_press_maybe(enabled.then_some(message))
}

pub(super) fn backdrop(message: Message, dim: bool) -> Element<'static, Message> {
    iced::widget::opaque(
        mouse_area(
            container(space())
                .width(Fill)
                .height(Fill)
                .style(move |_| container::Style {
                    background: dim.then(|| iced::Color::BLACK.scale_alpha(0.25).into()),
                    ..Default::default()
                }),
        )
        .on_press(message.clone())
        .on_right_press(message),
    )
}

/// Закрывает встроенную панель по нажатию вне неё, сохраняя обычную обработку клика.
pub(super) fn dismiss_on_outside_press<'a>(
    content: Element<'a, Message>,
    close: Message,
) -> Element<'a, Message> {
    Element::new(OutsidePress { content, close })
}

struct OutsidePress<'a> {
    content: Element<'a, Message>,
    close: Message,
}


impl Widget<Message, Theme, Renderer> for OutsidePress<'_> {
    fn tag(&self) -> widget::tree::Tag {
        self.content.as_widget().tag()
    }

    fn state(&self) -> widget::tree::State {
        self.content.as_widget().state()
    }

    fn children(&self) -> Vec<widget::Tree> {
        self.content.as_widget().children()
    }

    fn diff(&self, tree: &mut widget::Tree) {
        self.content.as_widget().diff(tree);
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
        self.content.as_widget_mut().layout(tree, renderer, limits)
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
        self.content.as_widget_mut().update(
            tree, event, layout, cursor, renderer, clipboard, shell, viewport,
        );
        // Проверяем и захваченные соседним виджетом события: кнопки вне панели
        // тоже должны закрывать её. Сам клик не перехватываем.
        if matches!(event, Event::Mouse(mouse::Event::ButtonPressed(_)))
            && cursor.position().is_some()
            && !cursor.is_over(layout.bounds())
        {
            shell.publish(self.close.clone());
        }
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
        self.content
            .as_widget()
            .draw(tree, renderer, theme, style, layout, cursor, viewport);
    }

    fn operate(
        &mut self,
        tree: &mut widget::Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn widget::Operation,
    ) {
        self.content
            .as_widget_mut()
            .operate(tree, layout, renderer, operation);
    }

    fn mouse_interaction(
        &self,
        tree: &widget::Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        self.content
            .as_widget()
            .mouse_interaction(tree, layout, cursor, viewport, renderer)
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut widget::Tree,
        layout: Layout<'b>,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, Message, Theme, Renderer>> {
        self.content
            .as_widget_mut()
            .overlay(tree, layout, renderer, viewport, translation)
    }
}
