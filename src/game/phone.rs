//! Alex's physical phone: the apartment stays visible around a readable, native UI screen.
use bevy::{
    asset::{RenderAssetUsages, uuid::Uuid},
    camera::{RenderTarget, visibility::RenderLayers},
    core_pipeline::tonemapping::{DebandDither, Tonemapping},
    image::{ImageLoaderSettings, ImageSampler},
    input::{
        ButtonState,
        keyboard::{Key, KeyboardInput},
        mouse::{MouseScrollUnit, MouseWheel},
    },
    light::NotShadowCaster,
    light::ShadowFilteringMethod,
    picking::{
        PickingSystems,
        pointer::{Location, PointerAction, PointerId, PointerInput},
    },
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat, TextureUsages},
    ui_render::UiAntiAlias,
    window::PrimaryWindow,
};

use super::{
    audio::Sound,
    campaign::*,
    campaign_save::{CampaignReplaced, CampaignStore},
    cursor::{CursorInput, GameCursor},
    player::{Player, PlayerInput},
    routine::{GameplayOverlay, OverlayInput, OverlayKind},
};
use crate::{localization::{Localization, format_text}, state::GameState};

const SCREEN_SIZE: Vec2 = Vec2::new(420.0, 735.0);
const SCREEN_METERS: Vec2 = Vec2::new(0.300, 0.525);
const PHONE_LAYER: usize = 1;
const PHONE_POINTER: PointerId = PointerId::Custom(Uuid::from_u128(0x416c657850686f6e65));
const INK: Color = Color::srgb(0.96, 0.98, 1.0);
const MUTED: Color = Color::srgb(0.67, 0.79, 0.84);
const ACCENT: Color = Color::srgb(0.32, 0.82, 0.85);

mod betting;
mod care;
mod finance;
mod launcher;
mod life;
mod ui;

pub struct PhonePlugin;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum PhonePage {
    #[default]
    Home,
    Betting,
    BettingHistory,
    Roulette,
    Slots,
    Football,
    History,
    Household,
    SaleOffers,
    ConfirmSale(HouseholdItem),
    Contacts,
    ContactThread(Contact),
    Support,
    DailyCap,
}
impl PhonePage {
    pub(crate) fn is_betting(self) -> bool {
        matches!(
            self,
            Self::Betting | Self::BettingHistory | Self::Roulette | Self::Slots | Self::Football
        )
    }
    fn is_game(self) -> bool {
        matches!(self, Self::Roulette | Self::Slots | Self::Football)
    }
    fn back(self) -> Self {
        if self == Self::SaleOffers {
            Self::Household
        } else if matches!(self, Self::ConfirmSale(_)) {
            Self::SaleOffers
        } else if matches!(self, Self::DailyCap | Self::Household) {
            Self::Support
        } else if matches!(self, Self::ContactThread(_)) {
            Self::Contacts
        } else if matches!(
            self,
            Self::Roulette | Self::Slots | Self::Football | Self::BettingHistory
        ) {
            Self::Betting
        } else {
            Self::Home
        }
    }
}

/// Feature 4 consumes this one exit hook to close the shared betting session.
/// Pausing/focus loss never emits it; deliberately leaving the app or phone does.
#[derive(Message)]
pub(crate) struct PhoneAppClosed;

#[derive(Resource)]
pub(crate) struct Phone {
    pub(crate) page: PhonePage,
    pub(crate) feedback: String,
    opened: bool,
    raise: f32,
    pointer: Option<Vec2>,
    pending: Option<(PhonePage, PhoneCommand)>,
    stake_cents: i64,
    stake_entry: Option<String>,
    cap_cents: i64,
    cap_entry: Option<String>,
    settled_round: Option<BetRound>,
    animation_left: f32,
    selected_bet: Option<BetChoice>,
    bet_info_open: bool,
}
impl Default for Phone {
    fn default() -> Self {
        Self {
            page: PhonePage::Home,
            feedback: String::new(),
            opened: false,
            raise: 0.0,
            pointer: None,
            pending: None,
            stake_cents: 100,
            stake_entry: None,
            cap_cents: 0,
            cap_entry: None,
            settled_round: None,
            animation_left: 0.0,
            selected_bet: None,
            bet_info_open: false,
        }
    }
}
#[derive(Component)]
pub(crate) struct PhoneCamera;
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct PhoneInput;
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct PhoneView;
#[derive(Component)]
struct TextureCamera;
#[derive(Component)]
struct PhoneRoot;
#[derive(Component)]
struct PhoneModel;
#[derive(Component)]
struct PhoneMesh;
#[derive(Component)]
struct Screen;
#[derive(Component)]
struct PhoneUi;
#[derive(Component)]
struct PhoneScroll;
#[derive(Component, Clone, Copy)]
struct PhoneButton(PhonePage, PhoneCommand);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PhoneCommand {
    Page(PhonePage),
    Close,
    Apply(Action),
    RetrySave,
    Stake(i64),
    EditStake,
    ConfirmStake,
    Cap(i64),
    EditCap,
    ConfirmCap,
    Skip,
    SelectBet(BetChoice),
    ToggleBetInfo,
}
#[derive(Resource)]
struct PhoneAssets {
    camera: Entity,
    target: Handle<Image>,
    font: Handle<Font>,
}

