mod menu;
mod playing;
mod title_render;

use bevy::prelude::*;

pub struct ScreensPlugin;

impl Plugin for ScreensPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((menu::MenuPlugin, playing::PlayingPlugin));
    }
}
