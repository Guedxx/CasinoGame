//! One new-campaign opening, driven by focused real time rather than campaign time.
use bevy::{
    app::AnimationSystems,
    asset::{LoadState, RecursiveDependencyLoadState, embedded_asset},
    post_process::dof::{DepthOfField, DepthOfFieldMode},
    prelude::*,
    render::render_resource::AsBindGroup,
    shader::ShaderRef,
    ui::FocusPolicy,
    window::PrimaryWindow,
    world_serialization::{WorldAsset, WorldInstanceReady},
};

use super::{
    camera::MainCamera,
    player::{PlayerCamera, StatusText},
    routine::{GameplayOverlay, OverlayKind},
    tv,
};
use crate::{
    localization::{Language, Localization},
    state::GameState,
};

pub(crate) const END_POSITION: Vec3 = Vec3::new(2.15, 0.92, -0.90);
pub(crate) const END_YAW: f32 = 1.97;
pub(crate) const END_PITCH: f32 = -0.24;
const DURATION: f32 = 45.0;
const WAKE_DURATION: f32 = 3.4;

pub(crate) struct IntroPlugin;

#[derive(Resource, Default)]
pub(crate) struct Opening {
    pub(crate) spawn_ready: bool,
    pub(crate) error: String,
    ready: bool,
    elapsed: f32,
    skip_armed: bool,
    skip_hold: f32,
    finished: bool,
}

impl Opening {
    fn advance(&mut self, delta: f32, focused: bool, enter: bool) -> bool {
        if !focused || !self.ready || !self.error.is_empty() {
            self.skip_hold = 0.0;
            self.skip_armed = false;
            return false;
        }
        // A background-tab delay must not jump over dialogue when focus returns.
        let delta = delta.clamp(0.0, 0.1);
        if !enter {
            self.skip_armed = true;
            self.skip_hold = 0.0;
        } else if self.skip_armed {
            self.skip_hold += delta;
        }
        self.elapsed = (self.elapsed + delta).min(DURATION);
        if self.elapsed >= DURATION || self.skip_hold >= 1.0 {
            self.elapsed = DURATION;
            return true;
        }
        false
    }
}

#[derive(Event)]
pub(crate) struct OpeningFinished;

#[derive(Event)]
pub(super) struct WakeRequested;

#[derive(Resource)]
struct OpeningAssets {
    scene: Handle<WorldAsset>,
    clip: Handle<AnimationClip>,
    graph: Handle<AnimationGraph>,
    animation: AnimationNodeIndex,
    font: Handle<Font>,
    eyelids: Handle<Eyelids>,
}

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
struct Eyelids {
    #[uniform(0)]
    opening: Vec4,
}

impl UiMaterial for Eyelids {
    fn fragment_shader() -> ShaderRef {
        "embedded://CasinoGame/game/eyelids.wgsl".into()
    }
}

#[derive(Component)]
struct OpeningBody;
#[derive(Component)]
struct OpeningActor;
#[derive(Component)]
struct OpeningEye;
#[derive(Component)]
struct OpeningCover;
#[derive(Component)]
struct OpeningText;
#[derive(Component)]
struct Subtitle;

#[derive(Component)]
struct WakeUi;

#[derive(Default)]
struct WakePlayback {
    elapsed: f32,
    skip_armed: bool,
}

impl WakePlayback {
    fn advance(&mut self, delta: f32, focused: bool, enter: bool) -> bool {
        if !focused {
            self.skip_armed = false;
            return false;
        }
        self.skip_armed |= !enter;
        self.elapsed += delta.min(0.1);
        self.elapsed >= WAKE_DURATION || (self.skip_armed && enter)
    }
}

impl Plugin for IntroPlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "eyelids.wgsl");
        app.add_plugins(UiMaterialPlugin::<Eyelids>::default())
            .init_resource::<Opening>()
            .add_systems(Startup, load)
            .add_systems(OnEnter(GameState::Intro), enter)
            .add_systems(
                Update,
                (tick, animate_eyelids)
                    .chain()
                    .run_if(in_state(GameState::Intro)),
            )
            .add_systems(
                PostUpdate,
                camera
                    .after(AnimationSystems)
                    .before(TransformSystems::Propagate)
                    .run_if(in_state(GameState::Intro)),
            )
            .add_systems(
                PostUpdate,
                wake.after(PlayerCamera)
                    .before(bevy::ui::UiSystems::Prepare)
                    .before(TransformSystems::Propagate)
                    .run_if(in_state(GameState::Playing)),
            )
            .add_systems(OnExit(GameState::Intro), cleanup);
    }
}