impl Plugin for PhonePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Phone>()
            .init_resource::<launcher::Skin>()
            .init_resource::<ui::Art>()
            .add_message::<PhoneAppClosed>()
            .add_systems(Startup, setup)
            .add_systems(
                PreUpdate,
                input
                    .in_set(PhoneInput)
                    .after(OverlayInput)
                    .before(PlayerInput)
                    .run_if(in_state(GameState::Playing)),
            )
            .add_systems(
                PreUpdate,
                drive_pointer
                    .after(CursorInput)
                    .before(PickingSystems::ProcessInput)
                    .run_if(in_state(GameState::Playing)),
            )
            .add_systems(
                Update,
                (
                    tag_model,
                    dispatch,
                    end_session,
                    tick_game,
                    scroll,
                    present,
                    betting::animate,
                    animate,
                )
                    .chain()
                    .in_set(PhoneView)
                    .after(crate::graphics::ApplyGraphics),
            )
            .add_systems(OnEnter(GameState::Playing), restore_session)
            .add_observer(reset);
    }
}

fn setup(
    mut commands: Commands,
    assets: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    let mut image = Image::new_fill(
        Extent3d {
            width: SCREEN_SIZE.x as u32,
            height: SCREEN_SIZE.y as u32,
            ..default()
        },
        TextureDimension::D2,
        &[0, 0, 0, 255],
        TextureFormat::Bgra8UnormSrgb,
        RenderAssetUsages::default(),
    );
    image.texture_descriptor.usage =
        TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST | TextureUsages::RENDER_ATTACHMENT;
    image.sampler = ImageSampler::nearest();
    let target = images.add(image);
    let camera = commands
        .spawn((
            TextureCamera,
            Camera2d,
            Msaa::Off,
            Tonemapping::None,
            DebandDither::Disabled,
            UiAntiAlias::Off,
            Camera {
                order: -1,
                is_active: false,
                ..default()
            },
            RenderTarget::Image(target.clone().into()),
        ))
        .id();
    commands.spawn((
        PhoneCamera,
        Camera3d::default(),
        Tonemapping::None,
        DebandDither::Disabled,
        UiAntiAlias::Off,
        ShadowFilteringMethod::Hardware2x2,
        Camera {
            order: 1,
            is_active: false,
            clear_color: ClearColorConfig::None,
            ..default()
        },
        Projection::Perspective(PerspectiveProjection {
            fov: 50.0_f32.to_radians(),
            near: 0.01,
            far: 3.0,
            ..default()
        }),
        RenderLayers::layer(PHONE_LAYER),
        Msaa::Off,
        Transform::IDENTITY,
    ));
    commands
        .spawn((
            Name::new("Alex holding phone"),
            PhoneRoot,
            Transform::default(),
            Visibility::Hidden,
        ))
        .with_children(|parent| {
            parent.spawn((
                PhoneModel,
                WorldAssetRoot(
                    assets.load(GltfAssetLabel::Scene(0).from_asset("models/phone_hand.glb")),
                ),
            ));
            parent.spawn((
                Screen,
                Mesh3d(meshes.add(Rectangle::new(SCREEN_METERS.x, SCREEN_METERS.y))),
                MeshMaterial3d(materials.add(StandardMaterial {
                    base_color_texture: Some(target.clone()),
                    unlit: true,
                    ..default()
                })),
                Transform::from_xyz(0.0, 0.0, 0.023),
                RenderLayers::layer(PHONE_LAYER),
                NotShadowCaster,
                Pickable::IGNORE,
            ));
        });
    commands.spawn(PHONE_POINTER);
    commands.insert_resource(PhoneAssets {
        camera,
        target,
        font: assets.load("fonts/LiberationSans-Bold.ttf"),
    });
}

fn tag_model(
    mut commands: Commands,
    models: Query<Entity, With<PhoneModel>>,
    children: Query<&Children>,
    meshes: Query<&MeshMaterial3d<StandardMaterial>, (With<Mesh3d>, Without<PhoneMesh>)>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for root in &models {
        for entity in children.iter_descendants(root) {
            if let Ok(material) = meshes.get(entity) {
                if let Some(mut material) = materials.get_mut(material) {
                    material.unlit = true;
                }
                commands.entity(entity).insert((
                    PhoneMesh,
                    RenderLayers::layer(PHONE_LAYER),
                    NotShadowCaster,
                    Pickable::IGNORE,
                ));
            }
        }
    }
}

fn may_operate(
    state: &State<GameState>,
    next: &NextState<GameState>,
    window: &Window,
    overlay: &GameplayOverlay,
    phone: &Phone,
) -> bool {
    *state.get() == GameState::Playing
        && window.focused
        && !overlay.escape_consumed
        && overlay.active == Some(OverlayKind::Phone)
        && !overlay.campaign_modal
        && phone.opened
        && !matches!(*next, NextState::Pending(s) | NextState::PendingIfNeq(s) if s != GameState::Playing)
}

fn leave_app(phone: &mut Phone, exits: &mut MessageWriter<PhoneAppClosed>) {
    phone.pending = None;
    phone.animation_left = 0.0;
    phone.stake_entry = None;
    phone.cap_entry = None;
    phone.bet_info_open = false;
    exits.write(PhoneAppClosed);
}

