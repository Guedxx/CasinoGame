//! Temporary browser review cameras. Remove after the user approves the city views.
use bevy::prelude::*;

use crate::state::GameState;

pub fn install(app: &mut App) {
    app.init_resource::<ReviewView>().add_systems(
        PostUpdate,
        review
            .after(super::player::PlayerCamera)
            .before(TransformSystems::Propagate)
            .run_if(in_state(GameState::Playing)),
    );
}

#[derive(Resource, Default)]
struct ReviewView(Option<usize>);

fn review(
    keys: Res<ButtonInput<KeyCode>>,
    mut view: ResMut<ReviewView>,
    mut cameras: Query<&mut Transform, With<super::camera::MainCamera>>,
    mut text: Query<&mut Visibility, With<Text>>,
) {
    let views = [
        (Vec3::new(1.45, 1.65, 3.8), Vec3::new(-0.03, 1.4, 2.7)),
        (Vec3::new(-1.1, 1.65, 6.5), Vec3::new(-1.1, 1.3, -4.8)),
        (Vec3::new(2.4, 1.65, -1.8), Vec3::new(8.2, 1.4, -2.2)),
        (Vec3::new(1.3, 1.65, 7.5), Vec3::new(1.7, 1.0, 14.4)),
        (Vec3::new(10.0, 8.0, -14.0), Vec3::new(-1.0, 0.0, 1.0)),
        (Vec3::new(2.9, 1.65, 1.3), Vec3::new(8.2, 0.4, 2.5)),
        (Vec3::new(5.6, -4.35, -5.0), Vec3::new(6.1, -3.0, 5.0)),
        (Vec3::new(1.6, 1.65, 1.3), Vec3::new(1.6, 1.65, 6.0)),
        (Vec3::new(-0.65, 1.65, -4.6), Vec3::new(-1.8, 1.1, -7.3)),
        (Vec3::new(6.9, 2.4, -4.8), Vec3::new(3.6, 1.5, -0.4)),
        (Vec3::new(3.1, 2.55, -3.9), Vec3::new(8.0, 2.6, -5.0)),
        (Vec3::new(3.12, 1.65, 1.0), Vec3::new(8.0, -4.0, -7.0)),
        (Vec3::new(3.12, 1.65, 2.65), Vec3::new(8.0, 4.0, 12.0)),
        (Vec3::new(0.45, 1.65, 8.9), Vec3::new(-10.0, 4.0, 14.0)),
        (Vec3::new(3.1, 1.65, 8.9), Vec3::new(15.0, 4.0, 14.0)),
        (Vec3::new(1.0, 1.65, -4.0), Vec3::new(5.0, 3.0, -4.0)),
        (Vec3::new(1.4, 1.6, 2.2), Vec3::new(2.88, 0.25, 1.7)),
        (Vec3::new(1.2, 1.6, 3.1), Vec3::new(2.9, 1.2, 4.7)),
        (Vec3::new(1.8, 1.65, 2.0), Vec3::new(1.0, 0.9, -0.6)),
    ];
    for (index, key) in [
        KeyCode::F1,
        KeyCode::F2,
        KeyCode::F3,
        KeyCode::F4,
        KeyCode::F5,
        KeyCode::F7,
        KeyCode::F8,
        KeyCode::F9,
        KeyCode::F10,
        KeyCode::F11,
        KeyCode::Digit1,
        KeyCode::Digit2,
        KeyCode::Digit3,
        KeyCode::Digit4,
        KeyCode::Digit5,
        KeyCode::Digit6,
        KeyCode::Home,
        KeyCode::Insert,
        KeyCode::Delete,
    ]
    .into_iter()
    .enumerate()
    {
        if keys.just_pressed(key) {
            view.0 = Some(index);
            info!("City review camera {}", index + 1);
        }
    }
    if keys.just_pressed(KeyCode::F6) {
        view.0 = None;
    }
    for mut visibility in &mut text {
        *visibility = if view.0.is_some() {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        };
    }
    if let Some(index) = view.0 {
        let (position, target) = views[index];
        for mut camera in &mut cameras {
            *camera = Transform::from_translation(position).looking_at(target, Vec3::Y);
        }
    }
}