fn load(
    mut commands: Commands,
    assets: Res<AssetServer>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
    mut eyelids: ResMut<Assets<Eyelids>>,
) {
    let clip = assets.load(GltfAssetLabel::Animation(0).from_asset("models/opening_body.glb"));
    let (graph, animation) = AnimationGraph::from_clip(clip.clone());
    commands.insert_resource(OpeningAssets {
        scene: assets.load(GltfAssetLabel::Scene(0).from_asset("models/opening_body.glb")),
        clip,
        graph: graphs.add(graph),
        animation,
        font: assets.load("fonts/LiberationSans-Regular.ttf"),
        eyelids: eyelids.add(Eyelids {
            opening: Vec4::ZERO,
        }),
    });
}

fn enter(
    localization: Res<Localization>,
    mut commands: Commands,
    mut opening: ResMut<Opening>,
    assets: Res<OpeningAssets>,
    mut time: ResMut<Time<Virtual>>,
    cameras: Query<Entity, With<MainCamera>>,
) {
    *opening = Opening::default();
    // Rapier must register apartment colliders even after returning from a paused campaign.
    time.unpause();
    commands
        .spawn((
            Name::new("Opening body"),
            OpeningBody,
            WorldAssetRoot(assets.scene.clone()),
            Transform::IDENTITY,
            DespawnOnExit(GameState::Intro),
        ))
        .observe(body_ready);
    for entity in &cameras {
        commands.entity(entity).insert(DepthOfField {
            mode: DepthOfFieldMode::Gaussian,
            focal_distance: 0.15,
            aperture_f_stops: 0.4,
            max_circle_of_confusion_diameter: 12.0,
            ..default()
        });
    }
    commands.spawn((
        DespawnOnExit(GameState::Intro),
        eyelid_overlay(assets.eyelids.clone(), true),
    ));
    commands.spawn((
        OpeningText,
        Subtitle,
        DespawnOnExit(GameState::Intro),
        dialogue_box(localization.text("loading_opening"), assets.font.clone()),
    ));
    commands.spawn((
        OpeningText,
        DespawnOnExit(GameState::Intro),
        skip_hint("", assets.font.clone()),
    ));
}

fn eyelid_overlay(eyelids: Handle<Eyelids>, closed: bool) -> impl Bundle {
    (
        GlobalZIndex(1000),
        Node {
            position_type: PositionType::Absolute,
            width: percent(100),
            height: percent(100),
            ..default()
        },
        MaterialNode(eyelids),
        Pickable::IGNORE,
        FocusPolicy::Pass,
        // Keep the viewport black while the material pipeline warms up.
        children![(
            OpeningCover,
            Node {
                width: percent(100),
                height: percent(100),
                display: if closed { Display::Flex } else { Display::None },
                ..default()
            },
            BackgroundColor(Color::BLACK),
            Pickable::IGNORE,
            FocusPolicy::Pass,
        )],
    )
}

fn skip_hint(text: &'static str, font: Handle<Font>) -> impl Bundle {
    (
        GlobalZIndex(1001),
        Text::new(text),
        TextFont::from_font_size(16.0).with_font(font),
        TextColor(Color::srgb(0.85, 0.84, 0.79)),
        Node {
            position_type: PositionType::Absolute,
            bottom: px(20),
            right: px(24),
            ..default()
        },
        Pickable::IGNORE,
        FocusPolicy::Pass,
    )
}

pub(super) fn dialogue_box(text: impl Into<String>, font: Handle<Font>) -> impl Bundle {
    (
        GlobalZIndex(1001),
        Text::new(text),
        TextFont::from_font_size(24.0).with_font(font),
        TextColor(Color::srgb(0.95, 0.93, 0.87)),
        TextLayout::justify(Justify::Center),
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.75)),
        Node {
            position_type: PositionType::Absolute,
            bottom: percent(12),
            left: percent(10),
            width: percent(80),
            padding: UiRect::axes(px(16), px(10)),
            ..default()
        },
        Pickable::IGNORE,
        FocusPolicy::Pass,
    )
}

