use bevy::{
    asset::{LoadState, RecursiveDependencyLoadState},
    gltf::GltfMaterialName,
    input::mouse::AccumulatedMouseMotion,
    prelude::*,
    window::{CursorGrabMode, CursorOptions, PrimaryWindow},
    world_serialization::{WorldInstance, WorldInstanceSpawner},
};
use bevy_rapier3d::prelude::*;

use crate::state::GameState;

const SPAWN: Vec3 = Vec3::new(1.6, 0.92, 1.3);
const DT: f32 = 1.0 / 60.0;
const EYE_OFFSET: f32 = 0.75;

pub struct PlayerPlugin;

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub(super) struct PlayerCamera;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(RapierPhysicsPlugin::<NoUserData>::default().in_fixed_schedule())
            .insert_resource(Time::<Fixed>::from_hz(60.0))
            .insert_resource(TimestepMode::Fixed {
                dt: DT,
                substeps: 1,
            })
            .init_resource::<Controls>()
            .add_systems(OnEnter(GameState::Playing), loading)
            .add_systems(OnExit(GameState::Playing), release)
            .add_systems(OnEnter(GameState::Menu), overview)
            .add_systems(
                Update,
                prepare_apartment.run_if(in_state(GameState::Playing)),
            )
            .add_systems(
                PreUpdate,
                input
                    .after(bevy::input::InputSystems)
                    .run_if(in_state(GameState::Playing)),
            )
            .add_systems(
                FixedUpdate,
                move_player
                    .before(PhysicsSet::SyncBackend)
                    .run_if(in_state(GameState::Playing)),
            )
            .add_systems(
                PostUpdate,
                camera
                    .in_set(PlayerCamera)
                    .before(TransformSystems::Propagate)
                    .run_if(in_state(GameState::Playing)),
            );
    }
}

#[derive(Component)]
pub(crate) struct Apartment;
#[derive(Component)]
struct ApartmentColliders;
#[derive(Component)]
struct CollisionFailed;
#[derive(Component)]
struct Loading;
#[derive(Component)]
struct StatusText;
#[derive(Component)]
struct Player {
    vertical_speed: f32,
    previous: Vec3,
}
#[derive(Resource)]
struct Controls {
    yaw: f32,
    pitch: f32,
    movement: Vec2,
    sprint: bool,
    jump: bool,
    captured: bool,
}
impl Default for Controls {
    fn default() -> Self {
        Self {
            yaw: std::f32::consts::PI,
            pitch: 0.0,
            movement: Vec2::ZERO,
            sprint: false,
            jump: false,
            captured: false,
        }
    }
}

fn loading(mut commands: Commands, mut controls: ResMut<Controls>) {
    *controls = default();
    commands.spawn((
        Loading,
        DespawnOnExit(GameState::Playing),
        Node {
            width: percent(100),
            height: percent(100),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        },
        children![(
            StatusText,
            Text::new("Loading apartment..."),
            TextFont::from_font_size(24.0)
        )],
    ));
}

