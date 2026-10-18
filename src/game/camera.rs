use bevy::{
    core_pipeline::prepass::DepthPrepass,
    light::cluster::{ClusterConfig, ClusterFarZMode, ClusterZConfig},
    prelude::*,
};

pub struct CameraPlugin;

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_camera);
    }
}

fn spawn_camera(mut commands: Commands, clear_color: Res<ClearColor>) {
    let far = 2000.0;
    commands.spawn((
        Camera3d::default(),
        DepthPrepass,
        // Separate nearby indoor lights while keeping distant scenery out of their last cluster.
        ClusterConfig::FixedZ {
            total: 4096,
            z_slices: 24,
            z_config: ClusterZConfig {
                first_slice_depth: 1.0,
                far_z_mode: ClusterFarZMode::Constant(far),
            },
            dynamic_resizing: true,
        },
        DistanceFog {
            color: clear_color.0,
            falloff: FogFalloff::ExponentialSquared { density: 0.045 },
            ..default()
        },
        Projection::Perspective(PerspectiveProjection {
            fov: 75.0_f32.to_radians(),
            near: 0.05,
            far,
            ..default()
        }),
        Transform::from_xyz(100.0, 80.0, 130.0).looking_at(Vec3::new(0.0, 3.0, 0.0), Vec3::Y),
    ));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn camera_has_explicit_clusters_covering_its_projection() {
        let mut app = App::new();
        app.insert_resource(ClearColor::default())
            .add_systems(Startup, spawn_camera);
        app.update();
        let world = app.world_mut();
        let mut query = world.query::<(&Projection, &ClusterConfig)>();
        let (Projection::Perspective(projection), ClusterConfig::FixedZ { z_config, .. }) = query
            .single(world)
            .expect("One camera with explicit clusters")
        else {
            panic!("Expected perspective clustered lighting");
        };
        assert!(
            matches!(z_config.far_z_mode, ClusterFarZMode::Constant(far) if far == projection.far)
        );
    }
}
