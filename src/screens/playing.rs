use bevy::{light::CascadeShadowConfigBuilder, prelude::*};

use crate::state::GameState;

pub struct PlayingPlugin;

impl Plugin for PlayingPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup)
            .add_systems(Update, return_to_menu.run_if(in_state(GameState::Playing)));
    }
}

fn setup(
    mut commands: Commands,
    assets: Res<AssetServer>,
    apartment: Query<(), With<crate::game::player::Apartment>>,
) {
    if !apartment.is_empty() {
        return;
    }
    commands.spawn((
        Name::new("Apartment and window scenery"),
        crate::game::player::Apartment,
        WorldAssetRoot(
            assets.load(GltfAssetLabel::Scene(0).from_asset("models/apartment_windows.glb")),
        ),
    ));
    commands.spawn((
        DirectionalLight {
            illuminance: 3_000.0,
            color: Color::srgb(0.78, 0.84, 0.94),
            shadow_maps_enabled: true,
            shadow_normal_bias: 0.4,
            ..default()
        },
        CascadeShadowConfigBuilder {
            num_cascades: 1,
            maximum_distance: 40.0,
            ..default()
        }
        .build(),
        Transform::from_xyz(-3.0, 8.0, -2.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    for (index, position) in [
        Vec3::new(1.761, 2.48, -0.491),
        Vec3::new(1.761, 2.48, 4.773),
        Vec3::new(1.761, 2.48, 7.848),
        // Preserve the warm light at the closed door seam without loading the corridor.
        Vec3::new(-1.25, 2.48, -2.5),
        Vec3::new(-1.25, 2.48, 3.5),
        Vec3::new(-1.25, 2.48, 7.5),
    ]
    .into_iter()
    .enumerate()
    {
        let mut light = commands.spawn((
            SpotLight {
                color: Color::srgb(1.0, 0.84, 0.62),
                intensity: 40_000.0,
                range: 7.0,
                inner_angle: 0.8,
                outer_angle: 1.3,
                shadow_maps_enabled: true,
                ..default()
            },
            Transform::from_translation(position).looking_to(Vec3::NEG_Y, Vec3::Z),
        ));
        if index < 3 {
            light.insert(crate::game::household::InteriorLight);
        }
    }
}

fn return_to_menu(
    keys: Res<ButtonInput<KeyCode>>,
    overlay: Res<crate::game::routine::GameplayOverlay>,
    mut next: ResMut<NextState<GameState>>,
) {
    if keys.just_pressed(KeyCode::Escape) && !overlay.escape_consumed && !overlay.blocks_world() {
        next.set(GameState::Menu);
    }
}