fn input(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    state: Res<State<GameState>>,
    next: Res<NextState<GameState>>,
    store: Res<CampaignStore>,
    windows: Query<&Window, With<PrimaryWindow>>,
    players: Query<(), With<Player>>,
    mut overlay: ResMut<GameplayOverlay>,
    mut phone: ResMut<Phone>,
    mut exits: MessageWriter<PhoneAppClosed>,
) {
    let Ok(window) = windows.single() else { return };
    if overlay.in_dialogue() {
        phone.pending = None;
        return;
    }
    if phone.opened && overlay.active != Some(OverlayKind::Phone) {
        leave_app(&mut phone, &mut exits);
        phone.opened = false;
        commands.trigger(Sound::Back);
    }
    if !window.focused
        || overlay.escape_consumed
        || overlay.campaign_modal
        || players.is_empty()
        || !store
            .campaign()
            .is_some_and(|c| c.stage == CampaignStage::Playing)
        || matches!(*next, NextState::Pending(s) | NextState::PendingIfNeq(s) if s != GameState::Playing)
    {
        phone.pending = None;
        return;
    }
    if keys.just_pressed(KeyCode::Tab) && (overlay.active.is_none() || phone.opened) {
        overlay.phone_hint = false;
        if phone.opened {
            leave_app(&mut phone, &mut exits);
            phone.opened = false;
            overlay.active = None;
            // The Tab that lowers the phone must not recapture a stale mouse press.
            overlay.escape_consumed = true;
            commands.trigger(Sound::Back);
        } else {
            phone.opened = true;
            phone.feedback.clear();
            overlay.active = Some(OverlayKind::Phone);
            commands.trigger(Sound::Open);
        }
    }
    if may_operate(&state, &next, window, &overlay, &phone)
        && phone.stake_entry.is_none()
        && phone.cap_entry.is_none()
    {
        if keys.just_pressed(KeyCode::KeyH) {
            phone.pending = Some((phone.page, PhoneCommand::Page(PhonePage::Home)));
        }
        if keys.just_pressed(KeyCode::Backspace) {
            phone.pending = Some((phone.page, PhoneCommand::Page(phone.page.back())));
        }
    }
}

/// Intersect only the foreground camera's real mouse ray with the live screen plane.
/// Reject both edges outside the screen and rays pointing away from it.
fn screen_coordinates(ray: Ray3d, screen: &GlobalTransform) -> Option<Vec2> {
    let inverse = screen.affine().inverse();
    let origin = inverse.transform_point3(ray.origin);
    let direction = inverse.transform_vector3(*ray.direction);
    if direction.z >= -1e-6 {
        return None;
    }
    let distance = -origin.z / direction.z;
    if !distance.is_finite() || distance < 0.0 {
        return None;
    }
    let hit = origin + direction * distance;
    let uv = Vec2::new(hit.x / SCREEN_METERS.x + 0.5, 0.5 - hit.y / SCREEN_METERS.y);
    (uv.is_finite() && uv.cmpge(Vec2::ZERO).all() && uv.cmplt(Vec2::ONE).all())
        .then_some(uv * SCREEN_SIZE)
}

fn scale_cursor(cursor: Vec2, input_size: Vec2, viewport: Vec2) -> Option<Vec2> {
    (cursor.is_finite()
        && input_size.is_finite()
        && viewport.is_finite()
        && input_size.cmpgt(Vec2::ZERO).all()
        && viewport.cmpgt(Vec2::ZERO).all())
    .then_some(cursor * viewport / input_size)
}

fn cursor_in_viewport(window: &Window, camera: &Camera, mouse: &GameCursor) -> Option<Vec2> {
    let viewport = camera.logical_viewport_size()?;
    if mouse.captured {
        return scale_cursor(
            mouse.position?,
            Vec2::new(window.width(), window.height()),
            viewport,
        );
    }
    let cursor = window.cursor_position()?;
    // The browser canvas fills the window; CSS cursor pixels can differ from render pixels.
    #[cfg(target_arch = "wasm32")]
    let window_size = {
        let browser = web_sys::window()?;
        Vec2::new(
            browser.inner_width().ok()?.as_f64()? as f32,
            browser.inner_height().ok()?.as_f64()? as f32,
        )
    };
    #[cfg(not(target_arch = "wasm32"))]
    let window_size = Vec2::new(window.width(), window.height());
    scale_cursor(cursor, window_size, viewport)
}

