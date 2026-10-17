mod camera;
pub(crate) mod player;
#[cfg(feature = "scene-review")]
mod review;

use bevy::prelude::*;

/// Register shared gameplay plugins here as the game grows.
pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((camera::CameraPlugin, player::PlayerPlugin));
        #[cfg(feature = "scene-review")]
        review::install(app);
    }
}