fn body_ready(
    ready: On<WorldInstanceReady>,
    localization: Res<Localization>,
    mut commands: Commands,
    assets: Res<OpeningAssets>,
    children: Query<&Children>,
    names: Query<&Name>,
    mut actors: Query<&mut AnimationPlayer>,
    mut opening: ResMut<Opening>,
) {
    let mut found_eye = false;
    let mut found_actor = false;
    for entity in children.iter_descendants(ready.entity) {
        if names
            .get(entity)
            .is_ok_and(|name| name.as_str() == "IntroEye")
        {
            commands.entity(entity).insert(OpeningEye);
            found_eye = true;
        }
        if let Ok(mut actor) = actors.get_mut(entity) {
            actor.play(assets.animation).pause().seek_to(0.0);
            commands
                .entity(entity)
                .insert((OpeningActor, AnimationGraphHandle(assets.graph.clone())));
            found_actor = true;
        }
    }
    if !found_eye || !found_actor {
        opening.error = localization.choose("The opening animation is incomplete. Press Escape for title.", "A animação de abertura está incompleta. Pressione Esc para voltar ao menu principal.").into();
    }
}

fn subtitle(seconds: f32, language: Language) -> &'static str {
    Localization { language }.text(match seconds {
        t if (14.0..19.0).contains(&t) => "sleep",
        t if (20.0..25.0).contains(&t) => "work",
        t if (28.0..34.0).contains(&t) => "bills",
        t if (35.0..38.0).contains(&t) => "way",
        _ => "",
    })
}

fn lid_height(seconds: f32) -> f32 {
    let keys = [
        (0.0, 51.0),
        (1.0, 51.0),
        (3.2, 32.0),
        (3.4, 51.0),
        (3.7, 30.0),
        (5.9, 12.0),
        (6.1, 51.0),
        (6.5, 20.0),
        (9.0, 0.0),
    ];
    for pair in keys.windows(2) {
        let [(start, from), (end, to)] = pair else {
            unreachable!()
        };
        if seconds <= *end {
            let t = ((seconds - start) / (end - start)).clamp(0.0, 1.0);
            let ease = t * t * (3.0 - 2.0 * t);
            return from + (to - from) * ease;
        }
    }
    0.0
}

fn tick(
    mut commands: Commands,
    time: Res<Time<Real>>,
    assets: Res<AssetServer>,
    loaded: Res<OpeningAssets>,
    television: Res<tv::TvAssets>,
    keys: Res<ButtonInput<KeyCode>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut opening: ResMut<Opening>,
    mut next: ResMut<NextState<GameState>>,
    mut actors: Query<&mut AnimationPlayer, With<OpeningActor>>,
    eyes: Query<(), With<OpeningEye>>,
    statuses: Query<(&Text, &StatusText), Without<OpeningText>>,
    mut texts: Query<(&mut Text, &mut Node, Has<Subtitle>), With<OpeningText>>,
    mut blur: Query<(Entity, &mut DepthOfField), With<MainCamera>>,
    localization: Res<Localization>,
) {
    if opening.finished {
        return;
    }
    let focused = windows.single().is_ok_and(|window| window.focused);
    if !opening.ready {
        for id in [loaded.scene.id().untyped(), loaded.clip.id().untyped()] {
            if let Some((LoadState::Failed(error), _, _))
            | Some((_, _, RecursiveDependencyLoadState::Failed(error))) =
                assets.get_load_states(id)
            {
                opening.error = match localization.language {
                    Language::English => format!("Could not load opening: {error}\nPress Escape for title."),
                    Language::Portuguese => "Não foi possível carregar a abertura. Pressione Esc para voltar ao menu principal.".into(),
                };
            }
        }
        if let Some((message, _)) = statuses.iter().find(|(_, status)| status.failed) {
            opening.error = message.0.replace("for menu", "for title");
        }
        match tv::readiness(&assets, &television) {
            Err(error) => {
                opening.error = match localization.language {
                    Language::English => format!("{error}\nPress Escape for title."),
                    Language::Portuguese => "Não foi possível preparar a TV para a abertura. Pressione Esc para voltar ao menu principal.".into(),
                };
            }
            Ok(tv_ready)
                if tv_ready
                    && opening.spawn_ready
                    && opening.error.is_empty()
                    && !actors.is_empty()
                    && !eyes.is_empty()
                    && assets.is_loaded_with_dependencies(loaded.scene.id())
                    && assets.is_loaded_with_dependencies(loaded.clip.id()) =>
            {
                opening.ready = true;
                info!("Opening cutscene ready");
            }
            _ => {}
        }
    }
    if focused && !opening.ready && keys.just_pressed(KeyCode::Escape) {
        next.set(GameState::Title);
    }
    if opening.advance(time.delta_secs(), focused, keys.pressed(KeyCode::Enter)) {
        opening.finished = true;
        commands.trigger(OpeningFinished);
    }
    for mut actor in &mut actors {
        actor
            .play(loaded.animation)
            .pause()
            .seek_to(opening.elapsed);
    }
    for (entity, mut blur) in &mut blur {
        if opening.elapsed >= 9.0 {
            commands.entity(entity).remove::<DepthOfField>();
        } else {
            blur.max_circle_of_confusion_diameter = 12.0 * (1.0 - opening.elapsed / 9.0);
        }
    }
    for (mut text, mut node, is_subtitle) in &mut texts {
        let value = if is_subtitle {
            if !opening.error.is_empty() {
                opening.error.clone()
            } else if !opening.ready {
                localization.text("loading_opening").into()
            } else {
                subtitle(opening.elapsed, localization.language).into()
            }
        } else if opening.ready && opening.skip_hold > 0.0 {
            format!("{}  {}%", localization.text("skip"), (opening.skip_hold * 100.0).min(100.0) as u32)
        } else if opening.ready {
            localization.text("skip").into()
        } else {
            String::new()
        };
        node.display = if value.is_empty() {
            Display::None
        } else {
            Display::Flex
        };
        if text.0 != value {
            text.0 = value;
        }
    }
}