fn prepare_apartment(
    mut commands: Commands,
    apartment: Query<
        (
            Entity,
            &WorldAssetRoot,
            Option<&WorldInstance>,
            Has<ApartmentColliders>,
            Has<CollisionFailed>,
        ),
        With<Apartment>,
    >,
    spawner: Res<WorldInstanceSpawner>,
    assets: Res<AssetServer>,
    meshes: Res<Assets<Mesh>>,
    children: Query<&Children>,
    mesh_entities: Query<(Entity, &Mesh3d, Option<&GltfMaterialName>)>,
    colliders: Query<(), (With<Collider>, Without<RapierColliderHandle>)>,
    context: ReadRapierContext,
    player: Query<(), With<Player>>,
    mut text: Query<&mut Text, With<StatusText>>,
) {
    let Ok((root, scene, instance, built, failed)) = apartment.single() else {
        return;
    };
    if failed {
        return;
    }
    let failure = match assets.get_load_states(scene.0.id()) {
        Some((LoadState::Failed(error), _, _))
        | Some((_, _, RecursiveDependencyLoadState::Failed(error))) => Some(format!(
            "Could not load apartment: {error}\nPress Escape for menu"
        )),
        _ => None,
    };
    if let Some(message) = failure {
        for mut text in &mut text {
            **text = message.clone();
        }
        return;
    }
    let Some(instance) = instance else { return };
    if !spawner.instance_is_ready(**instance) {
        return;
    }
    if !built {
        let mut failed = false;
        for entity in children.iter_descendants(root) {
            let Ok((entity, mesh, material)) = mesh_entities.get(entity) else {
                continue;
            };
            // The city is scenery beyond the apartment's sealed collision shell.
            if material.is_some_and(|name| name.0.starts_with("Urban_")) {
                continue;
            }
            let Some(mesh) = meshes.get(mesh) else { return };
            if let Some(collider) = Collider::from_bevy_mesh(
                mesh,
                &ComputedColliderShape::TriMesh(
                    TriMeshFlags::MERGE_DUPLICATE_VERTICES | TriMeshFlags::FIX_INTERNAL_EDGES,
                ),
            ) {
                commands.entity(entity).insert(collider);
            } else {
                failed = true;
                error!("Cannot build static collider for {entity:?}");
            }
        }
        if failed {
            commands.entity(root).insert(CollisionFailed);
            for mut text in &mut text {
                **text = "Could not build apartment collision. Press Escape for menu".into();
            }
            return;
        }
        commands.entity(root).insert(ApartmentColliders);
        return;
    }
    if !colliders.is_empty() || !player.is_empty() {
        return;
    }
    let Ok(context) = context.single() else {
        return;
    };
    let capsule = Collider::capsule_y(0.6, 0.3);
    let mut overlaps = false;
    context.intersect_shape(
        SPAWN,
        Quat::IDENTITY,
        capsule.raw.as_ref(),
        QueryFilter::default(),
        |_| {
            overlaps = true;
            false
        },
    );
    let supported = context
        .cast_ray_and_get_normal(SPAWN, Vec3::NEG_Y, 1.0, true, QueryFilter::default())
        .is_some_and(|(_, hit)| hit.normal.y > 0.7 && hit.time_of_impact > 0.9);
    if overlaps || !supported {
        commands.entity(root).insert(CollisionFailed);
        error!("Player spawn blocked or unsupported: overlap={overlaps}, supported={supported}");
        for mut text in &mut text {
            **text = "Cannot safely spawn inside apartment. Press Escape for menu".into();
        }
        return;
    }
    commands.spawn((
        Player {
            vertical_speed: 0.0,
            previous: SPAWN,
        },
        Transform::from_translation(SPAWN),
        Collider::capsule_y(0.6, 0.3),
        RigidBody::KinematicPositionBased,
        character_controller(),
        DespawnOnExit(GameState::Playing),
    ));
    info!("First-person player ready at {SPAWN:?}; static colliders registered");
    for mut text in &mut text {
        **text = "Click to play | WASD move | Shift sprint | Space jump | Esc menu".into();
    }
}

fn character_controller() -> KinematicCharacterController {
    KinematicCharacterController {
        offset: CharacterLength::Absolute(0.02),
        slide: true,
        autostep: Some(CharacterAutostep {
            max_height: CharacterLength::Absolute(0.25),
            min_width: CharacterLength::Absolute(0.3),
            include_dynamic_bodies: false,
        }),
        snap_to_ground: Some(CharacterLength::Absolute(0.2)),
        max_slope_climb_angle: 45.0_f32.to_radians(),
        min_slope_slide_angle: 45.0_f32.to_radians(),
        apply_impulse_to_dynamic_bodies: false,
        ..default()
    }
}

fn pointer_locked(cursor: &CursorOptions) -> bool {
    #[cfg(target_arch = "wasm32")]
    {
        let _ = cursor;
        web_sys::window()
            .and_then(|window| window.document())
            .and_then(|document| document.pointer_lock_element())
            .is_some()
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        cursor.grab_mode != CursorGrabMode::None
    }
}

fn input(
    mut keys: ResMut<ButtonInput<KeyCode>>,
    buttons: Res<ButtonInput<MouseButton>>,
    motion: Res<AccumulatedMouseMotion>,
    mut window: Query<(&Window, &mut CursorOptions), With<PrimaryWindow>>,
    mut controls: ResMut<Controls>,
    player: Query<(), With<Player>>,
    mut next: ResMut<NextState<GameState>>,
    mut status: Query<&mut Node, With<Loading>>,
) {
    let Ok((window, mut cursor)) = window.single_mut() else {
        return;
    };
    if keys.just_pressed(KeyCode::Escape) || !window.focused {
        cursor.grab_mode = CursorGrabMode::None;
        cursor.visible = true;
    } else if buttons.just_pressed(MouseButton::Left) && !player.is_empty() {
        cursor.grab_mode = CursorGrabMode::Locked;
        cursor.visible = false;
    }
    let active = window.focused && pointer_locked(&cursor) && !keys.just_pressed(KeyCode::Escape);
    if !active {
        controls.movement = Vec2::ZERO;
        controls.jump = false;
        if controls.captured {
            cursor.grab_mode = CursorGrabMode::None;
            cursor.visible = true;
            keys.reset_all();
            next.set(GameState::Menu);
        }
    } else {
        controls.yaw -= motion.delta.x * 0.002;
        controls.pitch = (controls.pitch - motion.delta.y * 0.002)
            .clamp(-85.0_f32.to_radians(), 85.0_f32.to_radians());
        controls.movement = Vec2::new(
            (keys.pressed(KeyCode::KeyD) as i8 - keys.pressed(KeyCode::KeyA) as i8) as f32,
            (keys.pressed(KeyCode::KeyW) as i8 - keys.pressed(KeyCode::KeyS) as i8) as f32,
        )
        .normalize_or_zero();
        controls.sprint = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
        controls.jump |= keys.just_pressed(KeyCode::Space);
    }
    controls.captured = active;
    for mut node in &mut status {
        node.display = if active { Display::None } else { Display::Flex };
    }
}

