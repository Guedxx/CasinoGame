mod camera;
#[cfg(any(target_arch = "wasm32", test))]
mod cluster_settings;
mod lighting_fast_path;
pub(crate) mod player;
#[cfg(feature = "scene-review")]
mod review;

use bevy::prelude::*;

/// Register shared gameplay plugins here as the game grows.
pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((camera::CameraPlugin, player::PlayerPlugin));
        app.add_systems(Update, lighting_fast_path::install);
        #[cfg(target_arch = "wasm32")]
        app.add_systems(Startup, cluster_settings::use_cpu_clustering);
        #[cfg(feature = "scene-review")]
        review::install(app);
    }
}