fn animate_eyelids(
    opening: Res<Opening>,
    assets: Res<OpeningAssets>,
    mut materials: ResMut<Assets<Eyelids>>,
    mut covers: Query<&mut Node, With<OpeningCover>>,
) {
    set_eyelids(opening.elapsed, &assets, &mut materials, &mut covers);
}

fn set_eyelids(
    seconds: f32,
    assets: &OpeningAssets,
    materials: &mut Assets<Eyelids>,
    covers: &mut Query<&mut Node, With<OpeningCover>>,
) {
    if let Some(mut material) = materials.get_mut(&assets.eyelids) {
        material.opening.x = 1.0 - lid_height(seconds) / 51.0;
    }
    for mut cover in covers {
        cover.display = if seconds <= 1.0 {
            Display::Flex
        } else {
            Display::None
        };
    }
}

pub(super) fn wake_pose(elapsed: f32, standing: Transform) -> Transform {
    // shortcut: these poses fit the shipped bedroom, recalibrate if the bed or nearby walls move.
    let lying = Transform::from_xyz(2.97, 1.17, 8.36).with_rotation(Quat::from_euler(
        EulerRot::YXZ,
        std::f32::consts::FRAC_PI_2,
        1.22,
        -0.08,
    ));
    let seated = Transform::from_xyz(2.18, 1.39, 7.83).with_rotation(Quat::from_euler(
        EulerRot::YXZ,
        0.28,
        -0.34,
        0.0,
    ));
    let (from, to, t) = if elapsed < 2.22 {
        (lying, seated, (elapsed - 0.85) / 1.37)
    } else {
        (seated, standing, (elapsed - 2.22) / (WAKE_DURATION - 2.22))
    };
    let t = t.clamp(0.0, 1.0);
    let t = t * t * (3.0 - 2.0 * t);
    Transform::from_translation(from.translation.lerp(to.translation, t))
        .with_rotation(from.rotation.slerp(to.rotation, t))
}

