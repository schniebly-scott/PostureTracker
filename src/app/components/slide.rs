use iced::advanced::layout::{Layout, Limits, Node};
use iced::advanced::renderer::{self, Renderer as _};
use iced::advanced::widget::{Tree, Widget};
use iced::advanced::{Clipboard, Shell};
use iced::mouse::{self, Cursor};
use iced::{Element, Event, Length, Rectangle, Renderer, Size, Theme, Vector};

use crate::app::SlideDirection;

pub struct Slide<'a, Message> {
    current: Element<'a, Message, Theme, Renderer>,
    previous: Option<Element<'a, Message, Theme, Renderer>>,
    progress: f32,
    direction: SlideDirection,
}

pub fn slide<'a, Message>(
    current: impl Into<Element<'a, Message, Theme, Renderer>>,
    previous: Option<Element<'a, Message, Theme, Renderer>>,
    progress: f32,
    direction: SlideDirection,
) -> Slide<'a, Message> {
    Slide {
        current: current.into(),
        previous,
        progress: progress.clamp(0.0, 1.0),
        direction,
    }
}

fn ease_out_cubic(t: f32) -> f32 {
    let u = 1.0 - t;
    1.0 - u * u * u
}

impl<'a, Message> Widget<Message, Theme, Renderer> for Slide<'a, Message> {
    fn size(&self) -> Size<Length> {
        self.current.as_widget().size()
    }

    fn children(&self) -> Vec<Tree> {
        let mut children = vec![Tree::new(&self.current)];
        if let Some(prev) = &self.previous {
            children.push(Tree::new(prev));
        }
        children
    }

    fn diff(&self, tree: &mut Tree) {
        let mut items: Vec<&Element<'_, Message, Theme, Renderer>> = vec![&self.current];
        if let Some(prev) = &self.previous {
            items.push(prev);
        }
        tree.diff_children(&items);
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &Renderer, limits: &Limits) -> Node {
        let current_node =
            self.current
                .as_widget_mut()
                .layout(&mut tree.children[0], renderer, limits);
        let size = current_node.size();

        let mut nodes = vec![current_node];
        if let Some(prev) = self.previous.as_mut() {
            let prev_node =
                prev.as_widget_mut()
                    .layout(&mut tree.children[1], renderer, limits);
            nodes.push(prev_node);
        }

        Node::with_children(size, nodes)
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
        let bounds = layout.bounds();
        let width = bounds.width;
        let eased = ease_out_cubic(self.progress);

        let mut children = layout.children();
        let Some(current_layout) = children.next() else {
            return;
        };
        let previous_layout = children.next();

        // Right cycle: previous slides off to the left, current slides in from the right.
        // Left cycle: previous slides off to the right, current slides in from the left.
        let (current_offset, previous_offset) = match self.direction {
            SlideDirection::Right => (width * (1.0 - eased), -width * eased),
            SlideDirection::Left => (-width * (1.0 - eased), width * eased),
        };

        // Hard-clip to our bounds so translated children don't paint over siblings.
        let clip = bounds.intersection(viewport).unwrap_or(bounds);
        renderer.with_layer(clip, |renderer| {
            if let (Some(prev_layout), Some(prev_element)) =
                (previous_layout, self.previous.as_ref())
            {
                renderer.with_translation(Vector::new(previous_offset, 0.0), |renderer| {
                    prev_element.as_widget().draw(
                        &tree.children[1],
                        renderer,
                        theme,
                        style,
                        prev_layout,
                        cursor,
                        viewport,
                    );
                });
            }

            renderer.with_translation(Vector::new(current_offset, 0.0), |renderer| {
                self.current.as_widget().draw(
                    &tree.children[0],
                    renderer,
                    theme,
                    style,
                    current_layout,
                    cursor,
                    viewport,
                );
            });
        });
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
        // While transitioning, the current view hasn't settled — ignore events to
        // avoid clicks landing on offset widgets.
        if self.previous.is_some() {
            return;
        }
        let Some(current_layout) = layout.children().next() else {
            return;
        };
        self.current.as_widget_mut().update(
            &mut tree.children[0],
            event,
            current_layout,
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
        if self.previous.is_some() {
            return mouse::Interaction::None;
        }
        let Some(current_layout) = layout.children().next() else {
            return mouse::Interaction::None;
        };
        self.current.as_widget().mouse_interaction(
            &tree.children[0],
            current_layout,
            cursor,
            viewport,
            renderer,
        )
    }
}

impl<'a, Message: 'a> From<Slide<'a, Message>> for Element<'a, Message, Theme, Renderer> {
    fn from(slide: Slide<'a, Message>) -> Self {
        Element::new(slide)
    }
}

#[cfg(test)]
mod tests {
    use super::ease_out_cubic;

    #[test]
    fn ease_out_cubic_endpoints() {
        assert!((ease_out_cubic(0.0) - 0.0).abs() < 1e-6);
        assert!((ease_out_cubic(1.0) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn ease_out_cubic_is_monotonic_increasing() {
        let mut prev = ease_out_cubic(0.0);
        for i in 1..=10 {
            let t = i as f32 / 10.0;
            let v = ease_out_cubic(t);
            assert!(v >= prev, "easing should not decrease: {prev} -> {v}");
            prev = v;
        }
    }
}
