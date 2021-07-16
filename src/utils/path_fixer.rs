//! Special utility that allows you to fix paths to resources. It is very useful if you've
//! moved a resource in a file system, but a scene has old path.

use crate::{
    gui::{BuildContext, Ui, UiMessage, UiNode},
    make_scene_file_filter,
};
use rg3d::gui::message::WidgetMessage;
use rg3d::{
    core::{
        color::Color,
        futures::executor::block_on,
        pool::Handle,
        visitor::{Visit, Visitor},
    },
    gui::{
        border::BorderBuilder,
        brush::Brush,
        button::ButtonBuilder,
        decorator::DecoratorBuilder,
        file_browser::FileSelectorBuilder,
        formatted_text::WrapMode,
        grid::{Column, GridBuilder, Row},
        list_view::ListViewBuilder,
        message::{
            ButtonMessage, FileSelectorMessage, ListViewMessage, MessageDirection, TextMessage,
            UiMessageData, WindowMessage,
        },
        stack_panel::StackPanelBuilder,
        text::TextBuilder,
        widget::WidgetBuilder,
        window::{WindowBuilder, WindowTitle},
        HorizontalAlignment, Orientation, Thickness, VerticalAlignment,
    },
    resource::{model::Model, texture::Texture},
    scene::{light::Light, node::Node, Scene},
};
use std::{
    collections::HashSet,
    hash::{Hash, Hasher},
    path::PathBuf,
};

pub struct PathFixer {
    pub window: Handle<UiNode>,
    scene_path: Handle<UiNode>,
    scene_selector: Handle<UiNode>,
    load_scene: Handle<UiNode>,
    scene: Option<Scene>,
    scene_resources: Vec<SceneResource>,
    resources_list: Handle<UiNode>,
    cancel: Handle<UiNode>,
    ok: Handle<UiNode>,
    selection: Option<usize>,
    fix: Handle<UiNode>,
    resource_path: Handle<UiNode>,
}

enum SceneResource {
    Model(Model),
    Texture(Texture),
    // TODO: Add sound buffers.
}

impl SceneResource {
    fn path(&self) -> PathBuf {
        match self {
            SceneResource::Model(model) => model.state().path().to_path_buf(),
            SceneResource::Texture(texture) => texture.state().path().to_path_buf(),
        }
    }

    fn key(&self) -> usize {
        match self {
            SceneResource::Model(model) => model.key(),
            SceneResource::Texture(texture) => texture.key(),
        }
    }
}

impl Hash for SceneResource {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_usize(self.key());
    }
}

impl PartialEq for SceneResource {
    fn eq(&self, other: &Self) -> bool {
        self.key() == other.key()
    }
}

impl Eq for SceneResource {}

