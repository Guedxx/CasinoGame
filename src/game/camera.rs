use bevy::prelude::*;

pub struct CameraPlugin;

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_camera);
    }
}

fn spawn_camera(mut commands: Commands, clear_color: Res<ClearColor>) {
    commands.spawn((
        Camera3d::default(),
        DistanceFog {
            color: clear_color.0,
            falloff: FogFalloff::ExponentialSquared { density: 0.045 },
            ..default()
        },
        Projection::Perspective(PerspectiveProjection {
            fov: 75.0_f32.to_radians(),
            near: 0.05,
            far: 2000.0,
            ..default()
        }),
        Transform::from_xyz(100.0, 80.0, 130.0).looking_at(Vec3::new(0.0, 3.0, 0.0), Vec3::Y),
    ));
}
