use bevy::prelude::*;

use crate::state::GameState;

pub struct MenuPlugin;

impl Plugin for MenuPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::Menu), setup)
            .add_systems(Update, start_game.run_if(in_state(GameState::Menu)));
    }
}

fn setup(mut commands: Commands) {
    super::spawn_screen(
        &mut commands,
        GameState::Menu,
        "Casino Game\nPress Enter to play",
    );
}

fn start_game(keys: Res<ButtonInput<KeyCode>>, mut next: ResMut<NextState<GameState>>) {
    if keys.just_pressed(KeyCode::Enter) {
        next.set(GameState::Playing);
    }
}