fn move_player(
    mut controls: ResMut<Controls>,
    mut query: Query<(
        &Transform,
        &mut Player,
        &mut KinematicCharacterController,
        Option<&KinematicCharacterControllerOutput>,
    )>,
) {
    let Ok((transform, mut player, mut controller, output)) = query.single_mut() else {
        return;
    };
    player.previous = transform.translation;
    let grounded = output.is_some_and(|output| output.grounded);
    if grounded && player.vertical_speed < 0.0 {
        player.vertical_speed = 0.0;
    }
    if player.vertical_speed > 0.0
        && output.is_some_and(|output| {
            output.effective_translation.y + 0.001 < output.desired_translation.y
        })
    {
        player.vertical_speed = 0.0;
    }
    if controls.jump && grounded {
        player.vertical_speed = 5.0;
    }
    controls.jump = false;
    player.vertical_speed -= 18.0 * DT;
    let direction = Quat::from_rotation_y(controls.yaw)
        * Vec3::new(controls.movement.x, 0.0, -controls.movement.y);
    controller.translation = Some(
        (direction * if controls.sprint { 4.5 } else { 2.5 } + Vec3::Y * player.vertical_speed)
            * DT,
    );
}

fn camera(
    time: Res<Time<Fixed>>,
    controls: Res<Controls>,
    player: Query<(&Transform, &Player), Without<Camera3d>>,
    mut camera: Query<&mut Transform, With<Camera3d>>,
) {
    let (Ok((transform, player)), Ok(mut camera)) = (player.single(), camera.single_mut()) else {
        return;
    };
    camera.translation = player
        .previous
        .lerp(transform.translation, time.overstep_fraction())
        + Vec3::Y * EYE_OFFSET;
    camera.rotation = Quat::from_euler(EulerRot::YXZ, controls.yaw, controls.pitch, 0.0);
}

