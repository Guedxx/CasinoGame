mod menu;
mod playing;

use bevy::prelude::*;

pub struct ScreensPlugin;

impl Plugin for ScreensPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((menu::MenuPlugin, playing::PlayingPlugin));
    }
}

fn spawn_screen(commands: &mut Commands, state: crate::state::GameState, message: &str) {
    commands.spawn((
        DespawnOnExit(state),
        Node {
            width: percent(100),
            height: percent(100),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        },
        children![(Text::new(message), TextFont::from_font_size(32.0))],
    ));
}