fn drive_pointer(
    state: Res<State<GameState>>,
    next: Res<NextState<GameState>>,
    overlay: Res<GameplayOverlay>,
    mouse: Res<GameCursor>,
    mut phone: ResMut<Phone>,
    assets: Res<PhoneAssets>,
    windows: Query<&Window, With<PrimaryWindow>>,
    cameras: Query<(&Camera, &GlobalTransform), With<PhoneCamera>>,
    screens: Query<&GlobalTransform, With<Screen>>,
    buttons: Res<ButtonInput<MouseButton>>,
    mut pointers: MessageWriter<PointerInput>,
) {
    let hit = windows
        .single()
        .ok()
        .filter(|w| may_operate(&state, &next, w, &overlay, &phone))
        .filter(|_| phone.raise >= 0.99)
        .and_then(|window| {
            let (camera, transform) = cameras.single().ok()?;
            Some((
                cursor_in_viewport(window, camera, &mouse)?,
                (camera, transform),
                screens.single().ok()?,
            ))
        })
        .and_then(|(cursor, (camera, transform), screen)| {
            screen_coordinates(camera.viewport_to_world(transform, cursor).ok()?, screen)
        });
    let target = RenderTarget::Image(assets.target.clone().into())
        .normalize(None)
        .unwrap();
    let location = |position| Location {
        target: target.clone(),
        position,
    };
    if let Some(position) = hit {
        pointers.write(PointerInput::new(
            PHONE_POINTER,
            location(position),
            PointerAction::Move {
                delta: position - phone.pointer.unwrap_or(position),
            },
        ));
        if buttons.just_pressed(MouseButton::Left) {
            pointers.write(PointerInput::new(
                PHONE_POINTER,
                location(position),
                PointerAction::Press(PointerButton::Primary),
            ));
        }
        if buttons.just_released(MouseButton::Left) {
            pointers.write(PointerInput::new(
                PHONE_POINTER,
                location(position),
                PointerAction::Release(PointerButton::Primary),
            ));
        }
    } else if let Some(previous) = phone.pointer {
        pointers.write(PointerInput::new(
            PHONE_POINTER,
            location(previous),
            PointerAction::Cancel,
        ));
        pointers.write(PointerInput::new(
            PHONE_POINTER,
            location(Vec2::splat(-1000.0)),
            PointerAction::Move { delta: Vec2::ZERO },
        ));
    }
    phone.pointer = hit;
}

fn queue_press(press: On<Pointer<Press>>, buttons: Query<&PhoneButton>, mut phone: ResMut<Phone>) {
    if press.pointer_id == PHONE_POINTER
        && press.button == PointerButton::Primary
        && let Ok(button) = buttons.get(press.entity)
    {
        phone.pending = Some((button.0, button.1));
    }
}

