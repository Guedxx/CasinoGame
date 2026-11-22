use bevy::{
    asset::{LoadState, RecursiveDependencyLoadState},
    gltf::GltfMaterialName,
    input::mouse::AccumulatedMouseMotion,
    prelude::*,
    world_serialization::{WorldInstance, WorldInstanceSpawner},
};
use bevy_rapier3d::prelude::*;

use super::{
    camera::MainCamera,
    cursor::GameCursor,
    routine::{GameplayOverlay, OverlayInput},
};
use crate::{localization::{Language, Localization}, state::GameState};

const SPAWN: Vec3 = Vec3::new(1.6, 0.92, 1.3);
const DT: f32 = 1.0 / 60.0;
const EYE_OFFSET: f32 = 0.75;
const BEDSIDE: Vec3 = Vec3::new(1.95, 0.94, 7.16);
// Internal-edge correction assumes outward winding, but the apartment has two-sided surfaces.
const COLLIDER_FLAGS: TriMeshFlags = TriMeshFlags::MERGE_DUPLICATE_VERTICES;

pub struct PlayerPlugin;

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub(super) struct PlayerCamera;

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct PlayerInput;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(RapierPhysicsPlugin::<NoUserData>::default().in_fixed_schedule())
            .insert_resource(Time::<Fixed>::from_hz(60.0))
            .insert_resource(TimestepMode::Fixed {
                dt: DT,
                substeps: 1,
            })
            .add_observer(reset_new_campaign)
            .add_observer(finish_opening)
            .add_observer(start_wake)
            .init_resource::<Controls>()
            .init_resource::<GameplayOverlay>()
            .add_systems(OnEnter(GameState::Playing), loading)
            .add_systems(OnEnter(GameState::Intro), loading)
            .add_systems(OnExit(GameState::Playing), release)
            .add_systems(OnEnter(GameState::Playing), resume)
            .add_systems(Update, loading_visibility)
            .add_systems(
                Update,
                (prepare_apartment, prepare_opening_spawn.run_if(in_state(GameState::Intro)))
                    .chain()
                    .run_if(in_state(GameState::Playing).or_else(in_state(GameState::Intro))),
            )
            .add_systems(
                PreUpdate,
                input
                    .in_set(PlayerInput)
                    .after(bevy::input::InputSystems)
                    .after(OverlayInput)
                    .run_if(in_state(GameState::Playing)),
            )
            .add_systems(
                FixedUpdate,
                (move_player, footsteps)
                    .chain()
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
pub(crate) struct StatusText {
    pub(crate) failed: bool,
}
#[derive(Component, Default)]
pub(crate) struct Player {
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

fn reset_new_campaign(
    _: On<super::campaign_save::CampaignReplaced>,
    mut controls: ResMut<Controls>,
    mut players: Query<(
        &mut Transform,
        &mut Player,
        &mut KinematicCharacterController,
    )>,
) {
    *controls = Controls::default();
    for (mut transform, mut player, mut controller) in &mut players {
        transform.translation = SPAWN;
        player.previous = SPAWN;
        player.vertical_speed = 0.0;
        controller.translation = Some(Vec3::ZERO);
    }
}

fn loading(
    localization: Res<Localization>,
    assets: Res<AssetServer>,
    mut commands: Commands,
    mut controls: ResMut<Controls>,
    existing: Query<(), With<Loading>>,
) {
    if !existing.is_empty() {
        return;
    }
    *controls = default();
    commands.spawn((
        Loading,
        Node {
            width: percent(100),
            height: percent(100),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        },
        children![(
            StatusText { failed: false },
            Text::new(localization.choose("Loading apartment...", "Carregando o apartamento…")),
            TextFont::from_font_size(24.0).with_font(assets.load("fonts/LiberationSans-Regular.ttf"))
        )],
    ));
}

fn prepare_opening_spawn(
    localization: Res<Localization>,
    mut opening: ResMut<super::intro::Opening>,
    context: ReadRapierContext,
    players: Query<Entity, With<Player>>,
    colliders: Query<(), (With<Collider>, Without<RapierColliderHandle>)>,
) {
    if opening.spawn_ready || !opening.error.is_empty() || !colliders.is_empty() {
        return;
    }
    let (Ok(player), Ok(context)) = (players.single(), context.single()) else { return };
    let position = super::intro::END_POSITION;
    let (overlaps, supported) = standing_space(&context, player, position);
    if overlaps || !supported {
        opening.error = localization.choose("Cannot safely stand up in the apartment. Press Escape for title.", "Não foi possível levantar com segurança no apartamento. Pressione Esc para voltar ao menu principal.").into();
        error!("Opening exit blocked or unsupported: overlap={overlaps}, supported={supported}");
    } else {
        opening.spawn_ready = true;
    }
}

fn standing_space(context: &RapierContext, player: Entity, position: Vec3) -> (bool, bool) {
    let capsule = Collider::capsule_y(0.6, 0.3);
    let filter = QueryFilter::default().exclude_collider(player);
    let mut overlaps = false;
    context.intersect_shape(
        position,
        Quat::IDENTITY,
        capsule.raw.as_ref(),
        filter,
        |_| {
            overlaps = true;
            false
        },
    );
    let supported = context
        .cast_ray_and_get_normal(position, Vec3::NEG_Y, 1.0, true, filter)
        .is_some_and(|(_, hit)| hit.normal.y > 0.7 && hit.time_of_impact > 0.9);
    (overlaps, supported)
}

fn start_wake(
    _: On<super::intro::WakeRequested>,
    mut overlay: ResMut<GameplayOverlay>,
    mut controls: ResMut<Controls>,
    mut players: Query<(
        Entity,
        &mut Transform,
        &mut Player,
        &mut KinematicCharacterController,
    )>,
    physics: ReadRapierContext,
) {
    let mut bedside = false;
    if let (Ok(physics), Ok((entity, mut transform, mut player, mut controller))) =
        (physics.single(), players.single_mut())
        && standing_space(&physics, entity, BEDSIDE) == (false, true)
    {
        transform.translation = BEDSIDE;
        player.previous = BEDSIDE;
        player.vertical_speed = 0.0;
        controller.translation = Some(Vec3::ZERO);
        controls.yaw = 0.1;
        controls.pitch = -0.18;
        bedside = true;
    }
    overlay.active = Some(super::routine::OverlayKind::Wake { bedside });
}

fn finish_opening(
    _: On<super::intro::OpeningFinished>,
    mut controls: ResMut<Controls>,
    mut players: Query<(&mut Transform, &mut Player, &mut KinematicCharacterController), Without<MainCamera>>,
    mut cameras: Query<&mut Transform, With<MainCamera>>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut buttons: ResMut<ButtonInput<MouseButton>>,
    mut motion: ResMut<AccumulatedMouseMotion>,
    mut next: ResMut<NextState<GameState>>,
) {
    let position = super::intro::END_POSITION;
    *controls = Controls { yaw: super::intro::END_YAW, pitch: super::intro::END_PITCH, ..default() };
    for (mut transform, mut player, mut controller) in &mut players {
        transform.translation = position;
        player.previous = position;
        player.vertical_speed = 0.0;
        controller.translation = Some(Vec3::ZERO);
    }
    for mut camera in &mut cameras {
        *camera = Transform::from_translation(position + Vec3::Y * EYE_OFFSET)
            .with_rotation(Quat::from_euler(EulerRot::YXZ, controls.yaw, controls.pitch, 0.0));
    }
    keys.reset_all();
    buttons.reset_all();
    motion.delta = Vec2::ZERO;
    next.set(GameState::Playing);
}

fn prepare_apartment(
    localization: Res<Localization>,
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
    mut text: Query<(&mut Text, &mut StatusText)>,
) {
    let Ok((root, scene, instance, built, failed)) = apartment.single() else {
        return;
    };
    if failed {
        return;
    }
    let failure = match assets.get_load_states(scene.0.id()) {
        Some((LoadState::Failed(error), _, _))
        | Some((_, _, RecursiveDependencyLoadState::Failed(error))) => Some(match localization.language {
            Language::English => format!("Could not load apartment: {error}\nPress Escape for menu"),
            Language::Portuguese => "Não foi possível carregar o apartamento. Pressione Esc para voltar ao menu principal.".into(),
        }),
        _ => None,
    };
    if let Some(message) = failure {
        for (mut text, mut status) in &mut text {
                status.failed = true;
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
            if let Some(collider) =
                Collider::from_bevy_mesh(mesh, &ComputedColliderShape::TriMesh(COLLIDER_FLAGS))
            {
                commands.entity(entity).insert(collider);
            } else {
                failed = true;
                error!("Cannot build static collider for {entity:?}");
            }
        }
        if failed {
            commands.entity(root).insert(CollisionFailed);
            for (mut text, mut status) in &mut text {
                status.failed = true;
                **text = localization.choose("Could not build apartment collision. Press Escape for menu", "Não foi possível preparar o apartamento. Pressione Esc para voltar ao menu principal.").into();
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
        for (mut text, mut status) in &mut text {
                status.failed = true;
            **text = localization.choose("Cannot safely spawn inside apartment. Press Escape for menu", "Não foi possível posicionar o personagem no apartamento. Pressione Esc para voltar ao menu principal.").into();
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
    ));
    info!("First-person player ready at {SPAWN:?}; static colliders registered");
    for (mut text, mut status) in &mut text {
                status.failed = true;
        text.clear();
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

fn input(
    overlay: Res<GameplayOverlay>,
    keys: Res<ButtonInput<KeyCode>>,
    motion: Res<AccumulatedMouseMotion>,
    mouse: Res<GameCursor>,
    mut controls: ResMut<Controls>,
    next: Res<NextState<GameState>>,
) {
    controls.captured = mouse.captured;
    if !mouse.captured
        || overlay.blocks_world()
        || overlay.escape_consumed
        || matches!(*next, NextState::Pending(s) | NextState::PendingIfNeq(s) if s != GameState::Playing)
    {
        controls.movement = Vec2::ZERO;
        controls.jump = false;
        controls.sprint = false;
        return;
    }
    if !mouse.suppress_look {
        controls.yaw -= motion.delta.x * 0.002;
        controls.pitch = (controls.pitch - motion.delta.y * 0.002)
            .clamp(-85.0_f32.to_radians(), 85.0_f32.to_radians());
    }
    controls.movement = Vec2::new(
        (keys.pressed(KeyCode::KeyD) as i8 - keys.pressed(KeyCode::KeyA) as i8) as f32,
        (keys.pressed(KeyCode::KeyW) as i8 - keys.pressed(KeyCode::KeyS) as i8) as f32,
    )
    .normalize_or_zero();
    controls.sprint = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    controls.jump |= keys.just_pressed(KeyCode::Space);
}

fn move_player(
    overlay: Res<GameplayOverlay>,
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
    if overlay.blocks_world() || overlay.escape_consumed {
        controller.translation = Some(Vec3::ZERO);
        return;
    }
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

fn footsteps(
    mut commands: Commands,
    assets: Res<AssetServer>,
    controls: Res<Controls>,
    overlay: Res<GameplayOverlay>,
    player: Query<&KinematicCharacterControllerOutput, With<Player>>,
    mut distance: Local<f32>,
    mut sound: Local<Option<[Handle<AudioSource>; 2]>>,
    mut step: Local<usize>,
) {
    let Ok(output) = player.single() else {
        *distance = 0.0;
        return;
    };
    if !controls.captured
        || controls.movement == Vec2::ZERO
        || overlay.blocks_world()
        || overlay.escape_consumed
        || !output.grounded
    {
        *distance = 0.0;
        return;
    }
    *distance += output.effective_translation.xz().length();
    if *distance >= 1.5 {
        *distance %= 1.5;
        let sound = sound.get_or_insert_with(|| {
            [
                assets.load("audio/step1.wav"),
                assets.load("audio/step2.wav"),
            ]
        });
        commands.spawn((
            super::audio::Sound::Step,
            AudioPlayer::new(sound[*step].clone()),
            PlaybackSettings::DESPAWN.with_volume(bevy::audio::Volume::Linear(0.4)),
        ));
        *step = (*step + 1) % 2;
    }
}

fn camera(
    time: Res<Time<Fixed>>,
    controls: Res<Controls>,
    player: Query<(&Transform, &Player), Without<MainCamera>>,
    mut camera: Query<&mut Transform, With<MainCamera>>,
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

fn release(mut controls: ResMut<Controls>, mut time: ResMut<Time<Virtual>>) {
    time.pause();
    controls.movement = Vec2::ZERO;
    controls.jump = false;
    controls.sprint = false;
    controls.captured = false;
}
fn resume(mut time: ResMut<Time<Virtual>>) {
    time.unpause();
    info!("Gameplay resumed");
}

fn loading_visibility(
    state: Res<State<GameState>>,
    controls: Res<Controls>,
    overlay: Res<GameplayOverlay>,
    mut nodes: Query<&mut Node, With<Loading>>,
) {
    for mut node in &mut nodes {
        node.display = if *state.get() == GameState::Playing
            && !controls.captured
            && !overlay.blocks_world()
        {
            Display::Flex
        } else {
            Display::None
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};

    #[test]
    fn footsteps_require_ground_travel_and_stop_when_input_is_blocked() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::asset::AssetPlugin::default()))
            .init_asset::<AudioSource>()
            .init_resource::<Controls>()
            .init_resource::<GameplayOverlay>()
            .add_systems(Update, footsteps);
        {
            let mut controls = app.world_mut().resource_mut::<Controls>();
            controls.captured = true;
            controls.movement = Vec2::Y;
        }
        let player = app
            .world_mut()
            .spawn((
                Player::default(),
                KinematicCharacterControllerOutput {
                    grounded: true,
                    effective_translation: Vec3::X * 0.75,
                    ..default()
                },
            ))
            .id();
        app.update();
        assert_eq!(
            app.world_mut()
                .query::<&AudioPlayer>()
                .iter(app.world())
                .count(),
            0
        );
        app.update();
        assert_eq!(
            app.world_mut()
                .query::<&AudioPlayer>()
                .iter(app.world())
                .count(),
            1
        );
        for (grounded, translation, captured) in [
            (false, Vec3::X * 2.0, true),
            (true, Vec3::ZERO, true),
            (true, Vec3::X * 2.0, false),
        ] {
            {
                let mut output = app
                    .world_mut()
                    .get_mut::<KinematicCharacterControllerOutput>(player)
                    .unwrap();
                output.grounded = grounded;
                output.effective_translation = translation;
            }
            app.world_mut().resource_mut::<Controls>().captured = captured;
            app.update();
            assert_eq!(
                app.world_mut()
                    .query::<&AudioPlayer>()
                    .iter(app.world())
                    .count(),
                1
            );
        }
    }

    fn room() -> (App, Entity) {
        let mut app = App::new();
        app.init_resource::<Assets<Mesh>>();
        app.add_plugins((
            MinimalPlugins,
            TransformPlugin,
            bevy::asset::AssetPlugin::default(),
            RapierPhysicsPlugin::<NoUserData>::default().in_fixed_schedule(),
        ))
        .init_asset::<Font>()
        .insert_resource(TimestepMode::Fixed {
            dt: DT,
            substeps: 1,
        })
        .init_resource::<Controls>()
        .init_resource::<GameplayOverlay>()
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
                    COLLIDER_FLAGS,
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

    fn apartment_room() -> (App, Entity) {
        let (mut app, player) = room();
        let colliders: Vec<_> = app
            .world_mut()
            .query_filtered::<Entity, (With<Collider>, Without<Player>)>()
            .iter(app.world())
            .collect();
        for entity in colliders {
            app.world_mut().despawn(entity);
        }
        let bytes = std::fs::read("assets/models/apartment_windows.glb").unwrap();
        let gltf = bevy::gltf::gltf::Gltf::from_slice(&bytes).unwrap();
        for node in gltf.nodes() {
            let Some(mesh) = node.mesh() else { continue };
            for primitive in mesh.primitives() {
                if primitive
                    .material()
                    .name()
                    .is_some_and(|name| name.starts_with("Urban_"))
                {
                    continue;
                }
                let reader = primitive.reader(|_| gltf.blob.as_deref());
                let vertices = reader.read_positions().unwrap().map(Vec3::from).collect();
                let indices: Vec<_> = reader.read_indices().unwrap().into_u32().collect();
                app.world_mut().spawn((
                    Transform::from_matrix(Mat4::from_cols_array_2d(&node.transform().matrix())),
                    Collider::trimesh_with_flags(
                        vertices,
                        indices
                            .chunks_exact(3)
                            .map(|triangle| [triangle[0], triangle[1], triangle[2]])
                            .collect(),
                        COLLIDER_FLAGS,
                    )
                    .unwrap(),
                ));
            }
        }
        step(&mut app, 5);
        (app, player)
    }

    #[test]
    fn wake_camera_path_and_standing_position_fit_the_actual_bedroom() {
        let (mut app, player) = apartment_room();
        app.add_observer(start_wake);
        app.world_mut().trigger(super::super::intro::WakeRequested);
        assert_eq!(position(&app, player), BEDSIDE);
        assert_eq!(
            app.world().resource::<GameplayOverlay>().active,
            Some(super::super::routine::OverlayKind::Wake { bedside: true })
        );
        let controls = app.world().resource::<Controls>();
        let standing = Transform::from_translation(BEDSIDE + Vec3::Y * EYE_OFFSET).with_rotation(
            Quat::from_euler(EulerRot::YXZ, controls.yaw, controls.pitch, 0.0),
        );
        app.add_systems(Update, move |physics: ReadRapierContext| {
            let physics = physics.single().unwrap();
            let eye = Collider::ball(0.075);
            for frame in 0..=68 {
                let pose = super::super::intro::wake_pose(frame as f32 * 0.05, standing);
                let mut blocked = false;
                physics.intersect_shape(
                    pose.translation,
                    pose.rotation,
                    eye.raw.as_ref(),
                    QueryFilter::default().exclude_collider(player),
                    |_| {
                        blocked = true;
                        false
                    },
                );
                assert!(
                    !blocked,
                    "Wake camera intersects the bedroom at {:?}",
                    pose.translation
                );
            }
        });
        app.update();
    }

    #[test]
    fn opening_exit_is_safe_in_the_actual_apartment_and_handoff_survives_loading() {
        use super::super::intro::{END_PITCH, END_POSITION, END_YAW, Opening, OpeningFinished};
        let (mut app, player) = apartment_room();
        app.init_resource::<Opening>()
            .init_resource::<NextState<GameState>>()
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<ButtonInput<MouseButton>>()
            .init_resource::<AccumulatedMouseMotion>()
            .add_observer(finish_opening)
            .init_resource::<Localization>()
            .add_systems(Update, (loading, prepare_opening_spawn).chain());
        let camera = app.world_mut().spawn((MainCamera, Transform::default())).id();
        app.update();
        assert!(app.world().resource::<Opening>().spawn_ready,
            "{}", app.world().resource::<Opening>().error);
        {
            let mut controls = app.world_mut().resource_mut::<Controls>();
            controls.movement = Vec2::ONE;
            controls.jump = true;
            controls.captured = true;
        }
        app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::Space);
        app.world_mut().resource_mut::<AccumulatedMouseMotion>().delta = Vec2::splat(500.0);
        app.world_mut().trigger(OpeningFinished);
        assert_eq!(position(&app, player), END_POSITION);
        assert_eq!(app.world().get::<Player>(player).unwrap().previous, END_POSITION);
        assert_eq!(app.world().get::<Player>(player).unwrap().vertical_speed, 0.0);
        assert_eq!(app.world().get::<KinematicCharacterController>(player).unwrap().translation, Some(Vec3::ZERO));
        assert!(!app.world().resource::<ButtonInput<KeyCode>>().pressed(KeyCode::Space));
        assert_eq!(app.world().resource::<AccumulatedMouseMotion>().delta, Vec2::ZERO);
        let eye = app.world().get::<Transform>(camera).unwrap();
        assert_eq!(eye.translation, END_POSITION + Vec3::Y * EYE_OFFSET);
        assert!(eye.rotation.abs_diff_eq(Quat::from_euler(EulerRot::YXZ, END_YAW, END_PITCH, 0.0), 1e-6));
        app.update();
        let controls = app.world().resource::<Controls>();
        assert_eq!((controls.yaw, controls.pitch), (END_YAW, END_PITCH));
        assert!(!controls.jump && !controls.captured);
        assert!(matches!(app.world().resource::<NextState<GameState>>(), NextState::Pending(GameState::Playing)));

        let obstacle = app.world_mut().spawn((
            Transform::from_translation(END_POSITION), Collider::cuboid(0.1, 0.1, 0.1),
        )).id();
        step(&mut app, 1);
        app.world_mut().resource_mut::<Opening>().spawn_ready = false;
        app.update();
        assert!(!app.world().resource::<Opening>().spawn_ready);
        assert!(!app.world().resource::<Opening>().error.is_empty());
        app.world_mut().despawn(obstacle);
    }

    #[test]
    fn apartment_long_session_preserves_walking_and_jumping() {
        let (mut app, player) = apartment_room();
        step(&mut app, 60 * 60);
        let start = position(&app, player);
        assert!((start.y - 0.915).abs() < 0.015, "idle position: {start}");
        assert!(
            app.world()
                .get::<Player>(player)
                .unwrap()
                .vertical_speed
                .abs()
                < 1.0,
            "gravity accumulated while standing on the floor: speed={}, output={:?}",
            app.world().get::<Player>(player).unwrap().vertical_speed,
            app.world()
                .get::<KinematicCharacterControllerOutput>(player)
                .unwrap()
        );
        app.world_mut().resource_mut::<Controls>().movement = Vec2::Y;
        for tick in 1..=12 {
            step(&mut app, 1);
            let actual = position(&app, player);
            let expected = start + Vec3::Z * (2.5 * DT * tick as f32);
            assert!(
                actual.abs_diff_eq(expected, 0.01),
                "tick {tick}: {actual} != {expected}"
            );
        }
        let before_jump = position(&app, player);
        app.world_mut().resource_mut::<Controls>().jump = true;
        step(&mut app, 1);
        assert!(position(&app, player).y > before_jump.y + 0.05);
        for tick in 0..60 * 60 * 10 {
            if tick % 90 == 0 {
                let mut controls = app.world_mut().resource_mut::<Controls>();
                controls.yaw = (tick / 90) as f32 * 0.7;
                controls.sprint = tick % 180 == 0;
            }
            let before = position(&app, player);
            step(&mut app, 1);
            let after = position(&app, player);
            assert!(
                after.distance(before) < 0.35,
                "teleport at tick {tick}: {before} -> {after}; vertical speed {}",
                app.world().get::<Player>(player).unwrap().vertical_speed
            );
            assert!(
                after.y > 0.85 && after.y < 1.85,
                "left apartment at tick {tick}: {after}"
            );
        }
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
            .init_resource::<GameplayOverlay>()
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<ButtonInput<MouseButton>>()
            .init_resource::<AccumulatedMouseMotion>()
            .init_resource::<NextState<GameState>>()
            .insert_resource(State::new(GameState::Playing))
            .add_message::<bevy::picking::pointer::PointerInput>()
            .add_message::<bevy::input::mouse::MouseWheel>()
            .add_plugins(super::super::cursor::GameCursorPlugin)
            .add_systems(PreUpdate, input.in_set(PlayerInput));
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
    fn pause_preserves_player_and_camera_direction() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::state::app::StatesPlugin))
            .insert_state(GameState::Playing)
            .init_resource::<Controls>()
            .init_resource::<GameplayOverlay>()
            .add_systems(OnExit(GameState::Playing), release)
            .add_systems(OnEnter(GameState::Playing), resume);
        let position = Vec3::new(2.0, 1.5, 6.0);
        let entity = app
            .world_mut()
            .spawn((
                Player {
                    vertical_speed: 2.0,
                    previous: position,
                },
                Transform::from_translation(position),
            ))
            .id();
        app.update();
        app.world_mut().resource_mut::<Controls>().yaw = 0.4;
        for _ in 0..3 {
            app.world_mut().resource_mut::<Controls>().jump = true;
            app.world_mut()
                .resource_mut::<NextState<GameState>>()
                .set(GameState::Menu);
            app.update();
            assert!(app.world().resource::<Time<Virtual>>().is_paused());
            assert_eq!(
                app.world().get::<Transform>(entity).unwrap().translation,
                position
            );
            assert_eq!(
                app.world().get::<Player>(entity).unwrap().vertical_speed,
                2.0
            );
            let controls = app.world().resource::<Controls>();
            assert!(!controls.jump && !controls.captured);
            assert_eq!(controls.yaw, 0.4);
            app.world_mut()
                .resource_mut::<NextState<GameState>>()
                .set(GameState::Settings);
            app.update();
            assert!(app.world().resource::<Time<Virtual>>().is_paused());
            app.world_mut()
                .resource_mut::<NextState<GameState>>()
                .set(GameState::Playing);
            app.update();
            assert!(!app.world().resource::<Time<Virtual>>().is_paused());
            assert_eq!(
                app.world_mut().query::<&Player>().iter(app.world()).count(),
                1
            );
        }
    }

    #[test]
    fn overlay_keeps_capture_freezes_input_and_resumes_without_a_camera_jump() {
        let mut app = App::new();
        app.init_resource::<Controls>()
            .init_resource::<GameplayOverlay>()
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<ButtonInput<MouseButton>>()
            .init_resource::<AccumulatedMouseMotion>()
            .init_resource::<NextState<GameState>>()
            .insert_resource(State::new(GameState::Playing))
            .add_message::<bevy::picking::pointer::PointerInput>()
            .add_message::<bevy::input::mouse::MouseWheel>()
            .add_plugins(super::super::cursor::GameCursorPlugin)
            .add_systems(PreUpdate, input.in_set(PlayerInput));
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
        app.world_mut().resource_mut::<Controls>().captured = true;
        app.world_mut().resource_mut::<Controls>().movement = Vec2::ONE;
        app.world_mut().resource_mut::<GameplayOverlay>().active =
            Some(super::super::routine::OverlayKind::Context);
        let yaw = app.world().resource::<Controls>().yaw;
        app.world_mut()
            .resource_mut::<AccumulatedMouseMotion>()
            .delta = Vec2::ONE;
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyW);
        app.update();
        let controls = app.world().resource::<Controls>();
        assert_eq!(controls.yaw, yaw);
        assert_eq!(controls.movement, Vec2::ZERO);
        assert!(controls.captured);
        assert_eq!(
            app.world().get::<CursorOptions>(window).unwrap().grab_mode,
            CursorGrabMode::Locked
        );
        assert!(matches!(
            app.world().resource::<NextState<GameState>>(),
            NextState::Unchanged
        ));
        app.world_mut().resource_mut::<GameplayOverlay>().active = None;
        app.world_mut()
            .resource_mut::<AccumulatedMouseMotion>()
            .delta = Vec2::splat(120.0);
        app.update();
        let controls = app.world().resource::<Controls>();
        assert!(controls.captured);
        assert_eq!(controls.yaw, yaw);
        assert_eq!(controls.movement, Vec2::Y);
        app.update();
        assert!(app.world().resource::<Controls>().yaw < yaw);
        app.world_mut().get_mut::<Window>(window).unwrap().focused = false;
        app.update();
        assert!(matches!(
            app.world().resource::<NextState<GameState>>(),
            NextState::Pending(GameState::Menu)
        ));
    }

    #[test]
    fn new_campaign_resets_player_and_all_control_state() {
        let mut app = App::new();
        app.init_resource::<Controls>()
            .add_observer(reset_new_campaign);
        let entity = app
            .world_mut()
            .spawn((
                Player {
                    previous: Vec3::ONE,
                    vertical_speed: 4.0,
                },
                Transform::from_translation(Vec3::ONE),
                character_controller(),
            ))
            .id();
        {
            let mut controls = app.world_mut().resource_mut::<Controls>();
            controls.yaw = 0.5;
            controls.pitch = 0.6;
            controls.movement = Vec2::ONE;
            controls.captured = true;
            controls.jump = true;
        }
        app.world_mut()
            .trigger(super::super::campaign_save::CampaignReplaced);
        assert_eq!(
            app.world().get::<Transform>(entity).unwrap().translation,
            SPAWN
        );
        let player = app.world().get::<Player>(entity).unwrap();
        assert_eq!(player.previous, SPAWN);
        assert_eq!(player.vertical_speed, 0.0);
        let controls = app.world().resource::<Controls>();
        assert_eq!(controls.yaw, Controls::default().yaw);
        assert_eq!(controls.pitch, 0.0);
        assert_eq!(controls.movement, Vec2::ZERO);
        assert!(!controls.captured && !controls.jump);
    }

    #[test]
    fn world_camera_query_initializes_and_ignores_auxiliary_cameras() {
        let mut app = App::new();
        app.init_resource::<Time<Fixed>>()
            .init_resource::<Controls>()
            .add_systems(Update, camera);
        let position = Vec3::new(1.0, 2.0, 3.0);
        app.world_mut().spawn((
            Player {
                previous: position,
                vertical_speed: 0.0,
            },
            Transform::from_translation(position),
        ));
        let main = app
            .world_mut()
            .spawn((Camera3d::default(), MainCamera, Transform::default()))
            .id();
        let auxiliary_transform = Transform::from_xyz(9.0, 8.0, 7.0);
        let auxiliary = app
            .world_mut()
            .spawn((Camera3d::default(), auxiliary_transform))
            .id();
        app.update();
        let main = app.world().get::<Transform>(main).unwrap();
        assert_eq!(main.translation, position + Vec3::Y * EYE_OFFSET);
        assert_eq!(main.rotation, Quat::from_rotation_y(std::f32::consts::PI));
        assert_eq!(
            *app.world().get::<Transform>(auxiliary).unwrap(),
            auxiliary_transform
        );
    }
}
