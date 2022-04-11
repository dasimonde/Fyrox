use crate::{absm::segment::Segment, utils::fetch_node_center};
use fyrox::core::algebra::Vector2;
use fyrox::core::math::Rect;
use fyrox::gui::brush::Brush;
use fyrox::{
    core::pool::Handle,
    gui::{
        define_widget_deref,
        draw::{CommandTexture, Draw, DrawingContext},
        message::UiMessage,
        widget::{Widget, WidgetBuilder},
        BuildContext, Control, UiNode, UserInterface,
    },
};
use std::{
    any::{Any, TypeId},
    ops::{Deref, DerefMut},
    sync::mpsc::Sender,
};

#[derive(Debug, Clone)]
pub struct Connection {
    widget: Widget,
    segment: Segment,
}

define_widget_deref!(Connection);

pub fn draw_connection(
    drawing_context: &mut DrawingContext,
    source: Vector2<f32>,
    dest: Vector2<f32>,
    clip_bounds: Rect<f32>,
    brush: Brush,
) {
    drawing_context.push_line(source, dest, 2.0);
    drawing_context.commit(clip_bounds, brush, CommandTexture::None, None);
}

impl Control for Connection {
    fn query_component(&self, type_id: TypeId) -> Option<&dyn Any> {
        if type_id == TypeId::of::<Self>() {
            Some(self)
        } else {
            None
        }
    }

    fn draw(&self, drawing_context: &mut DrawingContext) {
        draw_connection(
            drawing_context,
            self.segment.source_pos,
            self.segment.dest_pos,
            self.clip_bounds(),
            self.foreground(),
        );
    }

    fn handle_routed_message(&mut self, ui: &mut UserInterface, message: &mut UiMessage) {
        self.widget.handle_routed_message(ui, message)
    }
}

pub struct ConnectionBuilder {
    widget_builder: WidgetBuilder,
    source: Handle<UiNode>,
    dest: Handle<UiNode>,
}

impl ConnectionBuilder {
    pub fn new(widget_builder: WidgetBuilder) -> Self {
        Self {
            widget_builder,
            source: Default::default(),
            dest: Default::default(),
        }
    }

    pub fn with_source(mut self, source: Handle<UiNode>) -> Self {
        self.source = source;
        self
    }

    pub fn with_dest(mut self, dest: Handle<UiNode>) -> Self {
        self.dest = dest;
        self
    }

    pub fn build(self, ctx: &mut BuildContext) -> Handle<UiNode> {
        let connection = Connection {
            widget: self.widget_builder.build(),
            segment: Segment {
                source: self.source,
                source_pos: fetch_node_center(self.source, ctx),
                dest: self.dest,
                dest_pos: fetch_node_center(self.dest, ctx),
            },
        };

        ctx.add_node(UiNode::new(connection))
    }
}
