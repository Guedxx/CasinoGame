//! One logical mouse position for window UI and the phone while the OS pointer stays locked.
use bevy::{
    camera::RenderTarget,
    input::mouse::{AccumulatedMouseMotion, MouseWheel},
    picking::{
        PickingSystems,
        input::PointerInputSettings,
        pointer::{Location, PointerAction, PointerId, PointerInput},
    },
    prelude::*,
    ui::{FocusPolicy, UiSystems},
    window::{CursorGrabMode, CursorOptions, PrimaryWindow, WindowRef},
};

use super::{
    camera::MainCamera,
    phone::{PhoneCamera, PhoneInput, PhoneView},
    player::{Player, PlayerInput},
    routine::{GameplayOverlay, OverlayInput},
};
use crate::state::GameState;

pub struct GameCursorPlugin;

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct CursorInput;

#[derive(Resource, Default)]
pub(crate) struct GameCursor {
    pub(crate) captured: bool,
    pub(crate) position: Option<Vec2>,
    pub(crate) suppress_look: bool,
    ui: bool,
    previous_pointer: Option<Vec2>,
    window_position: Option<Option<Vec2>>,
}

#[derive(Component)]
struct DigitalCursor;

impl Plugin for GameCursorPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GameCursor>()
            .insert_resource(PointerInputSettings {
                is_mouse_enabled: false,
                ..default()
            })
            .add_systems(Startup, setup)
            .add_systems(
                PreUpdate,
                capture
                    .after(bevy::input::InputSystems)
                    .before(OverlayInput),
            )
            .add_systems(
                PreUpdate,
                route
                    .in_set(CursorInput)
                    .after(PhoneInput)
                    .after(OverlayInput)
                    .before(PlayerInput)
                    .before(UiSystems::Focus)
                    .before(PickingSystems::ProcessInput),
            )
            .add_systems(
                PreUpdate,
                restore_window.after(UiSystems::Focus).after(CursorInput),
            )
            .add_systems(Update, draw.after(PhoneView));
    }
}

