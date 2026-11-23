use bevy::prelude::*;

use crate::{game::GamePlugin, localization::Localization, screens::ScreensPlugin, state::GameState};

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
        .init_state::<GameState>()
        .insert_resource(Localization::load())
        .add_observer(Localization::save_selection)
        .add_plugins((crate::graphics::GraphicsPlugin, GamePlugin, ScreensPlugin))
        .run();
}
