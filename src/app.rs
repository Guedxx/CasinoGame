use bevy::prelude::*;

use crate::{game::GamePlugin, screens::ScreensPlugin, state::GameState};

pub fn run() {
    App::new()
        .add_plugins(
            DefaultPlugins
                .set(ImagePlugin::default_nearest())
                .set(AssetPlugin {
                    meta_check: bevy::asset::AssetMetaCheck::Never,
                    ..default()
                })
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "Saldo Zero".into(),
                        resolution: (1280, 720).into(),
                        fit_canvas_to_parent: true,
                        ..default()
                    }),
                    ..default()
                }),
        )
        .insert_resource(ClearColor(Color::srgb(0.36, 0.40, 0.44)))
        .insert_resource(GlobalAmbientLight {
            brightness: 180.0,
            color: Color::srgb(0.78, 0.84, 0.94),
            ..default()
        })
        .init_state::<GameState>()
        .init_resource::<crate::localization::Localization>()
        .add_plugins((crate::graphics::GraphicsPlugin, GamePlugin, ScreensPlugin))
        .run();
}