pub(crate) fn pointer_locked(cursor: &CursorOptions) -> bool {
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

fn capture(
    state: Res<State<GameState>>,
    overlay: Res<GameplayOverlay>,
    mut next: ResMut<NextState<GameState>>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut buttons: ResMut<ButtonInput<MouseButton>>,
    mut windows: Query<(&Window, &mut CursorOptions), With<PrimaryWindow>>,
    players: Query<(), With<Player>>,
    mut mouse: ResMut<GameCursor>,
) {
    let Ok((window, mut cursor)) = windows.single_mut() else {
        return;
    };
    if *state.get() == GameState::Intro {
        cursor.grab_mode = CursorGrabMode::None;
        cursor.visible = !window.focused;
        mouse.captured = false;
        mouse.position = None;
        mouse.suppress_look = true;
        return;
    }
    let locked = window.focused && pointer_locked(&cursor);
    let playing = *state.get() == GameState::Playing;
    let browser_escape = cfg!(target_arch = "wasm32")
        && (locked || mouse.captured)
        && keys.just_pressed(KeyCode::Escape);
    #[cfg(target_arch = "wasm32")]
    let browser_escape = browser_escape
        && !web_sys::window()
            .and_then(|window| window.document())
            .and_then(|document| document.document_element())
            .is_some_and(|root| root.has_attribute("data-game-escape-lock"));
    let lost = !window.focused || (mouse.captured && !locked) || browser_escape;
    let title_escape = *state.get() == GameState::Title && keys.just_pressed(KeyCode::Escape);
    let menu_escape =
        *state.get() == GameState::Menu && locked && keys.just_pressed(KeyCode::Escape);
    if lost || title_escape || menu_escape {
        cursor.grab_mode = CursorGrabMode::None;
        cursor.visible = true;
        mouse.position = None;
        if playing && lost {
            next.set(GameState::Menu);
        }
        if lost || menu_escape {
            if lost {
                keys.reset_all();
            } else {
                keys.clear_just_pressed(KeyCode::Escape);
            }
            buttons.clear();
            buttons.release_all();
        }
    } else if playing {
        if keys.just_pressed(KeyCode::KeyP)
            || (keys.just_pressed(KeyCode::Escape) && !overlay.blocks_world())
        {
            next.set(GameState::Menu);
            keys.clear_just_pressed(KeyCode::Escape);
        } else if !locked
            && !overlay.blocks_world()
            && buttons.just_pressed(MouseButton::Left)
            && !players.is_empty()
        {
            cursor.grab_mode = CursorGrabMode::Locked;
            cursor.visible = false;
        }
    }
    let captured = window.focused && pointer_locked(&cursor);
    mouse.suppress_look = captured != mouse.captured;
    mouse.captured = captured;
    cursor.visible = !captured;
}

fn clamp_position(position: Vec2, size: Vec2) -> Vec2 {
    position.clamp(Vec2::ZERO, (size - Vec2::ONE).max(Vec2::ZERO))
}

fn route(
    state: Res<State<GameState>>,
    next: Res<NextState<GameState>>,
    overlay: Res<GameplayOverlay>,
    motion: Res<AccumulatedMouseMotion>,
    buttons: Res<ButtonInput<MouseButton>>,
    mut windows: Query<(Entity, &mut Window), With<PrimaryWindow>>,
    mut mouse: ResMut<GameCursor>,
    mut pointers: MessageWriter<PointerInput>,
    mut wheel: MessageReader<MouseWheel>,
) {
    let Ok((entity, mut window)) = windows.single_mut() else {
        return;
    };
    let pending_pause = matches!(*next, NextState::Pending(s) | NextState::PendingIfNeq(s) if s != GameState::Playing);
    let ui = *state.get() != GameState::Playing || overlay.blocks_world() || pending_pause;
    mouse.suppress_look |= ui != mouse.ui || overlay.escape_consumed;
    let size = Vec2::new(window.width(), window.height());
    if mouse.captured && ui {
        let mut position = mouse.position.unwrap_or(size * 0.5);
        if mouse.ui && !mouse.suppress_look && motion.delta.is_finite() {
            // Winit emits physical relative motion; UI and phone rays use logical window pixels.
            position += motion.delta / window.scale_factor();
        }
        mouse.position = Some(clamp_position(position, size));
        mouse.window_position = Some(window.physical_cursor_position());
        window
            .bypass_change_detection()
            .set_cursor_position(mouse.position);
    }
    let position = (window.focused && *state.get() != GameState::Intro)
        .then(|| {
            if mouse.captured {
                ui.then_some(mouse.position).flatten()
            } else {
                window.cursor_position()
            }
        })
        .flatten();
    let target = RenderTarget::Window(WindowRef::Entity(entity))
        .normalize(Some(entity))
        .unwrap();
    if let Some(position) = position {
        let location = Location { target, position };
        pointers.write(PointerInput::new(
            PointerId::Mouse,
            location.clone(),
            PointerAction::Move {
                delta: position - mouse.previous_pointer.unwrap_or(position),
            },
        ));
        for (button, pointer) in [
            (MouseButton::Left, PointerButton::Primary),
            (MouseButton::Right, PointerButton::Secondary),
            (MouseButton::Middle, PointerButton::Middle),
        ] {
            if buttons.just_pressed(button) {
                pointers.write(PointerInput::new(
                    PointerId::Mouse,
                    location.clone(),
                    PointerAction::Press(pointer),
                ));
            }
            if buttons.just_released(button) {
                pointers.write(PointerInput::new(
                    PointerId::Mouse,
                    location.clone(),
                    PointerAction::Release(pointer),
                ));
            }
        }
        for event in wheel.read() {
            pointers.write(PointerInput::new(
                PointerId::Mouse,
                location.clone(),
                PointerAction::Scroll {
                    x: event.x,
                    y: event.y,
                    unit: event.unit,
                    phase: event.phase,
                },
            ));
        }
    } else if let Some(position) = mouse.previous_pointer {
        let location = Location { target, position };
        pointers.write(PointerInput::new(
            PointerId::Mouse,
            location.clone(),
            PointerAction::Cancel,
        ));
        for button in [
            PointerButton::Primary,
            PointerButton::Secondary,
            PointerButton::Middle,
        ] {
            pointers.write(PointerInput::new(
                PointerId::Mouse,
                location.clone(),
                PointerAction::Release(button),
            ));
        }
        pointers.write(PointerInput::new(
            PointerId::Mouse,
            Location {
                position: Vec2::splat(-1000.0),
                ..location
            },
            PointerAction::Move { delta: Vec2::ZERO },
        ));
    }
    wheel.clear();
    mouse.previous_pointer = position;
    mouse.ui = ui;
}

fn restore_window(
    mut mouse: ResMut<GameCursor>,
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
) {
    if let Some(position) = mouse.window_position.take()
        && let Ok(mut window) = windows.single_mut()
    {
        // Only ui_focus_system needs the substituted position; never warp the native OS cursor.
        window
            .bypass_change_detection()
            .set_physical_cursor_position(position.map(Vec2::as_dvec2));
    }
}

fn setup(mut commands: Commands) {
    commands.spawn((
        Name::new("Digital mouse"),
        DigitalCursor,
        Node {
            position_type: PositionType::Absolute,
            width: px(15),
            height: px(15),
            display: Display::None,
            ..default()
        },
        GlobalZIndex(i32::MAX),
        Pickable::IGNORE,
        FocusPolicy::Pass,
        children![
            (
                Node {
                    position_type: PositionType::Absolute,
                    left: px(6),
                    width: px(3),
                    height: px(15),
                    border: UiRect::all(px(1)),
                    ..default()
                },
                BackgroundColor(Color::WHITE),
                BorderColor::all(Color::BLACK),
                Pickable::IGNORE,
                FocusPolicy::Pass
            ),
            (
                Node {
                    position_type: PositionType::Absolute,
                    top: px(6),
                    width: px(15),
                    height: px(3),
                    border: UiRect::all(px(1)),
                    ..default()
                },
                BackgroundColor(Color::WHITE),
                BorderColor::all(Color::BLACK),
                Pickable::IGNORE,
                FocusPolicy::Pass
            ),
        ],
    ));
}

fn draw(
    mut commands: Commands,
    mouse: Res<GameCursor>,
    cameras: Query<(Entity, &Camera, Has<PhoneCamera>), Or<(With<MainCamera>, With<PhoneCamera>)>>,
    mut cursors: Query<(Entity, &mut Node), With<DigitalCursor>>,
) {
    for (entity, mut node) in &mut cursors {
        node.display = if mouse.captured && mouse.ui {
            Display::Flex
        } else {
            Display::None
        };
        if let Some(position) = mouse.position {
            node.left = px(position.x - 7.0);
            node.top = px(position.y - 7.0);
        }
        if let Some((camera, _, _)) = cameras
            .iter()
            .filter(|(_, camera, _)| camera.is_active)
            .max_by_key(|(_, _, phone)| *phone)
        {
            commands.entity(entity).insert(UiTargetCamera(camera));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::routine::OverlayKind;
    use super::*;

    fn app() -> (App, Entity) {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .insert_resource(State::new(GameState::Playing))
            .init_resource::<NextState<GameState>>()
            .init_resource::<GameplayOverlay>()
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<ButtonInput<MouseButton>>()
            .init_resource::<AccumulatedMouseMotion>()
            .add_message::<PointerInput>()
            .add_message::<MouseWheel>()
            .add_plugins(GameCursorPlugin);
        let mut window = Window {
            focused: true,
            ..default()
        };
        window.resolution.set_scale_factor(2.0);
        window.set_cursor_position(Some(Vec2::new(20.0, 30.0)));
        let entity = app
            .world_mut()
            .spawn((
                window,
                PrimaryWindow,
                CursorOptions {
                    grab_mode: CursorGrabMode::Locked,
                    visible: false,
                    ..default()
                },
            ))
            .id();
        app.world_mut().spawn(Player::default());
        app.update();
        (app, entity)
    }

    #[test]
    fn opening_releases_capture_hides_cursor_and_does_not_route_clicks_or_pause() {
        let (mut app, window) = app();
        app.insert_resource(State::new(GameState::Intro));
        app.world_mut().resource_mut::<ButtonInput<MouseButton>>().press(MouseButton::Left);
        app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::KeyP);
        app.update();
        let cursor = app.world().get::<CursorOptions>(window).unwrap();
        assert_eq!(cursor.grab_mode, CursorGrabMode::None);
        assert!(!cursor.visible);
        let mouse = app.world().resource::<GameCursor>();
        assert!(!mouse.captured && mouse.position.is_none() && mouse.previous_pointer.is_none());
        assert!(matches!(app.world().resource::<NextState<GameState>>(), NextState::Unchanged));
        app.world_mut().get_mut::<Window>(window).unwrap().focused = false;
        app.update();
        assert!(app.world().get::<CursorOptions>(window).unwrap().visible);
    }

    #[test]
    fn digital_mouse_uses_relative_motion_once_clamps_resizes_and_never_warps_os_cursor() {
        let (mut app, window) = app();
        app.world_mut().resource_mut::<GameplayOverlay>().active = Some(OverlayKind::Phone);
        app.world_mut()
            .resource_mut::<AccumulatedMouseMotion>()
            .delta = Vec2::splat(600.0);
        app.update();
        let center = app.world().resource::<GameCursor>().position.unwrap();
        app.world_mut()
            .resource_mut::<AccumulatedMouseMotion>()
            .delta = Vec2::new(40.0, -60.0);
        app.update();
        assert_eq!(
            app.world().resource::<GameCursor>().position,
            Some(center + Vec2::new(20.0, -30.0))
        );
        assert_eq!(
            app.world().get::<Window>(window).unwrap().cursor_position(),
            Some(Vec2::new(20.0, 30.0))
        );
        app.world_mut()
            .resource_mut::<AccumulatedMouseMotion>()
            .delta = Vec2::new(f32::MAX, -f32::MAX);
        app.update();
        let size = app.world().get::<Window>(window).unwrap().size();
        assert_eq!(
            app.world().resource::<GameCursor>().position,
            Some(Vec2::new(size.x - 1.0, 0.0))
        );
        app.world_mut()
            .get_mut::<Window>(window)
            .unwrap()
            .resolution
            .set(120.0, 80.0);
        app.world_mut()
            .resource_mut::<AccumulatedMouseMotion>()
            .delta = Vec2::ZERO;
        app.update();
        assert_eq!(
            app.world().resource::<GameCursor>().position,
            Some(Vec2::new(119.0, 0.0))
        );
        let cursor = app
            .world_mut()
            .query_filtered::<Entity, With<DigitalCursor>>()
            .single(app.world())
            .unwrap();
        assert_eq!(app.world().get::<Pickable>(cursor), Some(&Pickable::IGNORE));
        assert_eq!(app.world().get::<GlobalZIndex>(cursor).unwrap().0, i32::MAX);
    }

    #[test]
    fn standard_mouse_pointer_uses_digital_position_and_cancels_on_close_without_held_repeats() {
        let (mut app, _) = app();
        app.world_mut().resource_mut::<GameplayOverlay>().active = Some(OverlayKind::Context);
        app.update();
        app.world_mut()
            .resource_mut::<Messages<PointerInput>>()
            .clear();
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        app.update();
        let position = app.world().resource::<GameCursor>().position.unwrap();
        let events = app.world().resource::<Messages<PointerInput>>();
        let press = events
            .iter_current_update_messages()
            .find(|event| matches!(event.action, PointerAction::Press(PointerButton::Primary)))
            .unwrap();
        assert_eq!(press.pointer_id, PointerId::Mouse);
        assert_eq!(press.location.position, position);
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .clear();
        app.world_mut()
            .resource_mut::<Messages<PointerInput>>()
            .clear();
        app.update();
        assert!(
            !app.world()
                .resource::<Messages<PointerInput>>()
                .iter_current_update_messages()
                .any(|event| matches!(event.action, PointerAction::Press(_)))
        );
        app.world_mut().resource_mut::<GameplayOverlay>().active = None;
        app.update();
        assert!(
            app.world()
                .resource::<Messages<PointerInput>>()
                .iter_current_update_messages()
                .any(|event| matches!(event.action, PointerAction::Cancel))
        );
        assert!(app.world().resource::<GameCursor>().suppress_look);
        app.world_mut().resource_mut::<GameplayOverlay>().active = Some(OverlayKind::Phone);
        app.update();
        assert_eq!(
            app.world().resource::<GameCursor>().position,
            Some(position)
        );
        assert!(
            !app.world()
                .resource::<Messages<PointerInput>>()
                .iter_current_update_messages()
                .any(|event| matches!(event.action, PointerAction::Press(_)))
        );
    }

    #[test]
    fn pause_settings_title_keep_capture_but_real_blur_and_lock_loss_pause_with_overlay_intact() {
        for blur in [false, true] {
            let (mut app, window) = app();
            app.world_mut().resource_mut::<GameplayOverlay>().active = Some(OverlayKind::Phone);
            app.world_mut()
                .resource_mut::<ButtonInput<KeyCode>>()
                .press(KeyCode::KeyP);
            app.update();
            assert!(matches!(
                *app.world().resource::<NextState<GameState>>(),
                NextState::Pending(GameState::Menu)
            ));
            assert!(app.world().resource::<GameCursor>().captured);
            app.world_mut()
                .resource_mut::<ButtonInput<KeyCode>>()
                .reset_all();
            for state in [
                GameState::Menu,
                GameState::Settings,
                GameState::Menu,
                GameState::Title,
                GameState::Playing,
            ] {
                app.insert_resource(State::new(state));
                app.insert_resource(NextState::<GameState>::Unchanged);
                app.update();
                assert!(app.world().resource::<GameCursor>().captured);
                assert_eq!(
                    app.world().get::<CursorOptions>(window).unwrap().grab_mode,
                    CursorGrabMode::Locked
                );
            }
            if blur {
                app.world_mut().get_mut::<Window>(window).unwrap().focused = false;
            } else {
                app.world_mut()
                    .get_mut::<CursorOptions>(window)
                    .unwrap()
                    .grab_mode = CursorGrabMode::None;
            }
            app.update();
            assert!(!app.world().resource::<GameCursor>().captured);
            assert!(matches!(
                *app.world().resource::<NextState<GameState>>(),
                NextState::Pending(GameState::Menu)
            ));
            assert_eq!(
                app.world().resource::<GameplayOverlay>().active,
                Some(OverlayKind::Phone)
            );
        }
    }

    #[test]
    fn escape_opens_pause_with_capture_and_second_escape_releases_without_resuming() {
        let (mut app, window) = app();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Escape);
        app.update();
        assert!(app.world().resource::<GameCursor>().captured);
        assert!(
            !app.world()
                .resource::<ButtonInput<KeyCode>>()
                .just_pressed(KeyCode::Escape)
        );
        assert!(matches!(
            *app.world().resource::<NextState<GameState>>(),
            NextState::Pending(GameState::Menu)
        ));
        app.insert_resource(State::new(GameState::Menu));
        app.insert_resource(NextState::<GameState>::Unchanged);
        app.update();
        assert!(app.world().resource::<GameCursor>().captured);
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        app.update();
        assert!(app.world().resource::<GameCursor>().captured);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .release(KeyCode::Escape);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Escape);
        app.update();
        assert!(!app.world().resource::<GameCursor>().captured);
        assert_eq!(
            app.world().get::<CursorOptions>(window).unwrap().grab_mode,
            CursorGrabMode::None
        );
        assert!(matches!(
            *app.world().resource::<NextState<GameState>>(),
            NextState::Unchanged
        ));
        assert!(
            !app.world()
                .resource::<ButtonInput<KeyCode>>()
                .just_pressed(KeyCode::Escape)
        );
        assert!(
            !app.world()
                .resource::<ButtonInput<MouseButton>>()
                .pressed(MouseButton::Left)
        );
    }

    #[test]
    fn captured_title_uses_digital_mouse_and_escape_preserves_menu_cancellation_key() {
        let (mut app, window) = app();
        app.insert_resource(State::new(GameState::Title));
        app.update();
        assert!(app.world().resource::<GameCursor>().captured);
        let cursor = app
            .world_mut()
            .query_filtered::<Entity, With<DigitalCursor>>()
            .single(app.world())
            .unwrap();
        assert_eq!(
            app.world().get::<Node>(cursor).unwrap().display,
            Display::Flex
        );
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Escape);
        app.update();
        assert!(!app.world().resource::<GameCursor>().captured);
        assert!(
            app.world()
                .resource::<ButtonInput<KeyCode>>()
                .just_pressed(KeyCode::Escape)
        );
        assert_eq!(
            app.world().get::<CursorOptions>(window).unwrap().grab_mode,
            CursorGrabMode::None
        );
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .reset_all();
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        app.update();
        assert!(!app.world().resource::<GameCursor>().captured);
        assert_eq!(
            app.world().get::<Node>(cursor).unwrap().display,
            Display::None
        );
    }

    #[test]
    fn focus_loss_releases_held_ui_and_the_first_fresh_click_still_dispatches() {
        use bevy::{
            input::touch::Touches,
            reflect::structs::Struct,
            ui::{ComputedUiTargetCamera, UiGlobalTransform, UiStack, ui_focus_system},
        };
        #[derive(Resource, Default)]
        struct Clicks(usize);
        let (mut app, window) = app();
        app.insert_resource(State::new(GameState::Settings))
            .init_resource::<Touches>()
            .init_resource::<UiStack>()
            .init_resource::<Clicks>()
            .add_systems(PreUpdate, ui_focus_system.in_set(UiSystems::Focus))
            .add_systems(
                Update,
                |buttons: Query<&Interaction, Changed<Interaction>>, mut clicks: ResMut<Clicks>| {
                    clicks.0 += buttons
                        .iter()
                        .filter(|interaction| **interaction == Interaction::Pressed)
                        .count();
                },
            );
        let camera = app
            .world_mut()
            .spawn((
                Camera::default(),
                RenderTarget::Window(WindowRef::Entity(window)),
            ))
            .id();
        let mut target = ComputedUiTargetCamera::default();
        *target
            .field_mut("camera")
            .unwrap()
            .try_downcast_mut::<Entity>()
            .unwrap() = camera;
        let button = app
            .world_mut()
            .spawn((
                Button,
                Node::default(),
                ComputedNode {
                    size: Vec2::splat(2000.0),
                    ..default()
                },
                UiGlobalTransform::from_xy(300.0, 300.0),
                InheritedVisibility::VISIBLE,
                target,
            ))
            .id();
        {
            let mut stack = app.world_mut().resource_mut::<UiStack>();
            stack.uinodes = vec![button];
            stack.partition = vec![0..1];
        }
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        app.update();
        assert_eq!(app.world().resource::<Clicks>().0, 1);
        app.world_mut().get_mut::<Window>(window).unwrap().focused = false;
        app.update();
        assert_ne!(
            *app.world().get::<Interaction>(button).unwrap(),
            Interaction::Pressed
        );
        assert_eq!(app.world().resource::<Clicks>().0, 1);
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .release(MouseButton::Left);
        app.update();
        app.world_mut().get_mut::<Window>(window).unwrap().focused = true;
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        app.update();
        assert_eq!(app.world().resource::<Clicks>().0, 2);
    }
}