impl PathFixer {
    pub fn new(ctx: &mut BuildContext) -> Self {
        let scene_selector = FileSelectorBuilder::new(
            WindowBuilder::new(WidgetBuilder::new().with_width(300.0).with_height(400.0))
                .open(false)
                .with_title(WindowTitle::Text("Select a scene for diagnostics".into())),
        )
        .with_filter(make_scene_file_filter())
        .build(ctx);

        let load_scene;
        let scene_path;
        let resources_list;
        let cancel;
        let ok;
        let fix;
        let resource_path;
        let window = WindowBuilder::new(WidgetBuilder::new().with_width(400.0).with_height(500.0))
            .with_title(WindowTitle::text("Path Fixer"))
            .open(false)
            .with_content(
                GridBuilder::new(
                    WidgetBuilder::new()
                        .with_child({
                            scene_path = TextBuilder::new(WidgetBuilder::new().on_row(0))
                                .with_text("Scene: No scene loaded!")
                                .with_wrap(WrapMode::Word)
                                .build(ctx);
                            scene_path
                        })
                        .with_child(
                            GridBuilder::new(
                                WidgetBuilder::new()
                                    .on_row(1)
                                    .with_child({
                                        resource_path =
                                            TextBuilder::new(WidgetBuilder::new().on_column(0))
                                                .with_vertical_text_alignment(
                                                    VerticalAlignment::Center,
                                                )
                                                .build(ctx);
                                        resource_path
                                    })
                                    .with_child({
                                        fix = ButtonBuilder::new(
                                            WidgetBuilder::new()
                                                .with_width(40.0)
                                                .on_column(1)
                                                .with_enabled(false)
                                                .with_margin(Thickness::uniform(1.0)),
                                        )
                                        .with_text("Fix...")
                                        .build(ctx);
                                        fix
                                    }),
                            )
                            .add_column(Column::stretch())
                            .add_column(Column::auto())
                            .add_row(Row::stretch())
                            .build(ctx),
                        )
                        .with_child({
                            resources_list =
                                ListViewBuilder::new(WidgetBuilder::new().on_row(2)).build(ctx);
                            resources_list
                        })
                        .with_child(
                            StackPanelBuilder::new(
                                WidgetBuilder::new()
                                    .with_horizontal_alignment(HorizontalAlignment::Right)
                                    .on_row(3)
                                    .with_child({
                                        load_scene = ButtonBuilder::new(
                                            WidgetBuilder::new()
                                                .with_width(100.0)
                                                .with_margin(Thickness::uniform(1.0)),
                                        )
                                        .with_text("Load Scene...")
                                        .build(ctx);
                                        load_scene
                                    })
                                    .with_child({
                                        ok = ButtonBuilder::new(
                                            WidgetBuilder::new()
                                                .with_width(100.0)
                                                .with_margin(Thickness::uniform(1.0)),
                                        )
                                        .with_text("OK")
                                        .build(ctx);
                                        ok
                                    })
                                    .with_child({
                                        cancel = ButtonBuilder::new(
                                            WidgetBuilder::new()
                                                .with_width(100.0)
                                                .with_margin(Thickness::uniform(1.0)),
                                        )
                                        .with_text("Cancel")
                                        .build(ctx);
                                        cancel
                                    }),
                            )
                            .with_orientation(Orientation::Horizontal)
                            .build(ctx),
                        ),
                )
                .add_row(Row::auto())
                .add_row(Row::strict(28.0))
                .add_row(Row::stretch())
                .add_row(Row::strict(28.0))
                .add_column(Column::stretch())
                .build(ctx),
            )
            .build(ctx);

        Self {
            window,
            scene_selector,
            load_scene,
            scene_path,
            scene: None,
            scene_resources: Default::default(),
            resources_list,
            ok,
            cancel,
            resource_path,
            fix,
            selection: None,
        }
    }