fn wake(
    mut commands: Commands,
    mut overlay: ResMut<GameplayOverlay>,
    mut playback: Local<WakePlayback>,
    time: Res<Time<Real>>,
    keys: Res<ButtonInput<KeyCode>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    localization: Res<Localization>,
    assets: Res<OpeningAssets>,
    mut materials: ResMut<Assets<Eyelids>>,
    mut covers: Query<&mut Node, With<OpeningCover>>,
    ui: Query<Entity, With<WakeUi>>,
    mut cameras: Query<&mut Transform, With<MainCamera>>,
) {
    let Some(OverlayKind::Wake { bedside }) = overlay.active else {
        *playback = WakePlayback::default();
        return;
    };
    let focused = windows.single().is_ok_and(|window| window.focused);
    if ui.is_empty() {
        playback.skip_armed = false;
    }
    if playback.advance(time.delta_secs(), focused, keys.pressed(KeyCode::Enter)) {
        *playback = WakePlayback::default();
        overlay.active = None;
        for entity in &ui {
            commands.entity(entity).despawn();
        }
        return;
    }
    let eye_seconds = playback.elapsed * 6.0;
    if ui.is_empty() {
        commands.spawn((
            WakeUi,
            DespawnOnExit(GameState::Playing),
            eyelid_overlay(assets.eyelids.clone(), eye_seconds <= 1.0),
        ));
        commands.spawn((
            WakeUi,
            DespawnOnExit(GameState::Playing),
            skip_hint(localization.text("skip_wake"), assets.font.clone()),
        ));
    }
    set_eyelids(eye_seconds, &assets, &mut materials, &mut covers);
    if bedside && let Ok(mut camera) = cameras.single_mut() {
        *camera = wake_pose(playback.elapsed, *camera);
    }
}

fn camera(
    eyes: Query<&Transform, (With<OpeningEye>, Without<MainCamera>)>,
    mut cameras: Query<&mut Transform, With<MainCamera>>,
) {
    if let Ok(eye) = eyes.single() {
        for mut camera in &mut cameras {
            *camera = *eye;
        }
    }
}

