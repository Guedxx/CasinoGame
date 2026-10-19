pub(crate) mod betting;
pub(crate) mod camera;
pub(crate) mod campaign;
pub(crate) mod campaign_save;
pub(crate) mod cursor;
#[cfg(any(target_arch = "wasm32", test))]
mod cluster_settings;
pub(crate) mod household;
mod lighting_fast_path;
mod obligations;
pub(crate) mod phone;
pub(crate) mod player;
#[cfg(feature = "scene-review")]
mod review;
pub(crate) mod routine;
mod support;

use bevy::prelude::*;

/// Register shared gameplay plugins here as the game grows.
pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            camera::CameraPlugin,
            player::PlayerPlugin,
            cursor::GameCursorPlugin,
            campaign_save::CampaignPlugin,
            routine::RoutinePlugin,
            phone::PhonePlugin,
            household::HouseholdPlugin,
        ));
        app.add_systems(Update, lighting_fast_path::install);
        #[cfg(target_arch = "wasm32")]
        app.add_systems(Startup, cluster_settings::use_cpu_clustering);
        #[cfg(feature = "scene-review")]
        review::install(app);
    }
}