    pub fn handle_ui_message(&mut self, message: &UiMessage, ui: &mut Ui) {
        match message.data() {
            UiMessageData::FileSelector(FileSelectorMessage::Commit(path)) => {
                if message.destination() == self.scene_selector {
                    let mut scene = Scene::default();
                    let message;
                    match block_on(Visitor::load_binary(path)) {
                        Ok(mut visitor) => {
                            if let Err(e) = scene.visit("Scene", &mut visitor) {
                                message = format!(
                                    "Failed to load a scene {}\nReason: {}",
                                    path.display(),
                                    e
                                );
                            } else {
                                // Gather resources.

                                // Use hash map to remove duplicates.
                                let mut scene_resources = HashSet::new();

                                for node in scene.graph.linear_iter() {
                                    if let Some(model) = node.resource() {
                                        scene_resources.insert(SceneResource::Model(model));
                                    }

                                    match node {
                                        Node::Light(light) => {
                                            if let Light::Spot(spot) = light {
                                                if let Some(texture) = spot.cookie_texture() {
                                                    scene_resources.insert(SceneResource::Texture(
                                                        texture.clone(),
                                                    ));
                                                }
                                            }
                                        }
                                        Node::Camera(camera) => {
                                            if let Some(skybox) = camera.skybox_ref() {
                                                for texture in skybox.textures().iter().flatten() {
                                                    scene_resources.insert(SceneResource::Texture(
                                                        texture.clone(),
                                                    ));
                                                }
                                            }
                                        }
                                        Node::Mesh(mesh) => {
                                            for surface in mesh.surfaces() {
                                                for texture in [
                                                    surface.diffuse_texture(),
                                                    surface.normal_texture(),
                                                    surface.roughness_texture(),
                                                    surface.height_texture(),
                                                    surface.specular_texture(),
                                                ]
                                                .iter()
                                                .flatten()
                                                {
                                                    scene_resources.insert(SceneResource::Texture(
                                                        texture.clone(),
                                                    ));
                                                }
                                            }
                                        }
                                        Node::Sprite(sprite) => {
                                            if let Some(texture) = sprite.texture() {
                                                scene_resources
                                                    .insert(SceneResource::Texture(texture));
                                            }
                                        }
                                        Node::ParticleSystem(particle_system) => {
                                            if let Some(texture) = particle_system.texture() {
                                                scene_resources
                                                    .insert(SceneResource::Texture(texture));
                                            }
                                        }
                                        Node::Terrain(terrain) => {
                                            if let Some(first) = terrain.chunks_ref().first() {
                                                for layer in first.layers() {
                                                    for texture in [
                                                        layer.diffuse_texture.clone(),
                                                        layer.specular_texture.clone(),
                                                        layer.roughness_texture.clone(),
                                                        layer.height_texture.clone(),
                                                        layer.normal_texture.clone(),
                                                    ]
                                                    .iter()
                                                    .flatten()
                                                    {
                                                        scene_resources.insert(
                                                            SceneResource::Texture(texture.clone()),
                                                        );
                                                    }
                                                }
                                            }
                                        }
                                        Node::Base(_) => {
                                            // Nothing
                                        }
                                    }
                                }

                                // Turn hash map into vec to be able to index it.
                                self.scene_resources =
                                    scene_resources.into_iter().collect::<Vec<_>>();

                                let ctx = &mut ui.build_ctx();
                                let items = self
                                    .scene_resources
                                    .iter()
                                    .filter_map(|r| {
                                        let path = r.path();
                                        if !path.exists() {
                                            Some(
                                                DecoratorBuilder::new(BorderBuilder::new(
                                                    WidgetBuilder::new()
                                                        .with_height(22.0)
                                                        .with_child(
                                                            TextBuilder::new(
                                                                WidgetBuilder::new()
                                                                    .with_margin(
                                                                        Thickness::uniform(1.0),
                                                                    )
                                                                    .with_foreground(Brush::Solid(
                                                                        Color::RED,
                                                                    )),
                                                            )
                                                            .with_vertical_text_alignment(
                                                                VerticalAlignment::Center,
                                                            )
                                                            .with_text(
                                                                path.to_string_lossy().to_string(),
                                                            )
                                                            .build(ctx),
                                                        ),
                                                ))
                                                .build(ctx),
                                            )
                                        } else {
                                            None
                                        }
                                    })
                                    .collect::<Vec<_>>();

                                ui.send_message(ListViewMessage::items(
                                    self.resources_list,
                                    MessageDirection::ToWidget,
                                    items,
                                ));
                                ui.send_message(ListViewMessage::selection(
                                    self.resources_list,
                                    MessageDirection::ToWidget,
                                    None,
                                ));

                                self.scene = Some(scene);

                                message = format!("Scene: {}", path.display());
                            }
                        }
                        Err(e) => {
                            message =
                                format!("Failed to load a scene {}\nReason: {}", path.display(), e);
                        }
                    }

                    ui.send_message(TextMessage::text(
                        self.scene_path,
                        MessageDirection::ToWidget,
                        message,
                    ));
                }
            }
            UiMessageData::Button(ButtonMessage::Click) => {
                if message.destination() == self.load_scene {
                    ui.send_message(WindowMessage::open_modal(
                        self.scene_selector,
                        MessageDirection::ToWidget,
                        true,
                    ));
                } else if message.destination() == self.cancel {
                    ui.send_message(WindowMessage::close(
                        self.window,
                        MessageDirection::ToWidget,
                    ));
                } else if message.destination() == self.ok {
                    ui.send_message(WindowMessage::close(
                        self.window,
                        MessageDirection::ToWidget,
                    ));

                    // TODO: Apply changes.
                }
            }
            UiMessageData::ListView(ListViewMessage::SelectionChanged(selection)) => {
                if message.destination() == self.resources_list {
                    self.selection = selection.clone();

                    if let Some(selection) = selection {
                        ui.send_message(TextMessage::text(
                            self.resource_path,
                            MessageDirection::ToWidget,
                            format!(
                                "Resource: {}",
                                self.scene_resources[*selection].path().display()
                            ),
                        ))
                    } else {
                        ui.send_message(TextMessage::text(
                            self.resource_path,
                            MessageDirection::ToWidget,
                            "No resource selected".to_owned(),
                        ));
                    }

                    ui.send_message(WidgetMessage::enabled(
                        self.fix,
                        MessageDirection::ToWidget,
                        selection.is_some(),
                    ));
                }
            }
            _ => {}
        }
    }
}
