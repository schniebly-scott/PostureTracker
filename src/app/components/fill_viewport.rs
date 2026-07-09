use iced::advanced::layout::{Layout, Limits, Node};
use iced::advanced::renderer;
use iced::advanced::widget::{Tree, Widget};
use iced::advanced::{Clipboard, Shell};
use iced::mouse::{self, Cursor};
use iced::{Element, Event, Length, Rectangle, Renderer, Size, Theme};

/// Restores `Length::Fill` height semantics for content inside a vertical
/// `scrollable`.
///
/// iced 0.14 lays out scrollable content with unbounded, *compressed* height
/// limits, so any `Fill` height inside (the cards' accent stripes, the primary
/// row that absorbs leftover space) collapses to its intrinsic size and the
/// panel packs to the top. This wrapper measures the content's natural height,
/// recovers the viewport height from `limits.min()` (the scrollable forwards
/// the min it was given, and the panel column allocates the body a fixed
/// share), and re-lays the content out with plain bounded limits of
/// `max(natural, viewport)`: identical to a non-scrolling `Fill` container
/// when the content fits, and a fully-styled, taller-than-viewport layout
/// that scrolls when it doesn't.
pub struct FillViewport<'a, Message> {
    content: Element<'a, Message, Theme, Renderer>,
}

pub fn fill_viewport<'a, Message>(
    content: impl Into<Element<'a, Message, Theme, Renderer>>,
) -> FillViewport<'a, Message> {
    FillViewport {
        content: content.into(),
    }
}

impl<'a, Message> Widget<Message, Theme, Renderer> for FillViewport<'a, Message> {
    fn size(&self) -> Size<Length> {
        self.content.as_widget().size()
    }

    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.content)]
    }

    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(std::slice::from_ref(&self.content));
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &Renderer, limits: &Limits) -> Node {
        let viewport_height = limits.min().height;

        let natural = self
            .content
            .as_widget_mut()
            .layout(&mut tree.children[0], renderer, limits)
            .size()
            .height;

        let bounded = Limits::new(
            Size::ZERO,
            Size::new(limits.max().width, natural.max(viewport_height)),
        );
        let content = self
            .content
            .as_widget_mut()
            .layout(&mut tree.children[0], renderer, &bounded);

        Node::with_children(content.size(), vec![content])
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: Cursor,
        viewport: &Rectangle,
    ) {
        let Some(content_layout) = layout.children().next() else {
            return;
        };
        self.content.as_widget().draw(
            &tree.children[0],
            renderer,
            theme,
            style,
            content_layout,
            cursor,
            viewport,
        );
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        let Some(content_layout) = layout.children().next() else {
            return;
        };
        self.content.as_widget_mut().update(
            &mut tree.children[0],
            event,
            content_layout,
            cursor,
            renderer,
            clipboard,
            shell,
            viewport,
        );
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        let Some(content_layout) = layout.children().next() else {
            return mouse::Interaction::None;
        };
        self.content.as_widget().mouse_interaction(
            &tree.children[0],
            content_layout,
            cursor,
            viewport,
            renderer,
        )
    }
}

impl<'a, Message: 'a> From<FillViewport<'a, Message>> for Element<'a, Message, Theme, Renderer> {
    fn from(widget: FillViewport<'a, Message>) -> Self {
        Element::new(widget)
    }
}