fn dispatch(
    mut commands: Commands,
    localization: Res<Localization>,
    keys: Res<ButtonInput<KeyCode>>,
    mut keyboard: MessageReader<KeyboardInput>,
    mouse: Res<ButtonInput<MouseButton>>,
    state: Res<State<GameState>>,
    next: Res<NextState<GameState>>,
    windows: Query<(Entity, &Window), With<PrimaryWindow>>,
    mut phone: ResMut<Phone>,
    mut overlay: ResMut<GameplayOverlay>,
    mut store: ResMut<CampaignStore>,
    mut exits: MessageWriter<PhoneAppClosed>,
) {
    let pending = phone.pending.take();
    let Ok((primary_window, window)) = windows.single() else {
        keyboard.clear();
        return;
    };
    if !may_operate(&state, &next, window, &overlay, &phone) {
        keyboard.clear();
        return;
    }
    let Some(campaign) = store.campaign() else {
        keyboard.clear();
        return;
    };
    let choices = page_actions(&phone, campaign, &localization);
    let editing_cap = phone.cap_entry.is_some();
    if phone.stake_entry.is_some() || editing_cap {
        let entry = if editing_cap {
            phone.cap_entry.as_mut()
        } else {
            phone.stake_entry.as_mut()
        };
        let entered = edit_amount(keyboard.read(), primary_window, entry.unwrap());
        keyboard.clear();
        let confirm = entered
            || pending.is_some_and(|(page, command)| {
                page == phone.page
                    && command
                        == if editing_cap {
                            PhoneCommand::ConfirmCap
                        } else {
                            PhoneCommand::ConfirmStake
                        }
                    && phone.pointer.is_some()
                    && mouse.just_pressed(MouseButton::Left)
            });
        if confirm {
            if editing_cap {
                confirm_cap(&mut phone, &localization);
            } else {
                confirm_stake(&mut phone, campaign.balance_cents, &localization);
            }
            commands.trigger(
                if phone.stake_entry.is_some() || phone.cap_entry.is_some() {
                    Sound::Error
                } else {
                    Sound::Confirm
                },
            );
            return;
        }
        let navigating = pending.is_some_and(|(page, command)| {
            page == phone.page
                && phone.pointer.is_some()
                && mouse.just_pressed(MouseButton::Left)
                && matches!(
                    command,
                    PhoneCommand::Page(_)
                        | PhoneCommand::Close
                        | PhoneCommand::RetrySave
                        | PhoneCommand::Skip
                        | PhoneCommand::ToggleBetInfo
                )
        });
        if !navigating {
            return;
        }
    } else {
        keyboard.clear();
    }
    let command = [
        KeyCode::Digit1,
        KeyCode::Digit2,
        KeyCode::Digit3,
        KeyCode::Digit4,
        KeyCode::Digit5,
        KeyCode::Digit6,
        KeyCode::Digit7,
        KeyCode::Digit8,
        KeyCode::Digit9,
    ]
    .iter()
    .position(|key| {
        phone.stake_entry.is_none() && phone.cap_entry.is_none() && keys.just_pressed(*key)
    })
    .and_then(|i| choices.get(i).map(|(_, command)| *command))
    .or_else(|| {
        pending
            .filter(|(page, command)| {
                *page == phone.page
                    && (matches!(command, PhoneCommand::Page(PhonePage::Home))
                        && keys.just_pressed(KeyCode::KeyH)
                        || matches!(command, PhoneCommand::Page(p) if *p == phone.page.back())
                            && keys.just_pressed(KeyCode::Backspace)
                        || phone.pointer.is_some() && mouse.just_pressed(MouseButton::Left))
            })
            .map(|(_, command)| command)
    });
    let Some(command) = command else { return };
    let allowed = (phone.page == PhonePage::DailyCap
        && matches!(
            command,
            PhoneCommand::Cap(-1000 | -100 | 100 | 1000) | PhoneCommand::EditCap
        ))
        || choices.iter().any(|(_, allowed)| *allowed == command)
        || matches!(command, PhoneCommand::Close | PhoneCommand::RetrySave)
        || matches!(command, PhoneCommand::Page(p) if p == PhonePage::Home || p == phone.page.back());
    if !allowed {
        return;
    }
    if !matches!(
        command,
        PhoneCommand::Apply(_) | PhoneCommand::Skip | PhoneCommand::RetrySave
    ) {
        commands.trigger(
            if matches!(command, PhoneCommand::Close)
                || matches!(command, PhoneCommand::Page(p) if p == phone.page.back())
            {
                Sound::Back
            } else {
                Sound::Click
            },
        );
    }
    match command {
        PhoneCommand::Page(page) => {
            if phone.page.is_betting() && !page.is_betting() {
                leave_app(&mut phone, &mut exits);
            }
            if phone.page != page {
                phone.selected_bet = None;
            }
            phone.page = page;
            phone.bet_info_open = false;
            phone.animation_left = 0.0;
            phone.stake_entry = None;
            phone.cap_entry = None;
            phone.feedback.clear();
        }
        PhoneCommand::Close => {
            leave_app(&mut phone, &mut exits);
            phone.opened = false;
            overlay.active = None;
            overlay.escape_consumed = true;
        }
        PhoneCommand::RetrySave => {
            phone.feedback.clear();
            commands.trigger(if store.save().is_ok() {
                Sound::Confirm
            } else {
                Sound::Error
            });
        }
        PhoneCommand::Stake(delta) => {
            phone.stake_cents = phone
                .stake_cents
                .saturating_add(delta)
                .clamp(100, (campaign.balance_cents / 100 * 100).max(100));
            phone.feedback.clear();
        }
        PhoneCommand::EditStake => {
            phone.stake_entry = Some(String::new());
            phone.feedback.clear();
        }
        PhoneCommand::ConfirmStake => {}
        PhoneCommand::Cap(delta) => {
            phone.cap_cents = phone
                .cap_cents
                .saturating_add(delta)
                .clamp(0, i64::MAX / 100 * 100);
            phone.feedback.clear();
        }
        PhoneCommand::EditCap => {
            phone.cap_entry = Some(String::new());
            phone.feedback.clear();
        }
        PhoneCommand::ConfirmCap => {}
        PhoneCommand::Skip => {
            reveal_sound(&phone, &mut commands);
            phone.animation_left = 0.0;
        }
        PhoneCommand::SelectBet(choice) => {
            phone.selected_bet = Some(choice);
            phone.feedback.clear();
        }
        PhoneCommand::ToggleBetInfo => {
            phone.bet_info_open = !phone.bet_info_open;
        }
        PhoneCommand::Apply(action) => {
            if matches!(action, Action::Bet { .. } | Action::Sell(_))
                && (phone.animation_left > 0.0 || phone.raise < 0.99)
            {
                return;
            }
            let result = store.apply(action);
            match &result {
                Ok(_) => {
                    if let Some(sound) = Sound::action(action) {
                        commands.trigger(sound);
                    }
                }
                Err(_) => commands.trigger(Sound::Error),
            }
            if result.is_ok() && matches!(action, Action::Sell(_)) {
                phone.page = PhonePage::Household;
                if store
                    .campaign()
                    .is_some_and(|c| c.stage != CampaignStage::Playing)
                {
                    phone.opened = false;
                    overlay.active = None;
                    overlay.campaign_modal = true;
                }
            }
            if result.is_ok() && matches!(action, Action::Bet { .. }) {
                phone.settled_round = store
                    .campaign()
                    .and_then(|c| c.betting.history.last())
                    .cloned();
                phone.animation_left = 1.6;
                if matches!(phone.page, PhonePage::Roulette) {
                    commands.trigger(Sound::Roulette);
                }
                phone.feedback.clear();
                return;
            }
            phone.feedback = match result {
                Ok(_) => match action {
                    Action::Call(Contact::Marina) => localization.choose("Call with Marina finished", "Ligação com a Marina encerrada"),
                    Action::Call(Contact::Rafa) => localization.choose("Call with Rafa finished", "Ligação com o Rafa encerrada"),
                    Action::CareConversation => localization.choose("Care step recorded", "Conversa de apoio concluída"),
                    Action::TherapySession => localization.choose("Therapy session completed", "Sessão de terapia concluída"),
                    Action::FinancialPlanning => localization.choose("Plan recorded", "Planejamento registrado"),
                    Action::SetDailyCap(_) => localization.choose("Limit updated", "Limite atualizado"),
                    Action::BlockBetting => localization.choose("Betting blocked until morning", "Apostas bloqueadas até amanhã de manhã"),
                    Action::Sell(_) => localization.choose("Sold", "Venda concluída"),
                    _ => localization.choose("Done.", "Concluído."),
                }
                .into(),
                Err(error) => localization.action_error(&error),
            };
        }
    }
}