fn cleanup(mut commands: Commands, cameras: Query<Entity, With<MainCamera>>) {
    for camera in &cameras {
        commands.entity(camera).remove::<DepthOfField>();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn waking_ignores_held_report_enter_and_cleans_up_after_skip_or_playback() {
        let mut app = App::new();
        app.init_resource::<Time<Real>>()
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<Assets<Eyelids>>()
            .init_resource::<Localization>()
            .init_resource::<GameplayOverlay>()
            .add_systems(Update, wake);
        let eyelids = app
            .world_mut()
            .resource_mut::<Assets<Eyelids>>()
            .add(Eyelids { opening: Vec4::ONE });
        app.insert_resource(OpeningAssets {
            scene: default(),
            clip: default(),
            graph: default(),
            animation: AnimationNodeIndex::new(0),
            font: default(),
            eyelids: eyelids.clone(),
        });
        app.world_mut().spawn((
            Window {
                focused: true,
                ..default()
            },
            PrimaryWindow,
        ));
        app.world_mut()
            .resource_mut::<Time<Real>>()
            .advance_by(std::time::Duration::from_millis(100));
        let active = Some(OverlayKind::Wake { bedside: false });
        let enter = |app: &mut App, pressed| {
            let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
            if pressed {
                keys.press(KeyCode::Enter);
            } else {
                keys.release(KeyCode::Enter);
            }
        };
        let aperture = |app: &App| {
            app.world()
                .resource::<Assets<Eyelids>>()
                .get(&eyelids)
                .unwrap()
                .opening
                .x
        };
        app.world_mut().resource_mut::<GameplayOverlay>().active = active;
        enter(&mut app, true);
        app.update();
        assert_eq!(aperture(&app), 0.0);
        assert_eq!(app.world().resource::<GameplayOverlay>().active, active);
        enter(&mut app, false);
        app.update();
        let mut nodes = app.world_mut().query_filtered::<Entity, With<WakeUi>>();
        let paused_nodes: Vec<_> = nodes.iter(app.world()).collect();
        for entity in paused_nodes {
            app.world_mut().despawn(entity);
        }
        enter(&mut app, true);
        app.update();
        assert_eq!(app.world().resource::<GameplayOverlay>().active, active);
        enter(&mut app, false);
        app.update();
        enter(&mut app, true);
        app.update();
        assert_eq!(app.world().resource::<GameplayOverlay>().active, None);
        assert_eq!(nodes.iter(app.world()).count(), 0);

        enter(&mut app, false);
        app.world_mut().resource_mut::<GameplayOverlay>().active = active;
        for _ in 0..20 {
            app.update();
        }
        assert_eq!(aperture(&app), 1.0);
        for _ in 0..16 {
            app.update();
        }
        assert_eq!(app.world().resource::<GameplayOverlay>().active, None);
        assert_eq!(nodes.iter(app.world()).count(), 0);
    }

    #[test]
    fn curved_eyelids_follow_blinks_and_clear_the_startup_cover() {
        let mut app = App::new();
        app.init_resource::<Opening>()
            .init_resource::<Assets<Eyelids>>()
            .add_systems(Update, animate_eyelids);
        let eyelids = app
            .world_mut()
            .resource_mut::<Assets<Eyelids>>()
            .add(Eyelids { opening: Vec4::ONE });
        app.insert_resource(OpeningAssets {
            scene: Handle::default(),
            clip: Handle::default(),
            graph: Handle::default(),
            animation: AnimationNodeIndex::new(0),
            font: Handle::default(),
            eyelids: eyelids.clone(),
        });
        let cover = app.world_mut().spawn((OpeningCover, Node::default())).id();
        for (seconds, expected, cover_visible) in [
            (0.0, 0.0, true),
            (1.0, 0.0, true),
            (3.4, 0.0, false),
            (6.1, 0.0, false),
            (9.0, 1.0, false),
            (45.0, 1.0, false),
        ] {
            app.world_mut().resource_mut::<Opening>().elapsed = seconds;
            app.update();
            assert_eq!(
                app.world()
                    .resource::<Assets<Eyelids>>()
                    .get(&eyelids)
                    .unwrap()
                    .opening
                    .x,
                expected
            );
            assert_eq!(
                app.world().get::<Node>(cover).unwrap().display == Display::Flex,
                cover_visible
            );
        }
    }

    #[test]
    fn entering_opening_resumes_paused_physics_and_resets_previous_progress() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .insert_resource(Opening {
                spawn_ready: true,
                ready: true,
                elapsed: 32.0,
                skip_armed: true,
                skip_hold: 0.6,
                finished: true,
                error: "Previous opening failed".into(),
            })
            .insert_resource(OpeningAssets {
                scene: Handle::default(),
                clip: Handle::default(),
                graph: Handle::default(),
                animation: AnimationNodeIndex::new(0),
                font: Handle::default(),
                eyelids: Handle::default(),
            })
            .add_systems(Update, enter);
        app.world_mut().resource_mut::<Time<Virtual>>().pause();
        app.update();
        assert!(!app.world().resource::<Time<Virtual>>().is_paused());
        let opening = app.world().resource::<Opening>();
        assert_eq!(opening.elapsed, 0.0);
        assert_eq!(opening.skip_hold, 0.0);
        assert!(!opening.spawn_ready && !opening.ready && !opening.skip_armed && !opening.finished);
        assert!(opening.error.is_empty());
    }

    #[test]
    fn opening_waits_for_readiness_focus_and_a_new_skip_hold() {
        let mut opening = Opening::default();
        assert!(!opening.advance(1.0, true, true));
        assert_eq!(opening.elapsed, 0.0);
        opening.ready = true;
        for _ in 0..20 {
            assert!(!opening.advance(0.1, true, true));
        }
        assert_eq!(opening.skip_hold, 0.0);
        opening.advance(0.1, true, false);
        for _ in 0..5 {
            assert!(!opening.advance(0.1, true, true));
        }
        let elapsed = opening.elapsed;
        assert!(!opening.advance(30.0, false, true));
        assert_eq!(opening.elapsed, elapsed);
        assert_eq!(opening.skip_hold, 0.0);
        assert!(!opening.advance(0.1, true, true));
        opening.advance(0.1, true, false);
        let mut completed = false;
        for _ in 0..11 {
            completed |= opening.advance(0.1, true, true);
        }
        assert!(completed);
        assert_eq!(opening.elapsed, DURATION);
        let mut natural = Opening {
            ready: true,
            elapsed: 44.95,
            ..default()
        };
        assert!(natural.advance(0.1, true, false));
        assert_eq!(natural.elapsed, opening.elapsed);
    }

    #[test]
    fn waking_and_dialogue_finish_before_handoff() {
        assert_eq!(lid_height(0.0), 51.0);
        assert_eq!(lid_height(3.4), 51.0);
        assert_eq!(lid_height(6.1), 51.0);
        assert_eq!(lid_height(9.0), 0.0);
        for t in [14.0, 20.0, 28.0, 35.0] {
            assert!(!subtitle(t, Language::English).is_empty());
        }
        for t in [0.0, 19.0, 25.0, 34.0, 38.0, DURATION] {
            assert!(subtitle(t, Language::English).is_empty());
        }
        for frame in 0..=450 {
            assert!((0.0..=51.0).contains(&lid_height(frame as f32 / 10.0)));
        }
    }
}