fn release(
    mut window: Query<&mut CursorOptions, With<PrimaryWindow>>,
    mut controls: ResMut<Controls>,
) {
    if let Ok(mut cursor) = window.single_mut() {
        cursor.grab_mode = CursorGrabMode::None;
        cursor.visible = true;
    }
    *controls = default();
}
fn overview(mut camera: Query<&mut Transform, With<Camera3d>>) {
    for mut camera in &mut camera {
        *camera =
            Transform::from_xyz(100.0, 80.0, 130.0).looking_at(Vec3::new(0.0, 3.0, 0.0), Vec3::Y);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn room() -> (App, Entity) {
        let mut app = App::new();
        app.init_resource::<Assets<Mesh>>();
        app.add_plugins((
            MinimalPlugins,
            TransformPlugin,
            RapierPhysicsPlugin::<NoUserData>::default().in_fixed_schedule(),
        ))
        .insert_resource(TimestepMode::Fixed {
            dt: DT,
            substeps: 1,
        })
        .init_resource::<Controls>()
        .add_systems(FixedUpdate, move_player.before(PhysicsSet::SyncBackend));
        // Hollow triangle surfaces exercise the same collider type as the GLB, not a solid room box.
        for vertices in [
            [
                Vec3::new(0.027, -0.005, -4.6),
                Vec3::new(3.47, -0.005, -4.6),
                Vec3::new(3.47, -0.005, 9.223),
                Vec3::new(0.027, -0.005, 9.223),
            ],
            [
                Vec3::new(0.027, 2.752, -4.6),
                Vec3::new(0.027, 2.752, 9.223),
                Vec3::new(3.47, 2.752, 9.223),
                Vec3::new(3.47, 2.752, -4.6),
            ],
            [
                Vec3::new(3.47, -0.005, -4.6),
                Vec3::new(3.47, 2.752, -4.6),
                Vec3::new(3.47, 2.752, 9.223),
                Vec3::new(3.47, -0.005, 9.223),
            ],
            [
                Vec3::new(0.027, 0.0, 2.44),
                Vec3::new(0.027, 0.0, 3.31),
                Vec3::new(0.027, 2.3, 3.31),
                Vec3::new(0.027, 2.3, 2.44),
            ],
            [
                Vec3::new(0.027, 0.9, 5.0),
                Vec3::new(0.027, 0.9, 6.5),
                Vec3::new(0.027, 2.0, 6.5),
                Vec3::new(0.027, 2.0, 5.0),
            ],
        ] {
            app.world_mut().spawn((
                Transform::default(),
                Collider::trimesh_with_flags(
                    vertices.to_vec(),
                    vec![[0, 2, 1], [0, 3, 2]],
                    TriMeshFlags::FIX_INTERNAL_EDGES,
                )
                .unwrap(),
            ));
        }
        app.world_mut().spawn((
            Transform::from_xyz(1.75, 0.495, 6.0),
            Collider::cuboid(0.35, 0.5, 0.35),
        ));
        let player = app
            .world_mut()
            .spawn((
                Player {
                    vertical_speed: 0.0,
                    previous: SPAWN,
                },
                Transform::from_translation(SPAWN),
                Collider::capsule_y(0.6, 0.3),
                RigidBody::KinematicPositionBased,
                character_controller(),
            ))
            .id();
        app.finish();
        app.cleanup();
        app.update();
        step(&mut app, 5);
        (app, player)
    }

    fn step(app: &mut App, count: usize) {
        for _ in 0..count {
            app.world_mut().run_schedule(FixedUpdate);
        }
    }
    fn position(app: &App, player: Entity) -> Vec3 {
        app.world().get::<Transform>(player).unwrap().translation
    }
    fn teleport(app: &mut App, player: Entity, position: Vec3) {
        app.world_mut()
            .get_mut::<Transform>(player)
            .unwrap()
            .translation = position;
        app.world_mut()
            .get_mut::<Player>(player)
            .unwrap()
            .vertical_speed = 0.0;
        app.world_mut().resource_mut::<Controls>().movement = Vec2::ZERO;
        step(app, 1);
    }

    #[test]
    fn floor_wall_slide_furniture_and_closed_openings() {
        let (mut app, player) = room();
        assert!((position(&app, player).y - 0.915).abs() < 0.015);
        assert!(
            app.world()
                .get::<KinematicCharacterControllerOutput>(player)
                .unwrap()
                .grounded
        );
        app.world_mut().resource_mut::<Controls>().movement = Vec2::Y;
        step(&mut app, 12);
        assert!((position(&app, player).z - SPAWN.z - 0.5).abs() < 0.01);
        app.world_mut().resource_mut::<Controls>().sprint = true;
        step(&mut app, 12);
        assert!((position(&app, player).z - SPAWN.z - 1.4).abs() < 0.01);
        app.world_mut().resource_mut::<Controls>().sprint = false;
        teleport(&mut app, player, SPAWN);
        {
            let mut controls = app.world_mut().resource_mut::<Controls>();
            controls.yaw = 0.0;
            controls.movement = Vec2::new(1.0, -1.0).normalize();
        }
        step(&mut app, 80);
        let p = position(&app, player);
        assert!(
            p.x < 3.16 && p.x > 3.1 && p.z > 3.4,
            "wall should block X and retain Z slide: {p}"
        );
        for (start, label) in [
            (Vec3::new(0.8, 0.92, 2.875), "door"),
            (Vec3::new(0.65, 1.45, 5.75), "window screen"),
        ] {
            teleport(&mut app, player, start);
            app.world_mut().resource_mut::<Controls>().movement = Vec2::NEG_X;
            step(&mut app, 15);
            assert!(position(&app, player).x > 0.33, "crossed closed {label}");
        }
        teleport(&mut app, player, Vec3::new(1.75, 0.92, 4.7));
        app.world_mut().resource_mut::<Controls>().movement = Vec2::NEG_Y;
        step(&mut app, 60);
        assert!(position(&app, player).z < 5.36, "passed through furniture");
    }

    #[test]
    fn grounded_jump_ceiling_reset_and_no_air_jump() {
        let (mut app, player) = room();
        app.world_mut().resource_mut::<Controls>().jump = true;
        step(&mut app, 1);
        assert!(position(&app, player).y > 0.97);
        let mut peak = 0.0_f32;
        let mut ceiling_hit = false;
        for tick in 0..80 {
            if tick == 4 {
                app.world_mut().resource_mut::<Controls>().jump = true;
            }
            step(&mut app, 1);
            peak = peak.max(position(&app, player).y);
            let output = app
                .world()
                .get::<KinematicCharacterControllerOutput>(player)
                .unwrap();
            if output.desired_translation.y > 0.001
                && output.effective_translation.y + 0.001 < output.desired_translation.y
            {
                ceiling_hit = true;
                step(&mut app, 1);
                assert!(app.world().get::<Player>(player).unwrap().vertical_speed <= 0.0);
            }
        }
        // The ordinary jump fits under this ceiling. An elevated start specifically exercises a head hit.
        assert!(
            peak < 1.64 && peak > 1.5,
            "air jump changed the ballistic peak: {peak}"
        );
        assert!(
            app.world()
                .get::<KinematicCharacterControllerOutput>(player)
                .unwrap()
                .grounded
        );
        teleport(&mut app, player, Vec3::new(1.6, 1.7, 1.3));
        app.world_mut()
            .get_mut::<Player>(player)
            .unwrap()
            .vertical_speed = 5.0;
        for _ in 0..15 {
            step(&mut app, 1);
            let output = app
                .world()
                .get::<KinematicCharacterControllerOutput>(player)
                .unwrap();
            if output.desired_translation.y > 0.001
                && output.effective_translation.y + 0.001 < output.desired_translation.y
            {
                ceiling_hit = true;
                step(&mut app, 1);
                assert!(app.world().get::<Player>(player).unwrap().vertical_speed < 0.0);
            }
            assert!(position(&app, player).y < 1.84);
        }
        assert!(ceiling_hit);
    }

    #[test]
    fn keyboard_diagonal_is_normalized_and_lock_loss_clears_controls() {
        let mut app = App::new();
        app.init_resource::<Controls>()
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<ButtonInput<MouseButton>>()
            .init_resource::<AccumulatedMouseMotion>()
            .init_resource::<NextState<GameState>>()
            .add_systems(PreUpdate, input);
        app.world_mut().spawn(Player {
            vertical_speed: 0.0,
            previous: SPAWN,
        });
        let window = app
            .world_mut()
            .spawn((
                Window {
                    focused: true,
                    ..default()
                },
                PrimaryWindow,
                CursorOptions {
                    grab_mode: CursorGrabMode::Locked,
                    ..default()
                },
            ))
            .id();
        {
            let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
            keys.press(KeyCode::KeyW);
            keys.press(KeyCode::KeyD);
            keys.press(KeyCode::Space);
        }
        app.update();
        let controls = app.world().resource::<Controls>();
        assert!((controls.movement.length() - 1.0).abs() < 0.0001 && controls.jump);
        app.world_mut()
            .get_mut::<CursorOptions>(window)
            .unwrap()
            .grab_mode = CursorGrabMode::None;
        app.update();
        let controls = app.world().resource::<Controls>();
        assert_eq!(controls.movement, Vec2::ZERO);
        assert!(!controls.jump && !controls.captured);
        assert!(
            !app.world()
                .resource::<ButtonInput<KeyCode>>()
                .pressed(KeyCode::KeyW)
        );
        assert!(matches!(
            app.world().resource::<NextState<GameState>>(),
            NextState::Pending(GameState::Menu)
        ));
    }
    #[test]
    fn leaving_and_reentering_despawns_the_previous_player() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::state::app::StatesPlugin))
            .init_state::<GameState>()
            .init_resource::<Controls>()
            .add_systems(OnEnter(GameState::Playing), |mut commands: Commands| {
                commands.spawn((
                    Player {
                        vertical_speed: 0.0,
                        previous: SPAWN,
                    },
                    DespawnOnExit(GameState::Playing),
                ));
            })
            .add_systems(OnExit(GameState::Playing), release);
        for _ in 0..3 {
            app.update();
            assert_eq!(
                app.world_mut().query::<&Player>().iter(app.world()).count(),
                1
            );
            app.world_mut().resource_mut::<Controls>().jump = true;
            app.world_mut()
                .resource_mut::<NextState<GameState>>()
                .set(GameState::Menu);
            app.update();
            assert_eq!(
                app.world_mut().query::<&Player>().iter(app.world()).count(),
                0
            );
            assert!(!app.world().resource::<Controls>().jump);
            app.world_mut()
                .resource_mut::<NextState<GameState>>()
                .set(GameState::Playing);
        }
    }
}