fn edit_amount<'a>(
    events: impl Iterator<Item = &'a KeyboardInput>,
    primary_window: Entity,
    entry: &mut String,
) -> bool {
    for input in
        events.filter(|input| input.window == primary_window && input.state == ButtonState::Pressed)
    {
        match &input.logical_key {
            Key::Backspace => {
                entry.pop();
            }
            Key::Enter => return true,
            _ => {
                let text = input.text.as_deref().or_else(|| match &input.logical_key {
                    Key::Character(text) => Some(text.as_str()),
                    _ => None,
                });
                for digit in text
                    .unwrap_or_default()
                    .chars()
                    .filter(char::is_ascii_digit)
                {
                    if entry.len() < 16 {
                        entry.push(digit);
                    }
                }
            }
        }
    }
    false
}

fn confirm_stake(phone: &mut Phone, balance: i64, localization: &Localization) {
    let cents = phone
        .stake_entry
        .as_ref()
        .and_then(|s| s.parse::<i64>().ok())
        .and_then(|r| r.checked_mul(100));
    if let Some(cents) = cents.filter(|c| *c >= 100 && *c <= balance) {
        phone.stake_cents = cents;
        phone.stake_entry = None;
        phone.feedback.clear();
    } else {
        phone.feedback = localization.choose("Enter whole reais from R$1 to your balance.", "Digite um valor inteiro em reais, entre R$1 e seu saldo.").into();
    }
}

fn confirm_cap(phone: &mut Phone, localization: &Localization) {
    let cents = phone
        .cap_entry
        .as_ref()
        .and_then(|entry| entry.parse::<i64>().ok())
        .and_then(|reais| reais.checked_mul(100));
    if let Some(cents) = cents.filter(|value| *value >= 0) {
        phone.cap_cents = cents;
        phone.cap_entry = None;
        phone.feedback = localization.choose("Amount selected. Apply to save.", "Valor selecionado. Aplique o limite para salvar.").into();
    } else {
        phone.feedback = localization.choose("Enter a whole-real cap of R$0 or more.", "Digite um limite inteiro em reais, a partir de R$0.").into();
    }
}

fn end_session(mut exits: MessageReader<PhoneAppClosed>, mut store: ResMut<CampaignStore>) {
    if exits.read().count() > 0
        && store
            .campaign()
            .is_some_and(|c| c.betting.session.is_some())
    {
        let _ = store.apply(Action::EndBetSession);
    }
}

fn page_for_choice(choice: BetChoice) -> PhonePage {
    match choice {
        BetChoice::RouletteRed | BetChoice::RouletteBlack => PhonePage::Roulette,
        BetChoice::Slots => PhonePage::Slots,
        _ => PhonePage::Football,
    }
}

fn restore_session(
    store: Res<CampaignStore>,
    mut phone: ResMut<Phone>,
    mut overlay: ResMut<GameplayOverlay>,
) {
    if let Some(c) = store
        .campaign()
        .filter(|c| c.stage == CampaignStage::Playing && c.betting.session.is_some())
    {
        if !phone.opened {
            let round = c.betting.history.last().unwrap();
            phone.page = page_for_choice(round.choice);
            phone.opened = true;
            phone.settled_round = Some(round.clone());
            phone.animation_left = 0.0;
            phone.stake_cents = round.stake_cents;
            phone.selected_bet = Some(round.choice);
            phone.pending = None;
            overlay.active = Some(OverlayKind::Phone);
        }
    }
}

fn tick_game(
    mut commands: Commands,
    time: Res<Time<Real>>,
    state: Res<State<GameState>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut phone: ResMut<Phone>,
) {
    if *state.get() == GameState::Playing
        && phone.opened
        && windows.single().is_ok_and(|w| w.focused)
    {
        let previous = phone.animation_left;
        phone.animation_left = (phone.animation_left - time.delta_secs()).max(0.0);
        if previous > 0.0 && phone.animation_left == 0.0 {
            if phone.page == PhonePage::Football {
                commands.trigger(Sound::Whistle);
            }
            // The saved round is authoritative; audio only follows its reveal.
            if let Some(round) = &phone.settled_round {
                commands.trigger(if round.net_cents > 0 {
                    Sound::Win
                } else {
                    Sound::Loss
                });
            }
        } else if phone.page == PhonePage::Slots
            && (previous * 10.0).ceil() != (phone.animation_left * 10.0).ceil()
        {
            commands.trigger(Sound::Tick);
        }
    }
}

fn reveal_sound(phone: &Phone, commands: &mut Commands) {
    if phone.animation_left > 0.0
        && let Some(round) = &phone.settled_round
    {
        if phone.page == PhonePage::Football {
            commands.trigger(Sound::Whistle);
        }
        commands.trigger(if round.net_cents > 0 {
            Sound::Win
        } else {
            Sound::Loss
        });
    }
}

impl Phone {
    pub(super) fn animating(&self) -> bool {
        self.opened && self.animation_left > 0.0
    }
}

fn page_actions(phone: &Phone, c: &Campaign, localization: &Localization) -> Vec<(String, PhoneCommand)> {
    match phone.page {
        PhonePage::Home => launcher::APPS
            .iter()
            .map(|&(page, label, _)| (localization.text(label).into(), PhoneCommand::Page(page)))
            .collect(),
        page if page.is_betting() => betting::actions(phone, c, localization),
        PhonePage::Household | PhonePage::SaleOffers | PhonePage::ConfirmSale(_) => {
            life::actions(phone, c, localization)
        }
        _ => care::actions(phone, c, localization),
    }
}

fn home_reminder(c: &Campaign, localization: &Localization) -> String {
    let mut lines = Vec::new();
    let scheduled = c
        .commitments
        .iter()
        .filter(|plan| plan.status == CommitmentStatus::Scheduled);
    for plan in scheduled.clone().filter(|plan| plan.day == c.clock.day) {
        let activity = if c.uses_phone_for(plan.contact) { localization.choose("calls", "liga") } else { localization.choose("visits", "vem visitar") };
        lines.push(format!(
            "{} {activity} · {}",
            plan.contact.label(),
            localization.period(plan.period.label()).to_lowercase()
        ));
    }
    let unpaid = c.bills.iter().filter(|bill| bill.paid_day.is_none());
    let due_today = unpaid.clone().filter(|bill| bill.due_day == c.clock.day).count();
    if due_today > 0 {
        lines.push(if due_today == 1 {
            localization.choose("1 bill due today", "1 conta vence hoje").into()
        } else {
            format_text!(localization, "{due_today} bills due today", "{due_today} contas vencem hoje")
        });
    }
    let overdue = unpaid.filter(|bill| bill.due_day < c.clock.day).count();
    if overdue > 0 {
        lines.push(if overdue == 1 {
            localization.choose("1 overdue bill", "1 conta vencida").into()
        } else {
            format_text!(localization, "{overdue} overdue bills", "{overdue} contas vencidas")
        });
    }
    if !c.has_power() {
        lines.push(localization.choose("Power off · electricity bill on the table", "Energia cortada · conta de luz em cima da mesa").into());
    }
    if !lines.is_empty() {
        return lines.join("\n");
    }
    scheduled
        .min_by_key(|plan| (plan.day, plan.period))
        .map(|plan| {
            format_text!(
                localization,
                "{}: day {} {}",
                "{}: dia {} · {}",
                plan.contact.label(),
                plan.day,
                localization.period(plan.period.label()).to_lowercase()
            )
        })
        .unwrap_or_else(|| localization.choose("Check Chats for messages", "Confira as mensagens em Conversas").into())
}

fn text(
    parent: &mut ChildSpawnerCommands,
    font: &Handle<Font>,
    value: impl Into<String>,
    size: f32,
    color: Color,
) {
    parent.spawn((
        Text::new(value),
        TextFont {
            font: font.clone().into(),
            font_size: FontSize::Px(size),
            ..default()
        },
        TextColor(color),
        Pickable::IGNORE,
    ));
}
fn present(
    mut commands: Commands,
    phone: Res<Phone>,
    store: Res<CampaignStore>,
    assets: Res<PhoneAssets>,
    skin: Res<launcher::Skin>,
    art: Res<ui::Art>,
    localization: Res<Localization>,
    existing: Query<Entity, With<PhoneUi>>,
    scrolling: Query<&ScrollPosition, With<PhoneScroll>>,
    mut previous: Local<String>,
    mut last_page: Local<Option<PhonePage>>,
) {
    let Some(c) = store.campaign() else { return };
    let signature = format!(
        "{:?}|{}|{}|{:?}|{}|{}|{:?}|{}|{:?}|{:?}|{}",
        phone.page,
        phone.feedback,
        store.notice,
        phone.stake_entry,
        phone.stake_cents,
        phone.animation_left > 0.0,
        phone.settled_round,
        phone.cap_cents,
        phone.cap_entry,
        phone.selected_bet,
        phone.bet_info_open,
    );
    if *previous == signature && !store.is_changed() && !localization.is_changed() && !existing.is_empty() {
        return;
    }
    *previous = signature;
    let offset = if *last_page == Some(phone.page) {
        scrolling.single().map(|s| s.0).unwrap_or_default()
    } else {
        Vec2::ZERO
    };
    *last_page = Some(phone.page);
    for entity in &existing {
        commands.entity(entity).despawn();
    }
    commands
        .spawn((
            PhoneUi,
            UiTargetCamera(assets.camera),
            BackgroundColor(Color::srgb(0.035, 0.085, 0.13)),
            Node {
                width: percent(100),
                height: percent(100),
                flex_direction: FlexDirection::Column,
                ..default()
            },
        ))
        .with_children(|shell| {
            if phone.page == PhonePage::Home {
                shell.spawn((
                    ImageNode::new(skin.wallpaper.clone()),
                    Node {
                        position_type: PositionType::Absolute,
                        width: percent(100),
                        height: percent(100),
                        ..default()
                    },
                    Pickable::IGNORE,
                ));
            }
            launcher::status_bar(shell, &skin, c);
            let background = if phone.page == PhonePage::Home {
                Color::NONE
            } else if phone.page.is_betting() {
                Color::srgb(0.055, 0.065, 0.10)
            } else {
                Color::srgb(0.97, 0.97, 0.98)
            };
            shell
                .spawn((
                    BackgroundColor(background),
                    Node {
                        width: percent(100),
                        flex_grow: 1.0,
                        min_height: px(0),
                        flex_direction: FlexDirection::Column,
                        ..default()
                    },
                ))
                .with_children(|root| {
                    if !store.notice.is_empty() || !phone.feedback.is_empty() {
                        root.spawn((
                            BackgroundColor(Color::srgb(1., 0.91, 0.71)),
                            Node {
                                padding: UiRect::all(px(10)),
                                ..ui::column()
                            },
                        ))
                        .with_children(|notice| {
                            if !store.notice.is_empty() {
                                text(notice, &art.regular, localization.campaign_notice(&store.notice), 18., Color::BLACK);
                                ui::action(
                                    notice,
                                    &assets.font,
                                    phone.page,
                                    localization.choose("Retry saving", "Tentar salvar novamente"),
                                    PhoneCommand::RetrySave,
                                    Color::srgb(0.30, 0.20, 0.08),
                                    Color::WHITE,
                                );
                            }
                            if !phone.feedback.is_empty() {
                                text(notice, &art.regular, &phone.feedback, 19., Color::BLACK);
                            }
                        });
                    }
                    let mut body = root.spawn(Node {
                        width: percent(100),
                        flex_direction: FlexDirection::Column,
                        row_gap: px(12),
                        padding: UiRect::all(px(16)),
                        flex_grow: 1.0,
                        min_height: px(0),
                        overflow: if phone.page == PhonePage::Home {
                            Overflow::default()
                        } else {
                            Overflow::scroll_y()
                        },
                        ..default()
                    });
                    if phone.page != PhonePage::Home {
                        body.insert((PhoneScroll, ScrollPosition(offset)));
                    }
                    body.with_children(|body| match phone.page {
                        PhonePage::Home => launcher::home(body, &skin, c, &localization),
                        page if page.is_betting() => {
                            betting::render(body, &assets, &art, &phone, c, &localization)
                        }
                        PhonePage::History => finance::render(body, &assets, &art, c, &localization),
                        PhonePage::Household | PhonePage::SaleOffers | PhonePage::ConfirmSale(_) => {
                            life::render(body, &assets, &art, &phone, c, &localization)
                        }
                        _ => care::render(body, &assets, &art, &phone, c, &localization),
                    });
                });
            launcher::navigation(shell, &skin, phone.page, &localization);
        });
}

fn scroll(
    mut wheel: MessageReader<MouseWheel>,
    keys: Res<ButtonInput<KeyCode>>,
    state: Res<State<GameState>>,
    next: Res<NextState<GameState>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    overlay: Res<GameplayOverlay>,
    phone: Res<Phone>,
    mut panels: Query<(&mut ScrollPosition, &ComputedNode), With<PhoneScroll>>,
) {
    let events: f32 = wheel
        .read()
        .map(|e| {
            -e.y * if e.unit == MouseScrollUnit::Line {
                42.0
            } else {
                1.0
            }
        })
        .sum();
    if !windows
        .single()
        .is_ok_and(|w| may_operate(&state, &next, w, &overlay, &phone))
    {
        return;
    }
    let mut delta = if phone.pointer.is_some() { events } else { 0.0 };
    if keys.just_pressed(KeyCode::PageDown) {
        delta += 250.0;
    }
    if keys.just_pressed(KeyCode::PageUp) {
        delta -= 250.0;
    }
    for (mut position, node) in &mut panels {
        let max = ((node.content_size().y - node.size().y) * node.inverse_scale_factor()).max(0.0);
        position.y = (position.y + delta).clamp(0.0, max);
    }
}

fn animate(
    time: Res<Time<Real>>,
    state: Res<State<GameState>>,
    overlay: Res<GameplayOverlay>,
    mut phone: ResMut<Phone>,
    mut roots: Query<(&mut Transform, &mut Visibility), With<PhoneRoot>>,
    mut cameras: Query<&mut Camera, Or<(With<PhoneCamera>, With<TextureCamera>)>>,
    windows: Query<&Window, With<PrimaryWindow>>,
) {
    let playing = *state.get() == GameState::Playing;
    let target = if phone.opened && !overlay.in_dialogue() {
        1.0
    } else {
        0.0
    };
    let step = time.delta_secs() / 0.22;
    phone.raise += (target - phone.raise).clamp(-step, step);
    let visible = playing && phone.raise > 0.0;
    for mut camera in &mut cameras {
        camera.is_active = visible;
    }
    let ease = 1.0 - (1.0 - phone.raise).powi(3);
    // Keep the complete screen inside narrow windows without changing the world camera.
    let aspect = windows
        .single()
        .map(|w| w.width() / w.height().max(1.0))
        .unwrap_or(16.0 / 9.0);
    let distance = 0.74_f32.max(0.40 / aspect.max(0.2));
    for (mut transform, mut visibility) in &mut roots {
        *visibility = if visible {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
        transform.translation = Vec3::new(
            if aspect > 1.2 { 0.10 } else { 0.0 },
            -0.015 - (1.0 - ease) * 0.7,
            -distance,
        );
        transform.rotation =
            Quat::from_rotation_z(-0.025) * Quat::from_rotation_x((1.0 - ease) * -0.4);
    }
}

fn reset(_: On<CampaignReplaced>, mut phone: ResMut<Phone>) {
    *phone = Phone::default();
}

#[cfg(test)]
mod tests;
