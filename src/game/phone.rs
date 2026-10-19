//! Alex's physical phone: the apartment stays visible around a readable, native UI screen.
use bevy::{
    asset::{RenderAssetUsages, uuid::Uuid},
    camera::{RenderTarget, visibility::RenderLayers},
    input::{
        ButtonState,
        keyboard::{Key, KeyboardInput},
        mouse::{MouseScrollUnit, MouseWheel},
    },
    light::NotShadowCaster,
    picking::{
        PickingSystems,
        pointer::{Location, PointerAction, PointerId, PointerInput},
    },
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat, TextureUsages},
    window::PrimaryWindow,
};

use super::{
    campaign::*,
    campaign_save::{CampaignReplaced, CampaignStore},
    cursor::{CursorInput, GameCursor},
    player::{Player, PlayerInput},
    routine::{GameplayOverlay, OverlayInput, OverlayKind, reais},
};
use crate::state::GameState;

const SCREEN_SIZE: Vec2 = Vec2::new(420.0, 735.0);
const SCREEN_METERS: Vec2 = Vec2::new(0.300, 0.525);
const PHONE_LAYER: usize = 1;
const PHONE_POINTER: PointerId = PointerId::Custom(Uuid::from_u128(0x416c657850686f6e65));
const INK: Color = Color::srgb(0.94, 0.93, 0.88);
const MUTED: Color = Color::srgb(0.72, 0.76, 0.73);
const BUTTON: Color = Color::srgb(0.15, 0.21, 0.21);

pub struct PhonePlugin;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum PhonePage {
    #[default]
    Home,
    Betting,
    Roulette,
    Slots,
    Football,
    Bank,
    History,
    Reports,
    Status,
    Household,
    ConfirmSale(HouseholdItem),
    Contacts,
    Support,
    DailyCap,
    Food,
}
impl PhonePage {
    fn title(self) -> &'static str {
        match self {
            Self::Home => "Alex's phone",
            Self::Betting => "Betting",
            Self::Roulette => "Roulette",
            Self::Slots => "Slots",
            Self::Football => "Football",
            Self::Bank => "Bank",
            Self::History => "History",
            Self::Reports => "Daily reports",
            Self::Status => "Status",
            Self::Household => "Household / sales",
            Self::ConfirmSale(_) => "Confirm permanent sale",
            Self::Contacts => "Contacts",
            Self::Support => "Support",
            Self::DailyCap => "Daily wagering cap",
            Self::Food => "Food",
        }
    }
    pub(crate) fn is_betting(self) -> bool {
        matches!(
            self,
            Self::Betting | Self::Roulette | Self::Slots | Self::Football
        )
    }
    fn is_game(self) -> bool {
        matches!(self, Self::Roulette | Self::Slots | Self::Football)
    }
    fn back(self) -> Self {
        if self == Self::Reports {
            Self::History
        } else if matches!(self, Self::ConfirmSale(_)) {
            Self::Household
        } else if self == Self::Household {
            Self::Status
        } else if self == Self::DailyCap {
            Self::Support
        } else if matches!(self, Self::Roulette | Self::Slots | Self::Football) {
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
                    sync_view_msaa,
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
    let target = images.add(image);
    let camera = commands
        .spawn((
            TextureCamera,
            Camera2d,
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
    exits.write(PhoneAppClosed);
}

fn input(
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
    if phone.opened && overlay.active != Some(OverlayKind::Phone) {
        leave_app(&mut phone, &mut exits);
        phone.opened = false;
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
        if phone.opened {
            leave_app(&mut phone, &mut exits);
            phone.opened = false;
            overlay.active = None;
            // The Tab that lowers the phone must not recapture a stale mouse press.
            overlay.escape_consumed = true;
        } else {
            phone.opened = true;
            phone.feedback.clear();
            overlay.active = Some(OverlayKind::Phone);
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
    let (_, choices) = phone_content(&phone, campaign);
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
                confirm_cap(&mut phone);
            } else {
                confirm_stake(&mut phone, campaign.balance_cents);
            }
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
    let allowed = (phone.page.is_game()
        && matches!(
            command,
            PhoneCommand::Stake(-1000 | -100 | 100 | 1000)
                | PhoneCommand::EditStake
                | PhoneCommand::Page(PhonePage::Roulette | PhonePage::Slots | PhonePage::Football)
        ))
        || (phone.page == PhonePage::DailyCap
            && matches!(
                command,
                PhoneCommand::Cap(-1000 | -100 | 100 | 1000) | PhoneCommand::EditCap
            ))
        || (phone.animation_left > 0.0 && command == PhoneCommand::Skip)
        || choices.iter().any(|(_, allowed)| *allowed == command)
        || matches!(command, PhoneCommand::Close | PhoneCommand::RetrySave)
        || matches!(command, PhoneCommand::Page(p) if p == PhonePage::Home || p == phone.page.back());
    if !allowed {
        return;
    }
    match command {
        PhoneCommand::Page(page) => {
            if phone.page.is_betting() && !page.is_betting() {
                leave_app(&mut phone, &mut exits);
            }
            phone.page = page;
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
            let _ = store.save();
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
            phone.animation_left = 0.0;
        }
        PhoneCommand::Apply(action) => {
            if matches!(action, Action::Bet { .. } | Action::Sell(_))
                && (phone.animation_left > 0.0 || phone.raise < 0.99)
            {
                return;
            }
            let result = store.apply(action);
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
                phone.feedback.clear();
                return;
            }
            phone.feedback = match result {
                Ok(_) => match action {
                    Action::Groceries => "Groceries collected: 3 food portions.",
                    Action::Cook | Action::OrderFood => {
                        "Meal finished. Hunger reduced; the meal left some waste."
                    }
                    Action::PayBill(BillKind::Electricity) => "Electricity paid. Power is on.",
                    Action::PayBill(BillKind::Rent) => {
                        "Rent paid. The housing deadline is resolved."
                    }
                    Action::NegotiateRent => "Rent extended to end of day 10. R$420 is still owed.",
                    Action::Call(Contact::Marina) => "You made time to talk with Marina.",
                    Action::Call(Contact::Rafa) => {
                        "You made time to talk with Rafa. Visits still happen at the door."
                    }
                    Action::CareConversation => {
                        "One care step recorded. Change takes time; you can return."
                    }
                    Action::FinancialPlanning => {
                        "Planning recorded. Review your balance and deadlines in Bank."
                    }
                    Action::SetDailyCap(_) => {
                        "Cap choice applied. Check the current and pending limits below."
                    }
                    Action::BlockBetting => {
                        "Betting blocked until tomorrow morning. Information stays available."
                    }
                    Action::Sell(_) => "Sale completed. The item has permanently left your home.",
                    _ => "Done.",
                }
                .into(),
                Err(error) => error,
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

fn confirm_stake(phone: &mut Phone, balance: i64) {
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
        phone.feedback = "Enter whole reais from R$1 to your balance.".into();
    }
}

fn confirm_cap(phone: &mut Phone) {
    let cents = phone
        .cap_entry
        .as_ref()
        .and_then(|entry| entry.parse::<i64>().ok())
        .and_then(|reais| reais.checked_mul(100));
    if let Some(cents) = cents.filter(|value| *value >= 0) {
        phone.cap_cents = cents;
        phone.cap_entry = None;
        phone.feedback = "Amount selected. Apply cap to save this choice.".into();
    } else {
        phone.feedback = "Enter a whole-real cap of R$0 or more.".into();
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
            phone.pending = None;
            overlay.active = Some(OverlayKind::Phone);
        }
    }
}

fn tick_game(
    time: Res<Time<Real>>,
    state: Res<State<GameState>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut phone: ResMut<Phone>,
) {
    if *state.get() == GameState::Playing
        && phone.opened
        && windows.single().is_ok_and(|w| w.focused)
    {
        phone.animation_left = (phone.animation_left - time.delta_secs()).max(0.0);
    }
}

fn animation_frame(phone: &Phone) -> u8 {
    (phone.animation_left * 10.0).ceil() as u8
}

fn game_visual(phone: &Phone) -> String {
    let Some(round) = phone
        .settled_round
        .as_ref()
        .filter(|r| page_for_choice(r.choice) == phone.page)
    else {
        return "[ READY ]".into();
    };
    if phone.animation_left > 0.0 {
        let frame = animation_frame(phone);
        return match phone.page {
            PhonePage::Roulette => if frame % 2 == 0 {
                "[ > RED | BLACK ]"
            } else {
                "[ RED | BLACK < ]"
            }
            .into(),
            PhonePage::Slots => [
                "[ 7 | BAR | BELL ]",
                "[ BAR | BELL | 7 ]",
                "[ BELL | 7 | BAR ]",
            ][frame as usize % 3]
                .into(),
            _ => format!(
                "HOME {}o{} AWAY",
                ".".repeat(frame as usize % 6),
                ".".repeat(5 - frame as usize % 6)
            ),
        };
    }
    match round.outcome {
        BetOutcome::RouletteRed => "[ RED ]",
        BetOutcome::RouletteBlack => "[ BLACK ]",
        BetOutcome::SlotsWin => "[ 7 | 7 | 7 ]",
        BetOutcome::SlotsLoss => "[ BAR | 7 | BELL ]",
        BetOutcome::FootballHome => "HOME 2 - 1 AWAY",
        BetOutcome::FootballDraw => "HOME 1 - 1 AWAY",
        BetOutcome::FootballAway => "HOME 1 - 2 AWAY",
    }
    .into()
}

fn phone_content(phone: &Phone, c: &Campaign) -> (String, Vec<(String, PhoneCommand)>) {
    if phone.page == PhonePage::DailyCap {
        let (copy, _) = page_content(phone.page, c);
        let mut choices = if phone.cap_entry.is_some() {
            vec![]
        } else {
            vec![(
                format!("Apply cap: {} / free", reais(phone.cap_cents)),
                PhoneCommand::Apply(Action::SetDailyCap(Some(phone.cap_cents))),
            )]
        };
        if phone.cap_entry.is_none() && c.support.daily_cap_cents.is_some() {
            choices.push((
                "Remove cap tomorrow / free".into(),
                PhoneCommand::Apply(Action::SetDailyCap(None)),
            ));
        }
        return (copy, choices);
    }
    if !phone.page.is_game() {
        return page_content(phone.page, c);
    }
    let rules = match phone.page {
        PhonePage::Roulette => "Red / Black: each 50%, return 1.8x.",
        PhonePage::Slots => "Win: 30%, return 3x. Loss: 70%.",
        _ => "Home: 50% / 1.8x. Draw / Away: each 25% / 3.6x.",
    };
    let mut copy = format!(
        "{rules}\nReturns include stake. Average return: 90%.\n\nEach PA buys 3 rounds. Leaving forfeits leftovers. Whole reais; no borrowing."
    );
    if let Some(cap) = c.support.daily_cap_cents {
        copy.push_str(&format!("\nDaily gross cap: {}.", reais(cap)));
    }
    if c.support.app_blocked_today {
        copy.push_str("\nBlocked until tomorrow morning.");
    }
    let choices = if phone.animation_left > 0.0 {
        vec![("Stop / show settled result".into(), PhoneCommand::Skip)]
    } else if phone.stake_entry.is_some() {
        vec![]
    } else {
        let choices: &[(&str, BetChoice)] = match phone.page {
            PhonePage::Roulette => &[
                ("Bet RED", BetChoice::RouletteRed),
                ("Bet BLACK", BetChoice::RouletteBlack),
            ],
            PhonePage::Slots => &[("SPIN", BetChoice::Slots)],
            _ => &[
                ("Bet HOME", BetChoice::FootballHome),
                ("Bet DRAW", BetChoice::FootballDraw),
                ("Bet AWAY", BetChoice::FootballAway),
            ],
        };
        choices
            .iter()
            .map(|(label, choice)| {
                (
                    format!("{label}: {}", reais(phone.stake_cents)),
                    PhoneCommand::Apply(Action::Bet {
                        choice: *choice,
                        stake_cents: phone.stake_cents,
                    }),
                )
            })
            .collect()
    };
    (copy, choices)
}

fn game_controls(parent: &mut ChildSpawnerCommands, font: &Handle<Font>, phone: &Phone) {
    parent
        .spawn((
            BackgroundColor(Color::srgb(0.09, 0.16, 0.13)),
            Node {
                width: percent(100),
                min_height: px(58),
                padding: UiRect::all(px(10)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                flex_shrink: 0.0,
                ..default()
            },
        ))
        .with_children(|p| {
            text(
                p,
                font,
                game_visual(phone),
                25.0,
                Color::srgb(0.9, 0.87, 0.58),
            )
        });
    if let Some(round) = phone
        .settled_round
        .as_ref()
        .filter(|r| page_for_choice(r.choice) == phone.page)
    {
        text(
            parent,
            font,
            format!(
                "Settled: return {} / net {}",
                reais(round.total_return_cents),
                reais(round.net_cents)
            ),
            21.0,
            INK,
        );
    }
    if let Some(entry) = &phone.stake_entry {
        text(
            parent,
            font,
            format!("Type whole reais: R${entry}_\nBackspace: erase / Enter: set stake"),
            24.0,
            INK,
        );
        button(
            parent,
            font,
            "Set stake",
            phone.page,
            PhoneCommand::ConfirmStake,
        );
    } else {
        button(
            parent,
            font,
            &format!("Stake {} / type amount", reais(phone.stake_cents)),
            phone.page,
            PhoneCommand::EditStake,
        );
        parent
            .spawn(Node {
                flex_direction: FlexDirection::Row,
                column_gap: px(5),
                flex_shrink: 0.0,
                ..default()
            })
            .with_children(|row| {
                for (label, delta) in [("-10", -1000), ("-1", -100), ("+1", 100), ("+10", 1000)] {
                    row.spawn(Node {
                        flex_grow: 1.0,
                        flex_basis: px(0),
                        ..default()
                    })
                    .with_children(|p| {
                        button(p, font, label, phone.page, PhoneCommand::Stake(delta))
                    });
                }
            });
    }
}

fn game_modes(parent: &mut ChildSpawnerCommands, font: &Handle<Font>, page: PhonePage) {
    parent
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            column_gap: px(5),
            flex_shrink: 0.0,
            ..default()
        })
        .with_children(|row| {
            for (label, target) in [
                ("Roulette", PhonePage::Roulette),
                ("Slots", PhonePage::Slots),
                ("Football", PhonePage::Football),
            ] {
                row.spawn(Node {
                    flex_grow: 1.0,
                    flex_basis: px(0),
                    ..default()
                })
                .with_children(|p| button(p, font, label, page, PhoneCommand::Page(target)));
            }
        });
}

fn cap_controls(parent: &mut ChildSpawnerCommands, font: &Handle<Font>, phone: &Phone) {
    if let Some(entry) = &phone.cap_entry {
        text(
            parent,
            font,
            format!("Type whole reais: R${entry}_\nBackspace: erase / Enter: select"),
            24.0,
            INK,
        );
        button(
            parent,
            font,
            "Select amount / free",
            phone.page,
            PhoneCommand::ConfirmCap,
        );
    } else {
        button(
            parent,
            font,
            &format!("Selected {} / type amount", reais(phone.cap_cents)),
            phone.page,
            PhoneCommand::EditCap,
        );
        parent
            .spawn(Node {
                flex_direction: FlexDirection::Row,
                column_gap: px(5),
                flex_shrink: 0.0,
                ..default()
            })
            .with_children(|row| {
                for (label, delta) in [("-10", -1000), ("-1", -100), ("+1", 100), ("+10", 1000)] {
                    row.spawn(Node {
                        flex_grow: 1.0,
                        flex_basis: px(0),
                        ..default()
                    })
                    .with_children(|p| {
                        button(p, font, label, phone.page, PhoneCommand::Cap(delta))
                    });
                }
            });
    }
}

fn page_content(page: PhonePage, c: &Campaign) -> (String, Vec<(String, PhoneCommand)>) {
    use PhonePage::*;
    let nav = |label: &str, page| (label.into(), PhoneCommand::Page(page));
    let action = |label: &str, action| (label.into(), PhoneCommand::Apply(action));
    match page {
        Home => (home_reminder(c), vec![nav("Betting", Betting), nav("Bank", Bank), nav("History", History), nav("Status", Status), nav("Contacts / messages", Contacts), nav("Support", Support), nav("Food", Food)]),
        Betting => ("Fictional games. Stakes and returns use campaign reais.\n\nEach choice returns 90% on average over many bets. A win is never guaranteed.\n\nOne shared session buys up to 3 rounds per PA. Leaving the app forfeits unused rounds.".into(), vec![nav("Roulette", Roulette), nav("Slots", Slots), nav("Football", Football)]),
        Roulette | Slots | Football => phone_content(&Phone { page, ..default() }, c),
        Bank => {
            let mut text = format!("Available balance\n{}\n\nBills\n", reais(c.balance_cents));
            let mut choices = vec![];
            for bill in &c.bills {
                text.push_str(&format!("\n{}: {}\nDue end of day {}\n{}\n", bill.kind.label(), reais(bill.amount_cents), bill.due_day,
                    bill.paid_day.map(|day| format!("Paid on day {day}")).unwrap_or_else(|| if c.clock.day > bill.due_day { "Overdue".into() } else { "Unpaid".into() })));
                if bill.paid_day.is_none() {
                    choices.push(action(&format!("Pay {}: {} / free", bill.kind.label(), reais(bill.amount_cents)), Action::PayBill(bill.kind)));
                }
            }
            text.push_str(if c.has_power() { "\nPower is on." } else { "\nPower is off. Paying electricity restores it immediately." });
            if !c.support.rent_negotiated && c.clock.day < 9 && c.bills.iter().any(|bill| bill.kind == BillKind::Rent && bill.paid_day.is_none()) {
                text.push_str("\nOne rent negotiation costs 1 PA and extends the deadline to end of day 10. Arrange it before the start of day 9.");
                choices.push(action("Negotiate rent: 1 PA / due day 10", Action::NegotiateRent));
            } else if c.support.rent_negotiated {
                text.push_str("\nThe one-time rent extension was used: deadline end of day 10.");
            }
            text.push_str("\nPayments cost no PA, including at zero PA. All transactions are in History.");
            choices.push(nav("View full history", History));
            (text, choices)
        }
        History => (super::routine::history_text(c), vec![nav("Read all daily reports / free", Reports)]),
        Reports => (super::routine::reports_text(c), vec![]),
        Status => {
            let a = &c.attributes;
            let mut text = format!("Exact attributes / 0 to 100\n\nHunger: {}\nBoredom: {}\nStress: {}\nUrge: {}\nRelationships: {}\n\nFood: {} portions\nMess: {} / 3\nPower: {}\nPA remaining: {} / 6\n\nHousehold\n", a.hunger, a.boredom, a.stress, a.urge, a.relationships, c.household.food_portions, c.household.mess, if c.has_power() { "On" } else { "Off" }, c.clock.remaining_pa());
            for item in [HouseholdItem::Tv, HouseholdItem::Fridge, HouseholdItem::Stove, HouseholdItem::Cookware, HouseholdItem::Couch, HouseholdItem::Books, HouseholdItem::Pipoca] {
                text.push_str(&format!("\n{}: {}", item.label(), if c.household.owns(item) { "At home" } else { "Sold" }));
            }
            (text, vec![nav("Household / permanent sales", Household), nav("Food options", Food)])
        }
        Household => {
            let mut text = format!("Balance: {}\nPA remaining: {} / 6\n\nSales are voluntary and permanent. Each sale takes 1 PA.\n\n", reais(c.balance_cents), c.clock.remaining_pa());
            let mut choices = vec![];
            for item in super::household::HOUSEHOLD_ITEMS.into_iter().chain([HouseholdItem::Pipoca]) {
                text.push_str(&format!("{}: {}\n", item.label(), if c.household.owns(item) { "At home" } else { "Sold" }));
                if c.can_sell(item).is_ok() {
                    choices.push(nav(&format!("Review {} sale: {} / 1 PA", item.label(), reais(item.sale_cents())), ConfirmSale(item)));
                }
            }
            if c.attributes.stress < 70 && c.attributes.urge < 70 {
                text.push_str("\nHousehold sale offers appear at stress or urge 70+.\n");
            }
            text.push_str("\nPipoca is only offered after all six groups are sold. Selling Pipoca ends the campaign immediately. The bed is never for sale.");
            (text, choices)
        }
        ConfirmSale(item) => {
            let mut text = format!("Sell {} permanently?\n\nReceive {} / costs 1 PA\n\n{}", item.label(), reais(item.sale_cents()), item.sale_loss());
            if item == HouseholdItem::Fridge {
                text.push_str(&format!("\n\nYou will lose ALL {} stored portions.", c.household.food_portions));
            }
            text.push_str("\n\nThis cannot be undone in this campaign.");
            let mut choices = vec![nav("Cancel / keep at home", Household)];
            if c.can_sell(item).is_ok() {
                choices.push(action(if item == HouseholdItem::Pipoca { "Sell Pipoca / END CAMPAIGN" } else { "Confirm permanent sale" }, Action::Sell(item)));
            }
            (text, choices)
        }
        Contacts => {
            let mut text = format!("Messages / free to read\n\n{}\n\nMarina / Alex's sister\n{}\n\nRafa / Alex's friend\n{}\n\nAnnounced calendar\n", situation(c), contact_message(c, Contact::Marina), contact_message(c, Contact::Rafa));
            for plan in &c.commitments { text.push_str(&format!("\n{}: day {} / {}\n{} / {}\n", plan.contact.plan_label(), plan.day, plan.period.label(), if plan.contact == Contact::Marina { "Answer on this phone" } else { "Visit: use E at the front door" }, match plan.status { CommitmentStatus::Scheduled => "Scheduled", CommitmentStatus::Completed => "Kept", CommitmentStatus::Missed => "Missed" })); }
            text.push_str("\nCalls and visits cost 1 PA: relationships +8, stress -6. Reading is free. An ordinary Rafa phone call does not replace a visit. Answer during the scheduled afternoon; early sleep or leaving the period misses it. You can still call after a missed plan, but cannot rewrite it.");
            (text, vec![action(if c.commitment_due(Contact::Marina) { "Answer Marina's scheduled call: 1 PA" } else { "Call Marina: 1 PA" }, Action::Call(Contact::Marina)), action("Call Rafa: 1 PA (not a visit)", Action::Call(Contact::Rafa))])
        }
        Support => {
            let mut choices = vec![action("Care conversation: 1 PA / stress -10, urge -6", Action::CareConversation), action("Financial planning: 1 PA / stress -6", Action::FinancialPlanning), nav("Daily wagering cap / free", DailyCap)];
            if !c.support.app_blocked_today { choices.push(action("Block betting until morning / free", Action::BlockBetting)); }
            choices.extend([nav("Contacts / information is free", Contacts), nav("Bank / planning information is free", Bank), nav("Financial and betting history / free", History)]);
            (format!("Fictional care desk\n\nName today's pressure and choose a manageable next step. Care is gradual, never a cure. All people and services here are invented.\n\nPlanning: review your balance, bill deadlines and food needs before choosing where money goes. Reading is always free, including at zero PA, during blackout or betting blocks.\n\nCare conversations: {}\nPlanning actions: {}\nBetting app: {}", c.support.care_actions, c.support.planning_actions, if c.support.app_blocked_today { "Blocked until morning" } else { "Available" }), choices)
        }
        DailyCap => {
            let pending = c.support.pending_cap.as_ref().map(|cap| format!("{} on day {} morning", cap.cents.map(reais).unwrap_or_else(|| "No cap".into()), cap.effective_day)).unwrap_or_else(|| "None".into());
            (format!("Current cap: {}\nPending: {}\nGross staked today: {}\n\nCaps count all stakes, including earlier bets; payouts do not restore allowance. R$0 blocks every stake.\n\nNew or lower caps apply now. Increases and removal wait until tomorrow morning. Applying the current or a lower cap cancels a pending change.\n\nSelecting an amount is free. Apply confirms the cap; no borrowing.", c.support.daily_cap_cents.map(reais).unwrap_or_else(|| "None".into()), pending, reais(c.daily_finances(c.clock.day).map_or(0, |f| f.gross_bets_cents))), vec![])
        }
        Food => {
            let mut choices = vec![];
            if c.require_kitchen().is_ok() {
                choices.extend([action("Groceries: R$40 / 3 portions", Action::Groceries), action("Cook and eat: 1 portion", Action::Cook)]);
            }
            choices.push(action("Order a meal: R$20", Action::OrderFood));
            (format!("{} portions at home\nHunger: {} / 100\nBalance: {}\n\nGroceries and cooking need the fridge, stove and cookware. Cooking uses one stored portion.\n\nPrepared-food delivery is always available, including after kitchen sales. Each option takes 1 PA.", c.household.food_portions, c.attributes.hunger, reais(c.balance_cents)), choices)
        }
    }
}

fn home_reminder(c: &Campaign) -> String {
    if !c.has_power() {
        return "Power off: see Bank".into();
    }
    if c.bills
        .iter()
        .any(|bill| bill.paid_day.is_none() && c.clock.day > bill.due_day)
    {
        return "Overdue bill: see Bank".into();
    }
    c.commitments
        .iter()
        .filter(|plan| plan.status == CommitmentStatus::Scheduled)
        .min_by_key(|plan| (plan.day, plan.period))
        .map(|plan| format!("{}: day {} afternoon", plan.contact.label(), plan.day))
        .unwrap_or_else(|| "Review Contacts / messages".into())
}

fn situation(c: &Campaign) -> &'static str {
    let unpaid = |kind| {
        c.bills
            .iter()
            .any(|bill| bill.kind == kind && bill.paid_day.is_none())
    };
    match c.clock.day {
        1 => "MeFood shifts start today. Marina calls tomorrow afternoon.",
        2 => "Marina's call is planned for this afternoon on the phone.",
        3 if unpaid(BillKind::Electricity) => {
            "Rafa visits tomorrow afternoon. Electricity is due end of day 4."
        }
        3 => "Electricity is paid. Rafa visits tomorrow afternoon.",
        4 if unpaid(BillKind::Electricity) => {
            "Rafa visits at the front door this afternoon. Electricity is due tonight."
        }
        4 => "Electricity is paid. Rafa visits at the front door this afternoon.",
        5 if unpaid(BillKind::Electricity) => {
            "Electricity warning: pay before day 6 to avoid a cutoff."
        }
        5 => "Electricity is paid. Review the rent deadline in Bank.",
        6 if !c.has_power() => "Power is off. Bank payment restores it; phone and door still work.",
        6 if !unpaid(BillKind::Rent) => "Rent is paid. Marina calls tomorrow afternoon.",
        6 if c.support.rent_negotiated => {
            "Rent is due end of day 10. Marina calls tomorrow afternoon."
        }
        6 => "Rent is due end of day 7. Marina calls tomorrow afternoon.",
        7 => "Marina calls this afternoon. Check the rent deadline in Bank.",
        8 if unpaid(BillKind::Rent) && !c.support.rent_negotiated => {
            "Rent warning: pay or negotiate today to avoid housing loss tomorrow."
        }
        8 if unpaid(BillKind::Rent) => {
            "Rent is extended to end of day 10. Rafa visits tomorrow afternoon."
        }
        8 => "Rent is paid. Rafa visits tomorrow afternoon.",
        9 => "Rafa visits at the front door this afternoon. There is no MeFood shift.",
        _ => "Day 10: review your finances, connections and care steps before sleep.",
    }
}

fn contact_message(c: &Campaign, contact: Contact) -> String {
    let missed = c
        .commitments
        .iter()
        .filter(|plan| plan.contact == contact && plan.status == CommitmentStatus::Missed)
        .count();
    let mut message = if missed > 0 {
        format!(
            "We missed {missed} planned {}. We can still talk; let's make time for the next step.",
            if contact == Contact::Marina {
                "call(s)"
            } else {
                "visit(s)"
            }
        )
    } else if c.attributes.relationships >= 80 {
        "I'm glad we've kept making time for each other.".into()
    } else if c.attributes.relationships < 45 {
        "We've been distant. A conversation can help us reconnect gradually.".into()
    } else if contact == Contact::Marina {
        "I'm your sister. You can call me about the day, even when money is tight.".into()
    } else {
        "I'm nearby. Our afternoon visits are at your front door; you can also phone me.".into()
    };
    if contact == Contact::Marina {
        let unpaid: i64 = c
            .bills
            .iter()
            .filter(|bill| bill.paid_day.is_none())
            .map(|bill| bill.amount_cents)
            .sum();
        if unpaid == 0 {
            message.push_str(&format!(
                " Both bills are paid; {} remains for meals and the days ahead.",
                reais(c.balance_cents)
            ));
        } else {
            message.push_str(&format!(
                " Your balance is {}; unpaid bills total {}. Check their deadlines in Bank.",
                reais(c.balance_cents),
                reais(unpaid)
            ));
        }
    } else if !c.betting.history.is_empty() {
        let net: i128 = c
            .betting
            .history
            .iter()
            .map(|round| i128::from(round.net_cents))
            .sum();
        message.push_str(&format!(
            " You've placed {} bets, net {}R${}.{:02}.",
            c.betting.history.len(),
            if net < 0 { "-" } else { "" },
            net.unsigned_abs() / 100,
            net.unsigned_abs() % 100,
        ));
        if c.betting
            .history
            .iter()
            .any(|round| round.session_round >= 4)
        {
            message.push_str(" Some sessions continued beyond three rounds. We can spend time together without another bet.");
        } else {
            message.push_str(" We can talk without placing another bet.");
        }
    } else {
        message.push_str(" There is no betting in your history. Our plans do not require it.");
    }
    if c.support.care_actions > 0 || c.support.planning_actions > 0 {
        message.push_str(" You've made care or planning steps. Keep building on them; one talk does not fix everything.");
    }
    message
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
fn button(
    parent: &mut ChildSpawnerCommands,
    font: &Handle<Font>,
    label: &str,
    page: PhonePage,
    command: PhoneCommand,
) {
    parent
        .spawn((
            Button,
            PhoneButton(page, command),
            BackgroundColor(BUTTON),
            Node {
                width: percent(100),
                min_height: px(if page.is_game() { 44.0 } else { 53.0 }),
                padding: UiRect::axes(
                    px(if page.is_game() { 7.0 } else { 14.0 }),
                    px(if page.is_game() { 7.0 } else { 11.0 }),
                ),
                align_items: AlignItems::Center,
                flex_shrink: 0.0,
                ..default()
            },
        ))
        .observe(queue_press)
        .observe(
            |over: On<Pointer<Over>>,
             mut colors: Query<&mut BackgroundColor, With<PhoneButton>>| {
                if let Ok(mut c) = colors.get_mut(over.entity) {
                    c.0 = Color::srgb(0.25, 0.34, 0.32);
                }
            },
        )
        .observe(
            |out: On<Pointer<Out>>, mut colors: Query<&mut BackgroundColor, With<PhoneButton>>| {
                if let Ok(mut c) = colors.get_mut(out.entity) {
                    c.0 = BUTTON;
                }
            },
        )
        .with_children(|parent| {
            text(
                parent,
                font,
                label,
                if page.is_game() { 21.0 } else { 25.0 },
                INK,
            )
        });
}

fn present(
    mut commands: Commands,
    phone: Res<Phone>,
    store: Res<CampaignStore>,
    assets: Res<PhoneAssets>,
    existing: Query<Entity, With<PhoneUi>>,
    mut previous: Local<String>,
) {
    let Some(c) = store.campaign() else { return };
    let (copy, choices) = phone_content(&phone, c);
    let signature = format!(
        "{:?}{:?}{copy}{choices:?}{}{}{:?}{}{}{:?}{}{:?}{}{:?}",
        phone.page,
        c.clock,
        phone.feedback,
        store.notice,
        phone.stake_entry,
        phone.stake_cents,
        animation_frame(&phone),
        phone.settled_round,
        c.balance_cents,
        c.betting.session,
        phone.cap_cents,
        phone.cap_entry
    );
    if *previous == signature && !existing.is_empty() {
        return;
    }
    *previous = signature;
    for entity in &existing {
        commands.entity(entity).despawn();
    }
    commands
        .spawn((
            PhoneUi,
            UiTargetCamera(assets.camera),
            BackgroundColor(Color::srgb(0.055, 0.085, 0.085)),
            Node {
                width: percent(100),
                height: percent(100),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(px(18)),
                row_gap: px(12),
                ..default()
            },
        ))
        .with_children(|root| {
            root.spawn(Node {
                flex_direction: FlexDirection::Column,
                row_gap: px(6),
                flex_shrink: 0.0,
                ..default()
            })
            .with_children(|header| {
                text(
                    header,
                    &assets.font,
                    format!(
                        "Day {} / {} / {} PA",
                        c.clock.day,
                        c.clock.period().label(),
                        c.clock.remaining_pa()
                    ),
                    20.0,
                    MUTED,
                );
                text(header, &assets.font, phone.page.title(), 36.0, INK);
                if phone.page.is_game() {
                    text(
                        header,
                        &assets.font,
                        format!(
                            "Balance {} / Stake {}",
                            reais(c.balance_cents),
                            reais(phone.stake_cents)
                        ),
                        21.0,
                        INK,
                    );
                    let session = c
                        .betting
                        .session
                        .as_ref()
                        .map(|s| {
                            format!(
                                "Session {} / round {} / {} paid left",
                                s.id, s.rounds, s.paid_rounds_remaining
                            )
                        })
                        .unwrap_or_else(|| "New session / next bet: 1 PA".into());
                    text(header, &assets.font, session, 19.0, MUTED);
                }
            });
            if !store.notice.is_empty() {
                text(
                    root,
                    &assets.font,
                    &store.notice,
                    20.0,
                    Color::srgb(1.0, 0.79, 0.55),
                );
                button(
                    root,
                    &assets.font,
                    "Retry saving",
                    phone.page,
                    PhoneCommand::RetrySave,
                );
            }
            if !phone.feedback.is_empty() {
                text(
                    root,
                    &assets.font,
                    &phone.feedback,
                    21.0,
                    Color::srgb(1.0, 0.83, 0.59),
                );
            }
            root.spawn((
                PhoneScroll,
                ScrollPosition::default(),
                Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: px(9),
                    flex_grow: 1.0,
                    min_height: px(0),
                    overflow: Overflow::scroll_y(),
                    ..default()
                },
            ))
            .with_children(|body| {
                if phone.page.is_game() {
                    game_controls(body, &assets.font, &phone);
                }
                if phone.page == PhonePage::DailyCap {
                    cap_controls(body, &assets.font, &phone);
                }
                if phone.page.is_game()
                    || matches!(phone.page, PhonePage::DailyCap | PhonePage::History)
                {
                    for (index, (label, command)) in choices.iter().enumerate() {
                        button(
                            body,
                            &assets.font,
                            &format!("{}  {label}", index + 1),
                            phone.page,
                            *command,
                        );
                    }
                    if phone.page.is_game() {
                        game_modes(body, &assets.font, phone.page);
                    }
                }
                body.spawn(Node {
                    flex_direction: FlexDirection::Column,
                    flex_shrink: 0.0,
                    margin: UiRect::bottom(px(10)),
                    ..default()
                })
                .with_children(|copy_node| {
                    text(
                        copy_node,
                        &assets.font,
                        copy,
                        if phone.page.is_game() { 22.0 } else { 25.0 },
                        INK,
                    )
                });
                for (index, (label, command)) in choices.iter().enumerate().filter(|_| {
                    !phone.page.is_game()
                        && !matches!(phone.page, PhonePage::DailyCap | PhonePage::History)
                }) {
                    button(
                        body,
                        &assets.font,
                        &format!("{}  {label}", index + 1),
                        phone.page,
                        *command,
                    );
                }
            });
            root.spawn(Node {
                flex_direction: FlexDirection::Row,
                column_gap: px(8),
                flex_shrink: 0.0,
                ..default()
            })
            .with_children(|nav| {
                nav.spawn(Node {
                    flex_grow: 1.0,
                    flex_basis: px(0),
                    ..default()
                })
                .with_children(|b| {
                    button(
                        b,
                        &assets.font,
                        "Home",
                        phone.page,
                        PhoneCommand::Page(PhonePage::Home),
                    )
                });
                nav.spawn(Node {
                    flex_grow: 1.0,
                    flex_basis: px(0),
                    ..default()
                })
                .with_children(|b| {
                    button(
                        b,
                        &assets.font,
                        "Back",
                        phone.page,
                        PhoneCommand::Page(phone.page.back()),
                    )
                });
                nav.spawn(Node {
                    flex_grow: 1.0,
                    flex_basis: px(0),
                    ..default()
                })
                .with_children(|b| {
                    button(b, &assets.font, "Close", phone.page, PhoneCommand::Close)
                });
            });
            text(
                root,
                &assets.font,
                if cfg!(target_arch = "wasm32") {
                    "H: Home / Backspace: Back\nTab: Lower / Wheel: Scroll / P: Pause\nEsc: Release mouse and pause"
                } else {
                    "H: Home / Backspace: Back\nTab / Esc: Lower / Wheel: Scroll / P: Pause"
                },
                18.0,
                MUTED,
            );
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

fn sync_view_msaa(
    world: Query<&Msaa, (With<super::camera::MainCamera>, Without<PhoneCamera>)>,
    mut foreground: Query<&mut Msaa, (With<PhoneCamera>, Without<super::camera::MainCamera>)>,
) {
    let Ok(msaa) = world.single() else { return };
    // ClearNone must share the world's intermediate texture; Bevy keys it by MSAA too.
    for mut foreground in &mut foreground {
        foreground.set_if_neq(*msaa);
    }
}

fn animate(
    time: Res<Time<Real>>,
    state: Res<State<GameState>>,
    mut phone: ResMut<Phone>,
    mut roots: Query<(&mut Transform, &mut Visibility), With<PhoneRoot>>,
    mut cameras: Query<&mut Camera, Or<(With<PhoneCamera>, With<TextureCamera>)>>,
    windows: Query<&Window, With<PrimaryWindow>>,
) {
    let playing = *state.get() == GameState::Playing;
    let target = if phone.opened { 1.0 } else { 0.0 };
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
mod tests {
    use super::super::campaign_save::SnapshotState;
    use super::*;

    #[test]
    fn phone_history_links_to_saved_reports_and_browsing_keeps_campaign_identical() {
        let mut c = Campaign::new(5);
        c.apply(Action::Work).unwrap();
        c.apply(Action::Sleep).unwrap();
        c.apply(Action::AcknowledgeReport).unwrap();
        let (history, choices) = page_content(PhonePage::History, &c);
        assert_eq!(history, super::super::routine::history_text(&c));
        assert_eq!(choices[0].1, PhoneCommand::Page(PhonePage::Reports));
        assert_eq!(PhonePage::Reports.back(), PhonePage::History);
        let (reports, choices) = page_content(PhonePage::Reports, &c);
        assert!(reports.contains("DAY 1 - DAILY REPORT"));
        assert!(reports.contains("PA used by actions: 2"));
        assert!(choices.is_empty());
        let mut app = app();
        app.insert_resource(CampaignStore::for_test(
            Some(c.clone()),
            SnapshotState::Protected,
        ));
        opened(&mut app, PhonePage::History);
        key(&mut app, KeyCode::Digit1);
        assert_eq!(app.world().resource::<Phone>().page, PhonePage::Reports);
        key(&mut app, KeyCode::Enter);
        key(&mut app, KeyCode::Backspace);
        assert_eq!(app.world().resource::<Phone>().page, PhonePage::History);
        assert_eq!(app.world().resource::<CampaignStore>().campaign(), Some(&c));
    }

    fn app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .insert_resource(State::new(GameState::Playing))
            .init_resource::<NextState<GameState>>()
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<ButtonInput<MouseButton>>()
            .init_resource::<Assets<Mesh>>()
            .add_message::<MouseWheel>()
            .add_message::<KeyboardInput>()
            .add_message::<PhoneAppClosed>()
            .insert_resource(CampaignStore::for_test(
                Some(Campaign::new(9)),
                SnapshotState::Protected,
            ))
            .init_resource::<Phone>()
            .init_resource::<GameCursor>()
            .add_plugins(super::super::routine::RoutinePlugin)
            .add_systems(PreUpdate, input.after(OverlayInput))
            .add_systems(Update, (dispatch, end_session).chain())
            .add_observer(reset);
        app.world_mut().spawn((
            Window {
                focused: true,
                ..default()
            },
            PrimaryWindow,
        ));
        app.world_mut().spawn(Player::default());
        app
    }
    fn amount_input(app: &mut App, key: KeyCode, text: Option<&str>, repeat: bool) {
        let window = app
            .world_mut()
            .query_filtered::<Entity, With<PrimaryWindow>>()
            .single(app.world())
            .unwrap();
        app.world_mut().write_message(KeyboardInput {
            key_code: key,
            logical_key: match key {
                KeyCode::Backspace => Key::Backspace,
                KeyCode::Enter => Key::Enter,
                _ => Key::Character(text.unwrap().into()),
            },
            state: ButtonState::Pressed,
            text: text.map(Into::into),
            repeat,
            window,
        });
    }
    fn key(app: &mut App, key: KeyCode) {
        let digit = match key {
            KeyCode::Digit0 => Some("0"),
            KeyCode::Digit1 => Some("1"),
            KeyCode::Digit2 => Some("2"),
            KeyCode::Digit3 => Some("3"),
            KeyCode::Digit4 => Some("4"),
            KeyCode::Digit5 => Some("5"),
            KeyCode::Digit6 => Some("6"),
            KeyCode::Digit7 => Some("7"),
            KeyCode::Digit8 => Some("8"),
            KeyCode::Digit9 => Some("9"),
            _ => None,
        };
        if digit.is_some() || matches!(key, KeyCode::Backspace | KeyCode::Enter) {
            amount_input(app, key, digit, false);
        }
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(key);
        app.update();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .reset(key);
    }
    fn opened(app: &mut App, page: PhonePage) {
        let mut phone = app.world_mut().resource_mut::<Phone>();
        phone.opened = true;
        phone.raise = 1.0;
        phone.page = page;
        app.world_mut().resource_mut::<GameplayOverlay>().active = Some(OverlayKind::Phone);
    }

    #[test]
    fn household_sales_are_under_status_and_each_confirmation_explains_permanent_loss() {
        let mut c = Campaign::new(5);
        let (_, status) = page_content(PhonePage::Status, &c);
        assert!(
            status
                .iter()
                .any(|(_, cmd)| *cmd == PhoneCommand::Page(PhonePage::Household))
        );
        assert!(page_content(PhonePage::Household, &c).1.is_empty());
        c.attributes.stress = 70;
        let (_, choices) = page_content(PhonePage::Household, &c);
        assert_eq!(choices.len(), 6);
        assert!(
            choices
                .iter()
                .all(|(_, cmd)| matches!(cmd, PhoneCommand::Page(PhonePage::ConfirmSale(_))))
        );
        for item in super::super::household::HOUSEHOLD_ITEMS
            .into_iter()
            .chain([HouseholdItem::Pipoca])
        {
            let (copy, choices) = page_content(PhonePage::ConfirmSale(item), &c);
            assert!(
                copy.contains("permanently")
                    && copy.contains("1 PA")
                    && copy.contains("cannot be undone")
            );
            assert_eq!(choices[0].1, PhoneCommand::Page(PhonePage::Household));
            if item == HouseholdItem::Fridge {
                assert!(copy.contains("ALL 3 stored portions"));
            }
            if item == HouseholdItem::Pipoca {
                assert!(copy.contains("ENDS THE CAMPAIGN"));
                assert_eq!(choices.len(), 1);
            }
        }
        assert_eq!(PhonePage::Household.back(), PhonePage::Status);
        c.apply(Action::Sell(HouseholdItem::Stove)).unwrap();
        assert_eq!(
            page_content(PhonePage::Food, &c).1,
            vec![(
                "Order a meal: R$20".into(),
                PhoneCommand::Apply(Action::OrderFood)
            )]
        );
        assert_eq!(
            PhonePage::ConfirmSale(HouseholdItem::Tv).back(),
            PhonePage::Household
        );
    }

    #[test]
    fn sale_navigation_cancel_held_keys_and_stale_commands_never_auto_confirm() {
        let mut app = app();
        let mut c = Campaign::new(5);
        c.attributes.urge = 70;
        app.insert_resource(CampaignStore::for_test(
            Some(c.clone()),
            SnapshotState::Protected,
        ));
        opened(&mut app, PhonePage::Household);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Digit1);
        app.update();
        assert_eq!(
            app.world().resource::<Phone>().page,
            PhonePage::ConfirmSale(HouseholdItem::Tv)
        );
        assert_eq!(
            app.world().resource::<CampaignStore>().campaign().unwrap(),
            &c
        );
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .clear();
        app.update();
        assert_eq!(
            app.world().resource::<CampaignStore>().campaign().unwrap(),
            &c
        );
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .reset_all();
        key(&mut app, KeyCode::Digit1);
        assert_eq!(app.world().resource::<Phone>().page, PhonePage::Household);
        assert_eq!(
            app.world().resource::<CampaignStore>().campaign().unwrap(),
            &c
        );
        // An old confirmation queued while the Household page is current is rejected.
        {
            let mut phone = app.world_mut().resource_mut::<Phone>();
            phone.pointer = Some(Vec2::ONE);
            phone.pending = Some((
                PhonePage::ConfirmSale(HouseholdItem::Tv),
                PhoneCommand::Apply(Action::Sell(HouseholdItem::Tv)),
            ));
        }
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        app.update();
        assert_eq!(
            app.world().resource::<CampaignStore>().campaign().unwrap(),
            &c
        );
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .reset_all();
        key(&mut app, KeyCode::Digit1);
        key(&mut app, KeyCode::Digit2);
        c.apply(Action::Sell(HouseholdItem::Tv)).unwrap();
        assert_eq!(
            app.world().resource::<CampaignStore>().campaign().unwrap(),
            &c
        );
        assert_eq!(app.world().resource::<Phone>().page, PhonePage::Household);
        assert!(!app.world().resource::<CampaignStore>().notice.is_empty());
    }

    #[test]
    fn fridge_confirm_destroys_only_disclosed_portions_and_dog_confirm_immediately_stops_phone() {
        let mut app = app();
        let mut c = Campaign::new(5);
        c.attributes.stress = 70;
        app.insert_resource(CampaignStore::for_test(
            Some(c.clone()),
            SnapshotState::Protected,
        ));
        opened(&mut app, PhonePage::ConfirmSale(HouseholdItem::Fridge));
        key(&mut app, KeyCode::Digit1);
        assert_eq!(
            app.world().resource::<CampaignStore>().campaign().unwrap(),
            &c
        );
        key(&mut app, KeyCode::Digit2);
        assert_eq!(
            app.world().resource::<Phone>().page,
            PhonePage::ConfirmSale(HouseholdItem::Fridge)
        );
        key(&mut app, KeyCode::Digit2);
        let stored = app.world().resource::<CampaignStore>().campaign().unwrap();
        assert_eq!(
            (
                stored.household.food_portions,
                stored.balance_cents,
                stored.clock.pa_spent
            ),
            (0, 24_000, 1)
        );
        for item in super::super::household::HOUSEHOLD_ITEMS {
            c.apply(Action::Sell(item)).unwrap();
        }
        c.apply(Action::Sleep).unwrap();
        c.apply(Action::AcknowledgeReport).unwrap();
        c.attributes.stress = 0;
        c.attributes.urge = 0;
        app.insert_resource(CampaignStore::for_test(Some(c), SnapshotState::Protected));
        opened(&mut app, PhonePage::Household);
        key(&mut app, KeyCode::Digit1);
        assert_eq!(
            app.world().resource::<Phone>().page,
            PhonePage::ConfirmSale(HouseholdItem::Pipoca)
        );
        key(&mut app, KeyCode::Digit2);
        assert_eq!(
            app.world()
                .resource::<CampaignStore>()
                .campaign()
                .unwrap()
                .stage,
            CampaignStage::Ended(Ending::DogSale)
        );
        assert!(!app.world().resource::<Phone>().opened);
        assert!(app.world().resource::<GameplayOverlay>().campaign_modal);
    }

    #[test]
    fn cursor_conversion_tracks_canvas_scale_dpr_resize_and_native_identity() {
        for (input, viewport) in [
            (Vec2::new(1280.0, 720.0), Vec2::new(1600.0, 900.0)),
            (Vec2::new(960.0, 540.0), Vec2::new(1200.0, 675.0)),
            (Vec2::new(960.0, 540.0), Vec2::new(1920.0, 1080.0)),
            (Vec2::new(960.0, 540.0), Vec2::new(960.0, 540.0)),
        ] {
            let normalized = Vec2::new(0.61, 0.42);
            assert!(
                scale_cursor(normalized * input, input, viewport)
                    .unwrap()
                    .distance(normalized * viewport)
                    < 0.001
            );
        }
        assert!(scale_cursor(Vec2::ONE, Vec2::ZERO, Vec2::ONE).is_none());
        assert!(scale_cursor(Vec2::splat(f32::NAN), Vec2::ONE, Vec2::ONE).is_none());
    }

    #[test]
    fn ray_maps_rotated_screen_corners_and_rejects_outside_behind_and_parallel() {
        let transform = Transform::from_xyz(0.12, -0.03, -0.8).with_rotation(Quat::from_euler(
            EulerRot::YXZ,
            0.15,
            -0.1,
            0.04,
        ));
        let screen = GlobalTransform::from(transform);
        for uv in [
            Vec2::splat(0.5),
            Vec2::new(0.01, 0.01),
            Vec2::new(0.99, 0.99),
        ] {
            let point = screen.transform_point(Vec3::new(
                (uv.x - 0.5) * SCREEN_METERS.x,
                (0.5 - uv.y) * SCREEN_METERS.y,
                0.0,
            ));
            let ray = Ray3d::new(Vec3::ZERO, Dir3::new(point).unwrap());
            assert!(
                screen_coordinates(ray, &screen)
                    .unwrap()
                    .distance(uv * SCREEN_SIZE)
                    < 0.001
            );
        }
        for uv in [Vec2::new(-0.01, 0.5), Vec2::new(0.5, 1.01)] {
            let point = screen.transform_point(Vec3::new(
                (uv.x - 0.5) * SCREEN_METERS.x,
                (0.5 - uv.y) * SCREEN_METERS.y,
                0.0,
            ));
            assert!(
                screen_coordinates(Ray3d::new(Vec3::ZERO, Dir3::new(point).unwrap()), &screen)
                    .is_none()
            );
        }
        let flat = GlobalTransform::from_translation(Vec3::NEG_Z);
        assert!(screen_coordinates(Ray3d::new(Vec3::ZERO, Dir3::Z), &flat).is_none());
        assert!(screen_coordinates(Ray3d::new(Vec3::ZERO, Dir3::X), &flat).is_none());
    }

    #[test]
    fn tab_browsing_escape_and_app_exit_never_spend_campaign_resources() {
        let mut app = app();
        let original = app
            .world()
            .resource::<CampaignStore>()
            .campaign()
            .unwrap()
            .clone();
        key(&mut app, KeyCode::Tab);
        assert!(app.world().resource::<Phone>().opened);
        assert!(app.world().resource::<GameplayOverlay>().blocks_world());
        key(&mut app, KeyCode::Digit4);
        assert_eq!(app.world().resource::<Phone>().page, PhonePage::Status);
        key(&mut app, KeyCode::KeyH);
        key(&mut app, KeyCode::Digit1);
        key(&mut app, KeyCode::Digit2);
        assert_eq!(app.world().resource::<Phone>().page, PhonePage::Slots);
        key(&mut app, KeyCode::Backspace);
        assert_eq!(app.world().resource::<Phone>().page, PhonePage::Betting);
        assert_eq!(app.world().resource::<Messages<PhoneAppClosed>>().len(), 0);
        key(&mut app, KeyCode::KeyH);
        assert_eq!(app.world().resource::<Messages<PhoneAppClosed>>().len(), 1);
        key(&mut app, KeyCode::Escape);
        assert!(!app.world().resource::<Phone>().opened);
        assert!(app.world().resource::<GameplayOverlay>().escape_consumed);
        assert_eq!(
            app.world().resource::<CampaignStore>().campaign(),
            Some(&original)
        );
    }

    #[test]
    fn focus_loss_and_pause_preserve_page_without_exiting_the_app() {
        let mut app = app();
        opened(&mut app, PhonePage::Roulette);
        let window = app
            .world_mut()
            .query_filtered::<Entity, With<PrimaryWindow>>()
            .single(app.world())
            .unwrap();
        app.world_mut().get_mut::<Window>(window).unwrap().focused = false;
        key(&mut app, KeyCode::Tab);
        assert!(app.world().resource::<Phone>().opened);
        assert_eq!(app.world().resource::<Phone>().page, PhonePage::Roulette);
        app.insert_resource(State::new(GameState::Menu));
        app.update();
        assert_eq!(app.world().resource::<Messages<PhoneAppClosed>>().len(), 0);
        app.insert_resource(State::new(GameState::Playing));
        app.world_mut().get_mut::<Window>(window).unwrap().focused = true;
        app.update();
        assert_eq!(app.world().resource::<Phone>().page, PhonePage::Roulette);
        assert_eq!(
            app.world().resource::<GameplayOverlay>().active,
            Some(OverlayKind::Phone)
        );
    }

    #[test]
    fn stale_dismissed_unfocused_paused_and_outside_presses_do_not_apply_food() {
        for scenario in 0..6 {
            let mut app = app();
            opened(&mut app, PhonePage::Food);
            let original = app
                .world()
                .resource::<CampaignStore>()
                .campaign()
                .unwrap()
                .clone();
            {
                let mut phone = app.world_mut().resource_mut::<Phone>();
                phone.pointer = Some(Vec2::splat(50.0));
                phone.pending = Some((
                    if scenario == 0 {
                        PhonePage::Status
                    } else {
                        PhonePage::Food
                    },
                    PhoneCommand::Apply(Action::Cook),
                ));
                if scenario == 1 {
                    phone.pointer = None;
                }
            }
            app.world_mut()
                .resource_mut::<ButtonInput<MouseButton>>()
                .press(MouseButton::Left);
            match scenario {
                2 => app
                    .world_mut()
                    .resource_mut::<ButtonInput<KeyCode>>()
                    .press(KeyCode::Escape),
                3 => {
                    let mut q = app
                        .world_mut()
                        .query_filtered::<&mut Window, With<PrimaryWindow>>();
                    q.single_mut(app.world_mut()).unwrap().focused = false;
                }
                4 => {
                    *app.world_mut().resource_mut::<NextState<GameState>>() =
                        NextState::Pending(GameState::Menu)
                }
                5 => {
                    app.world_mut()
                        .resource_mut::<ButtonInput<MouseButton>>()
                        .reset(MouseButton::Left);
                    app.world_mut()
                        .resource_mut::<ButtonInput<MouseButton>>()
                        .release(MouseButton::Left);
                }
                _ => {}
            }
            app.update();
            assert_eq!(
                app.world().resource::<CampaignStore>().campaign(),
                Some(&original),
                "scenario {scenario}"
            );
        }
    }

    #[test]
    fn leaving_screen_preserves_fresh_home_and_back_keyboard_navigation() {
        for (page, shortcut, expected) in [
            (PhonePage::Food, KeyCode::KeyH, PhonePage::Home),
            (PhonePage::Roulette, KeyCode::Backspace, PhonePage::Betting),
        ] {
            let mut app = app();
            opened(&mut app, page);
            app.add_message::<PointerInput>()
                .insert_resource(PhoneAssets {
                    camera: Entity::PLACEHOLDER,
                    target: Handle::default(),
                    font: Handle::default(),
                })
                .add_systems(PreUpdate, drive_pointer.after(input));
            app.world_mut().resource_mut::<Phone>().pointer = Some(Vec2::splat(50.0));
            key(&mut app, shortcut);
            assert_eq!(app.world().resource::<Phone>().page, expected);
            assert!(app.world().resource::<Phone>().pointer.is_none());
            assert_eq!(
                app.world()
                    .resource::<CampaignStore>()
                    .campaign()
                    .unwrap()
                    .clock
                    .pa_spent,
                0
            );
        }
    }

    #[test]
    fn outside_screen_cancels_held_virtual_pointer_and_never_reuses_its_position() {
        let mut app = app();
        opened(&mut app, PhonePage::Food);
        app.add_message::<PointerInput>()
            .insert_resource(PhoneAssets {
                camera: Entity::PLACEHOLDER,
                target: Handle::default(),
                font: Handle::default(),
            })
            .add_systems(PreUpdate, drive_pointer.after(input));
        app.world_mut().resource_mut::<Phone>().pointer = Some(Vec2::new(50.0, 150.0));
        app.world_mut().resource_mut::<Phone>().pending =
            Some((PhonePage::Food, PhoneCommand::Apply(Action::Cook)));
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        app.update();
        let mut cursor = bevy::ecs::message::MessageCursor::<PointerInput>::default();
        let events = app.world().resource::<Messages<PointerInput>>();
        assert!(
            cursor
                .read(events)
                .any(|event| matches!(event.action, PointerAction::Cancel))
        );
        assert!(app.world().resource::<Phone>().pointer.is_none());
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .reset(MouseButton::Left);
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .release(MouseButton::Left);
        app.update();
        assert!(
            !cursor
                .read(app.world().resource::<Messages<PointerInput>>())
                .any(|event| matches!(
                    event.action,
                    PointerAction::Press(_) | PointerAction::Release(_)
                ))
        );
        assert_eq!(
            app.world()
                .resource::<CampaignStore>()
                .campaign()
                .unwrap()
                .clock
                .pa_spent,
            0
        );
    }

    #[test]
    fn food_uses_authoritative_actions_and_report_or_ending_blocks_phone() {
        let mut app = app();
        opened(&mut app, PhonePage::Food);
        key(&mut app, KeyCode::Digit2);
        let campaign = app.world().resource::<CampaignStore>().campaign().unwrap();
        assert_eq!(campaign.clock.pa_spent, 1);
        assert_eq!(campaign.household.food_portions, 2);
        assert_eq!(campaign.attributes.hunger, 0);
        assert!(
            app.world()
                .resource::<CampaignStore>()
                .notice
                .contains("session")
        );
        for stage in [
            CampaignStage::DailyReport,
            CampaignStage::Ended(Ending::DayTen),
        ] {
            let mut campaign = Campaign::new(3);
            if stage == CampaignStage::DailyReport {
                campaign.apply(Action::Sleep).unwrap();
            } else {
                campaign.support.rent_negotiated = true;
                campaign.bills[1].due_day = 10;
                for _ in 1..=10 {
                    campaign.apply(Action::Sleep).unwrap();
                    campaign.apply(Action::AcknowledgeReport).unwrap();
                }
            }
            app.insert_resource(CampaignStore::for_test(
                Some(campaign),
                SnapshotState::Protected,
            ));
            key(&mut app, KeyCode::Tab);
            assert!(!app.world().resource::<Phone>().opened);
            assert!(app.world().resource::<GameplayOverlay>().campaign_modal);
        }
    }

    #[test]
    fn foreground_matches_world_msaa_without_changing_world_or_texture_camera() {
        let mut app = App::new();
        app.add_systems(Update, sync_view_msaa);
        let main = app
            .world_mut()
            .spawn((
                super::super::camera::MainCamera,
                Camera3d::default(),
                Msaa::Sample4,
            ))
            .id();
        let view = app
            .world_mut()
            .spawn((PhoneCamera, Camera3d::default(), Msaa::Off))
            .id();
        let texture = app
            .world_mut()
            .spawn((TextureCamera, Camera2d, Msaa::Off))
            .id();
        for mode in [Msaa::Sample4, Msaa::Off, Msaa::Sample4] {
            *app.world_mut().get_mut::<Msaa>(main).unwrap() = mode;
            app.update();
            assert_eq!(*app.world().get::<Msaa>(main).unwrap(), mode);
            assert_eq!(*app.world().get::<Msaa>(view).unwrap(), mode);
            assert_eq!(*app.world().get::<Msaa>(texture).unwrap(), Msaa::Off);
        }
    }

    #[test]
    fn camera_animation_queries_initialize_and_disable_both_auxiliary_cameras() {
        let mut app = app();
        app.init_resource::<Assets<StandardMaterial>>()
            .insert_resource(PhoneAssets {
                camera: Entity::PLACEHOLDER,
                target: Handle::default(),
                font: Handle::default(),
            })
            .add_systems(Update, (tag_model, scroll, present, animate).chain());
        let view = app
            .world_mut()
            .spawn((PhoneCamera, Camera3d::default()))
            .id();
        let texture = app.world_mut().spawn((TextureCamera, Camera2d)).id();
        let world_camera = app
            .world_mut()
            .spawn((
                super::super::camera::MainCamera,
                Camera3d::default(),
                IsDefaultUiCamera,
            ))
            .id();
        app.world_mut()
            .spawn((PhoneRoot, Transform::default(), Visibility::Hidden));
        app.update();
        assert!(!app.world().get::<Camera>(view).unwrap().is_active);
        assert!(!app.world().get::<Camera>(texture).unwrap().is_active);
        assert!(app.world().get::<Camera>(world_camera).unwrap().is_active);
        opened(&mut app, PhonePage::Home);
        app.update();
        assert!(app.world().get::<Camera>(view).unwrap().is_active);
        assert!(app.world().get::<Camera>(texture).unwrap().is_active);
        app.insert_resource(State::new(GameState::Menu));
        app.update();
        assert!(!app.world().get::<Camera>(view).unwrap().is_active);
        assert!(!app.world().get::<Camera>(texture).unwrap().is_active);
        assert!(app.world().get::<Camera>(world_camera).unwrap().is_active);
    }
    fn click_command(app: &mut App, command: PhoneCommand) {
        {
            let mut phone = app.world_mut().resource_mut::<Phone>();
            phone.pointer = Some(Vec2::splat(50.0));
            phone.pending = Some((phone.page, command));
        }
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        app.update();
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .reset(MouseButton::Left);
    }

    #[test]
    fn typed_stake_consumes_digits_and_backspace_and_only_explicit_bet_settles() {
        let mut app = app();
        opened(&mut app, PhonePage::Football);
        let original = app
            .world()
            .resource::<CampaignStore>()
            .campaign()
            .unwrap()
            .clone();
        click_command(&mut app, PhoneCommand::EditStake);
        for digit in [
            KeyCode::Digit1,
            KeyCode::Digit2,
            KeyCode::Digit3,
            KeyCode::Backspace,
        ] {
            key(&mut app, digit);
        }
        assert_eq!(
            app.world().resource::<Phone>().stake_entry.as_deref(),
            Some("12")
        );
        assert_eq!(app.world().resource::<Phone>().page, PhonePage::Football);
        assert_eq!(
            app.world().resource::<CampaignStore>().campaign(),
            Some(&original)
        );
        key(&mut app, KeyCode::Enter);
        assert_eq!(app.world().resource::<Phone>().stake_cents, 1200);
        assert!(app.world().resource::<Phone>().stake_entry.is_none());
        assert_eq!(
            app.world().resource::<CampaignStore>().campaign(),
            Some(&original)
        );
        key(&mut app, KeyCode::Digit2);
        let c = app.world().resource::<CampaignStore>().campaign().unwrap();
        assert_eq!(c.betting.history.len(), 1);
        assert_eq!(c.betting.history[0].choice, BetChoice::FootballDraw);
        assert_eq!(c.betting.history[0].stake_cents, 1200);
        assert!(app.world().resource::<Phone>().animation_left > 0.0);
    }

    #[test]
    fn held_duplicate_stale_busy_paused_and_outside_bet_presses_never_autoplay() {
        let bet = PhoneCommand::Apply(Action::Bet {
            choice: BetChoice::RouletteRed,
            stake_cents: 100,
        });
        let mut app = app();
        opened(&mut app, PhonePage::Roulette);
        click_command(&mut app, bet);
        let settled = app
            .world()
            .resource::<CampaignStore>()
            .campaign()
            .unwrap()
            .clone();
        click_command(&mut app, bet); // Busy presentation rejects a queued second wager.
        assert_eq!(
            app.world().resource::<CampaignStore>().campaign(),
            Some(&settled)
        );
        click_command(&mut app, PhoneCommand::Skip);
        assert_eq!(
            app.world().resource::<CampaignStore>().campaign(),
            Some(&settled)
        );
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .clear();
        for _ in 0..5 {
            let mut phone = app.world_mut().resource_mut::<Phone>();
            phone.pending = Some((PhonePage::Roulette, bet));
            app.update();
        }
        assert_eq!(
            app.world().resource::<CampaignStore>().campaign(),
            Some(&settled)
        );
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .reset(MouseButton::Left);
        for scenario in 0..4 {
            opened(&mut app, PhonePage::Roulette);
            {
                let mut phone = app.world_mut().resource_mut::<Phone>();
                phone.pointer = if scenario == 0 { None } else { Some(Vec2::ONE) };
                phone.pending = Some((
                    if scenario == 1 {
                        PhonePage::Slots
                    } else {
                        PhonePage::Roulette
                    },
                    bet,
                ));
            }
            if scenario == 2 {
                *app.world_mut().resource_mut::<NextState<GameState>>() =
                    NextState::Pending(GameState::Menu);
            }
            if scenario == 3 {
                let mut windows = app
                    .world_mut()
                    .query_filtered::<&mut Window, With<PrimaryWindow>>();
                windows.single_mut(app.world_mut()).unwrap().focused = false;
            }
            app.world_mut()
                .resource_mut::<ButtonInput<MouseButton>>()
                .press(MouseButton::Left);
            app.update();
            app.world_mut()
                .resource_mut::<ButtonInput<MouseButton>>()
                .reset(MouseButton::Left);
            *app.world_mut().resource_mut::<NextState<GameState>>() = NextState::Unchanged;
            assert_eq!(
                app.world().resource::<CampaignStore>().campaign(),
                Some(&settled)
            );
        }
    }

    #[test]
    fn shared_mouse_modes_close_once_and_restore_settled_session_without_rng_or_pa_change() {
        let mut app = app();
        opened(&mut app, PhonePage::Roulette);
        key(&mut app, KeyCode::Digit1);
        click_command(&mut app, PhoneCommand::Page(PhonePage::Slots));
        key(&mut app, KeyCode::Digit1);
        click_command(&mut app, PhoneCommand::Page(PhonePage::Football));
        key(&mut app, KeyCode::Digit3);
        let settled = app
            .world()
            .resource::<CampaignStore>()
            .campaign()
            .unwrap()
            .clone();
        assert_eq!(settled.betting.history.len(), 3);
        assert_eq!(settled.clock.pa_spent, 1);
        *app.world_mut().resource_mut::<Phone>() = Phone::default();
        app.world_mut().resource_mut::<GameplayOverlay>().active = None;
        app.add_systems(PostUpdate, restore_session);
        app.update();
        let phone = app.world().resource::<Phone>();
        assert!(phone.opened);
        assert_eq!(phone.page, PhonePage::Football);
        assert_eq!(phone.animation_left, 0.0);
        assert_eq!(phone.settled_round.as_ref(), settled.betting.history.last());
        assert_eq!(
            app.world().resource::<CampaignStore>().campaign(),
            Some(&settled)
        );
        key(&mut app, KeyCode::KeyH);
        assert!(
            app.world()
                .resource::<CampaignStore>()
                .campaign()
                .unwrap()
                .betting
                .session
                .is_none()
        );
        let closed = app
            .world()
            .resource::<CampaignStore>()
            .campaign()
            .unwrap()
            .clone();
        app.update();
        assert_eq!(
            app.world().resource::<CampaignStore>().campaign(),
            Some(&closed)
        );
        assert_eq!(closed.rng, settled.rng);
        assert_eq!(closed.balance_cents, settled.balance_cents);
    }
    #[test]
    fn visible_mouse_navigation_remains_usable_while_editing_stake() {
        for command in [
            PhoneCommand::Page(PhonePage::Home),
            PhoneCommand::Page(PhonePage::Slots),
            PhoneCommand::Close,
        ] {
            let mut app = app();
            opened(&mut app, PhonePage::Roulette);
            click_command(&mut app, PhoneCommand::EditStake);
            key(&mut app, KeyCode::Digit2);
            let before = app
                .world()
                .resource::<CampaignStore>()
                .campaign()
                .unwrap()
                .clone();
            click_command(&mut app, command);
            let phone = app.world().resource::<Phone>();
            assert!(phone.stake_entry.is_none());
            match command {
                PhoneCommand::Page(page) => assert_eq!(phone.page, page),
                _ => assert!(!phone.opened),
            }
            assert_eq!(
                app.world().resource::<CampaignStore>().campaign(),
                Some(&before)
            );
        }
    }
    #[test]
    fn retry_save_remains_usable_without_discarding_a_stake_draft() {
        let mut app = app();
        opened(&mut app, PhonePage::Slots);
        click_command(&mut app, PhoneCommand::EditStake);
        key(&mut app, KeyCode::Digit2);
        app.world_mut().resource_mut::<CampaignStore>().notice = "old warning".into();
        click_command(&mut app, PhoneCommand::RetrySave);
        assert_eq!(
            app.world().resource::<Phone>().stake_entry.as_deref(),
            Some("2")
        );
        assert!(
            app.world()
                .resource::<CampaignStore>()
                .notice
                .contains("protected")
        );
    }
    #[test]
    fn stop_stays_exposed_and_usable_during_stake_editing_without_settling_again() {
        let mut app = app();
        opened(&mut app, PhonePage::Roulette);
        key(&mut app, KeyCode::Digit1);
        let settled = super::super::campaign_save::encode_campaign(
            app.world().resource::<CampaignStore>().campaign().unwrap(),
        )
        .unwrap();
        click_command(&mut app, PhoneCommand::EditStake);
        key(&mut app, KeyCode::Digit2);
        key(&mut app, KeyCode::Digit1);
        let command = {
            let phone = app.world().resource::<Phone>();
            assert_eq!(phone.stake_entry.as_deref(), Some("21"));
            assert!(phone.animation_left > 0.0); // Numeric input edits the draft, including Stop's shortcut number.
            let c = app.world().resource::<CampaignStore>().campaign().unwrap();
            let (_, choices) = phone_content(phone, c);
            choices
                .iter()
                .find(|(_, command)| *command == PhoneCommand::Skip)
                .unwrap()
                .1
        };
        click_command(&mut app, command);
        let phone = app.world().resource::<Phone>();
        assert_eq!(phone.animation_left, 0.0);
        assert_eq!(phone.stake_entry.as_deref(), Some("21"));
        assert_eq!(
            super::super::campaign_save::encode_campaign(
                app.world().resource::<CampaignStore>().campaign().unwrap()
            )
            .unwrap(),
            settled
        );
    }
    #[test]
    fn ordered_amount_events_preserve_reverse_digits_repeats_and_backspace_in_one_frame() {
        let mut app = app();
        opened(&mut app, PhonePage::Roulette);
        let original = app
            .world()
            .resource::<CampaignStore>()
            .campaign()
            .unwrap()
            .clone();
        // Text received before opening the editor must be discarded.
        amount_input(&mut app, KeyCode::Digit9, Some("9"), false);
        click_command(&mut app, PhoneCommand::EditStake);
        assert_eq!(
            app.world().resource::<Phone>().stake_entry.as_deref(),
            Some("")
        );
        let window = app
            .world_mut()
            .query_filtered::<Entity, With<PrimaryWindow>>()
            .single(app.world())
            .unwrap();
        app.world_mut().get_mut::<Window>(window).unwrap().focused = false;
        amount_input(&mut app, KeyCode::Digit8, Some("8"), false);
        app.update();
        app.world_mut().get_mut::<Window>(window).unwrap().focused = true;
        app.insert_resource(State::new(GameState::Menu));
        amount_input(&mut app, KeyCode::Digit7, Some("7"), false);
        app.update();
        app.insert_resource(State::new(GameState::Playing));
        app.update();
        assert_eq!(
            app.world().resource::<Phone>().stake_entry.as_deref(),
            Some("")
        );
        for (key, text, repeat) in [
            (KeyCode::Digit2, Some("2"), false),
            (KeyCode::Digit1, Some("1"), false),
            (KeyCode::Digit1, Some("1"), true),
            (KeyCode::Digit1, Some("1"), true),
            (KeyCode::Backspace, None, false),
            (KeyCode::Digit3, Some("3"), false),
        ] {
            amount_input(&mut app, key, text, repeat);
        }
        app.update();
        assert_eq!(
            app.world().resource::<Phone>().stake_entry.as_deref(),
            Some("2113")
        );
        assert_eq!(
            app.world().resource::<CampaignStore>().campaign(),
            Some(&original)
        );
        // Backspace and confirmation also honor their original order in one frame.
        for _ in 0..2 {
            amount_input(&mut app, KeyCode::Backspace, None, false);
        }
        amount_input(&mut app, KeyCode::Enter, None, false);
        app.update();
        assert_eq!(app.world().resource::<Phone>().stake_cents, 2100);
        assert!(app.world().resource::<Phone>().stake_entry.is_none());
        assert_eq!(
            app.world().resource::<CampaignStore>().campaign(),
            Some(&original)
        );
    }

    #[test]
    fn bank_payment_is_free_at_zero_pa_and_contacts_remain_readable_in_blackout() {
        let mut c = Campaign::new(5);
        for _ in 1..6 {
            c.apply(Action::Sleep).unwrap();
            c.apply(Action::AcknowledgeReport).unwrap();
        }
        for _ in 0..6 {
            c.apply(Action::Wait).unwrap();
        }
        c.support.app_blocked_today = true;
        assert!(!c.has_power());
        let original = c.clone();
        let mut app = app();
        app.insert_resource(CampaignStore::for_test(Some(c), SnapshotState::Protected));
        opened(&mut app, PhonePage::Contacts);
        let (copy, choices) = page_content(PhonePage::Contacts, &original);
        assert!(copy.contains("Alex's sister") && copy.contains("Alex's friend"));
        assert!(copy.contains("front door") && copy.contains("Reading is free"));
        assert_eq!(choices.len(), 2);
        key(&mut app, KeyCode::KeyH);
        assert_eq!(
            app.world().resource::<CampaignStore>().campaign(),
            Some(&original)
        );
        key(&mut app, KeyCode::Digit2);
        key(&mut app, KeyCode::Digit1);
        let paid = app
            .world()
            .resource::<CampaignStore>()
            .campaign()
            .unwrap()
            .clone();
        assert!(paid.has_power());
        assert_eq!(paid.clock, original.clock);
        assert_eq!(paid.attributes, original.attributes);
        assert_eq!(paid.balance_cents, original.balance_cents - 10_000);
        assert!(
            app.world()
                .resource::<Phone>()
                .feedback
                .contains("Electricity paid")
        );
        click_command(
            &mut app,
            PhoneCommand::Apply(Action::PayBill(BillKind::Electricity)),
        );
        assert_eq!(
            app.world().resource::<CampaignStore>().campaign(),
            Some(&paid)
        );
        opened(&mut app, PhonePage::Contacts);
        key(&mut app, KeyCode::Digit1);
        assert!(app.world().resource::<Phone>().feedback.contains("1 PA"));
        assert_eq!(
            app.world().resource::<CampaignStore>().campaign(),
            Some(&paid)
        );
    }

    #[test]
    fn contacts_dispatch_the_matching_call_and_reject_visit_and_held_commands() {
        let mut c = Campaign::new(5);
        c.apply(Action::Sleep).unwrap();
        c.apply(Action::AcknowledgeReport).unwrap();
        for _ in 0..3 {
            c.apply(Action::Wait).unwrap();
        }
        let mut app = app();
        app.insert_resource(CampaignStore::for_test(Some(c), SnapshotState::Protected));
        opened(&mut app, PhonePage::Contacts);
        click_command(
            &mut app,
            PhoneCommand::Apply(Action::ReceiveVisit(Contact::Rafa)),
        );
        assert_eq!(
            app.world()
                .resource::<CampaignStore>()
                .campaign()
                .unwrap()
                .clock
                .pa_spent,
            3
        );
        key(&mut app, KeyCode::Digit1);
        let settled = app
            .world()
            .resource::<CampaignStore>()
            .campaign()
            .unwrap()
            .clone();
        assert_eq!(settled.clock.pa_spent, 4);
        assert_eq!(settled.commitments[0].status, CommitmentStatus::Completed);
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .clear();
        for _ in 0..3 {
            app.world_mut().resource_mut::<Phone>().pending = Some((
                PhonePage::Contacts,
                PhoneCommand::Apply(Action::Call(Contact::Marina)),
            ));
            app.update();
        }
        assert_eq!(
            app.world().resource::<CampaignStore>().campaign(),
            Some(&settled)
        );
        assert!(
            app.world()
                .resource::<CampaignStore>()
                .notice
                .contains("session")
        );
    }

    #[test]
    fn calendar_and_reactions_follow_real_bills_bets_misses_and_support() {
        let mut c = Campaign::new(5);
        let before = c.clone();
        let (home, choices) = page_content(PhonePage::Home, &c);
        assert_eq!(home, "Marina: day 2 afternoon");
        assert_eq!(choices.len(), 7);
        assert!(home.len() <= 28);
        assert!(situation(&c).contains("tomorrow afternoon"));
        assert!(contact_message(&c, Contact::Rafa).contains("no betting"));
        assert_eq!(c, before);
        for _ in 0..4 {
            c.apply(Action::Bet {
                choice: BetChoice::Slots,
                stake_cents: 100,
            })
            .unwrap();
        }
        assert!(contact_message(&c, Contact::Rafa).contains("4 bets"));
        assert!(contact_message(&c, Contact::Rafa).contains("beyond three rounds"));
        while c.clock.day < 5 {
            c.apply(Action::Sleep).unwrap();
            c.apply(Action::AcknowledgeReport).unwrap();
        }
        assert!(situation(&c).contains("Electricity warning"));
        assert_eq!(home_reminder(&c), "Overdue bill: see Bank");
        assert!(contact_message(&c, Contact::Marina).contains("missed 1"));
        assert!(contact_message(&c, Contact::Marina).contains("unpaid bills"));
        c.apply(Action::PayBill(BillKind::Electricity)).unwrap();
        assert!(situation(&c).contains("Electricity is paid"));
        c.apply(Action::Work).unwrap();
        assert!(situation(&c).contains("Electricity is paid"));
        assert!(!situation(&c).contains("available today"));
        c.support.care_actions = 1;
        assert!(contact_message(&c, Contact::Marina).contains("care or planning steps"));
    }

    #[test]
    fn lifetime_betting_message_keeps_totals_beyond_the_balance_range_exact() {
        let mut c = Campaign::new(5);
        c.apply(Action::Bet {
            choice: BetChoice::Slots,
            stake_cents: 100,
        })
        .unwrap();
        let mut round = c.betting.history[0].clone();
        round.net_cents = i64::MAX;
        c.betting.history = vec![round.clone(), round];
        assert!(contact_message(&c, Contact::Rafa).contains("net R$184467440737095516.14"));
        for round in &mut c.betting.history {
            round.net_cents = i64::MIN + 1;
        }
        assert!(contact_message(&c, Contact::Rafa).contains("net -R$184467440737095516.14"));
    }

    #[test]
    fn cap_amount_uses_ordered_events_and_requires_a_separate_explicit_apply() {
        let mut app = app();
        opened(&mut app, PhonePage::DailyCap);
        let original = app
            .world()
            .resource::<CampaignStore>()
            .campaign()
            .unwrap()
            .clone();
        amount_input(&mut app, KeyCode::Digit9, Some("9"), false);
        click_command(&mut app, PhoneCommand::EditCap);
        assert_eq!(
            app.world().resource::<Phone>().cap_entry.as_deref(),
            Some("")
        );
        for (key, text, repeat) in [
            (KeyCode::Digit2, Some("2"), false),
            (KeyCode::Digit1, Some("1"), false),
            (KeyCode::Digit1, Some("1"), true),
            (KeyCode::Backspace, None, false),
            (KeyCode::Enter, None, false),
        ] {
            amount_input(&mut app, key, text, repeat);
        }
        app.update();
        assert_eq!(app.world().resource::<Phone>().cap_cents, 2100);
        assert!(app.world().resource::<Phone>().cap_entry.is_none());
        assert_eq!(
            app.world().resource::<CampaignStore>().campaign(),
            Some(&original)
        );
        click_command(&mut app, PhoneCommand::Apply(Action::SetDailyCap(Some(0))));
        assert_eq!(
            app.world().resource::<CampaignStore>().campaign(),
            Some(&original)
        );
        click_command(
            &mut app,
            PhoneCommand::Apply(Action::SetDailyCap(Some(2100))),
        );
        assert_eq!(
            app.world()
                .resource::<CampaignStore>()
                .campaign()
                .unwrap()
                .support
                .daily_cap_cents,
            Some(2100)
        );
        assert!(app.world().resource::<Phone>().feedback.contains("applied"));
        assert!(!app.world().resource::<Phone>().feedback.contains("saved"));
        assert!(
            app.world()
                .resource::<CampaignStore>()
                .notice
                .contains("protected")
        );
        click_command(&mut app, PhoneCommand::EditCap);
        key(&mut app, KeyCode::Digit0);
        click_command(&mut app, PhoneCommand::ConfirmCap);
        assert_eq!(app.world().resource::<Phone>().cap_cents, 0);
        key(&mut app, KeyCode::Backspace);
        assert_eq!(app.world().resource::<Phone>().page, PhonePage::Support);
    }

    #[test]
    fn cap_editor_drains_inactive_text_and_keeps_navigation_and_retry_usable() {
        let mut app = app();
        opened(&mut app, PhonePage::DailyCap);
        click_command(&mut app, PhoneCommand::EditCap);
        let window = app
            .world_mut()
            .query_filtered::<Entity, With<PrimaryWindow>>()
            .single(app.world())
            .unwrap();
        app.world_mut().get_mut::<Window>(window).unwrap().focused = false;
        amount_input(&mut app, KeyCode::Digit9, Some("9"), false);
        app.update();
        app.world_mut().get_mut::<Window>(window).unwrap().focused = true;
        app.insert_resource(State::new(GameState::Menu));
        amount_input(&mut app, KeyCode::Digit8, Some("8"), false);
        app.update();
        app.insert_resource(State::new(GameState::Playing));
        app.update();
        assert_eq!(
            app.world().resource::<Phone>().cap_entry.as_deref(),
            Some("")
        );
        key(&mut app, KeyCode::Digit2);
        key(&mut app, KeyCode::Digit1);
        let original = app
            .world()
            .resource::<CampaignStore>()
            .campaign()
            .unwrap()
            .clone();
        click_command(&mut app, PhoneCommand::RetrySave);
        assert_eq!(
            app.world().resource::<Phone>().cap_entry.as_deref(),
            Some("21")
        );
        click_command(
            &mut app,
            PhoneCommand::Apply(Action::SetDailyCap(Some(2100))),
        );
        assert_eq!(
            app.world().resource::<CampaignStore>().campaign(),
            Some(&original)
        );
        click_command(&mut app, PhoneCommand::Page(PhonePage::Home));
        assert_eq!(app.world().resource::<Phone>().page, PhonePage::Home);
        assert!(app.world().resource::<Phone>().cap_entry.is_none());
        assert_eq!(
            app.world().resource::<CampaignStore>().campaign(),
            Some(&original)
        );
    }

    #[test]
    fn care_dispatch_rejects_held_stale_unfocused_and_zero_pa_actions() {
        let mut app = app();
        opened(&mut app, PhonePage::Support);
        let command = PhoneCommand::Apply(Action::CareConversation);
        click_command(&mut app, command);
        let after = app
            .world()
            .resource::<CampaignStore>()
            .campaign()
            .unwrap()
            .clone();
        assert_eq!((after.clock.pa_spent, after.support.care_actions), (1, 1));
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .clear();
        for _ in 0..3 {
            app.world_mut().resource_mut::<Phone>().pending = Some((PhonePage::Support, command));
            app.update();
        }
        assert_eq!(
            app.world().resource::<CampaignStore>().campaign(),
            Some(&after)
        );
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .reset(MouseButton::Left);
        for scenario in 0..3 {
            opened(&mut app, PhonePage::Support);
            {
                let mut phone = app.world_mut().resource_mut::<Phone>();
                phone.pointer = if scenario == 0 { None } else { Some(Vec2::ONE) };
                phone.pending = Some((
                    if scenario == 1 {
                        PhonePage::Contacts
                    } else {
                        PhonePage::Support
                    },
                    command,
                ));
            }
            if scenario == 2 {
                let mut query = app
                    .world_mut()
                    .query_filtered::<&mut Window, With<PrimaryWindow>>();
                query.single_mut(app.world_mut()).unwrap().focused = false;
            }
            app.world_mut()
                .resource_mut::<ButtonInput<MouseButton>>()
                .press(MouseButton::Left);
            app.update();
            app.world_mut()
                .resource_mut::<ButtonInput<MouseButton>>()
                .reset(MouseButton::Left);
            let mut query = app
                .world_mut()
                .query_filtered::<&mut Window, With<PrimaryWindow>>();
            query.single_mut(app.world_mut()).unwrap().focused = true;
            assert_eq!(
                app.world().resource::<CampaignStore>().campaign(),
                Some(&after)
            );
        }
        let mut exhausted = after.clone();
        while exhausted.clock.remaining_pa() > 0 {
            exhausted.apply(Action::Wait).unwrap();
        }
        app.insert_resource(CampaignStore::for_test(
            Some(exhausted.clone()),
            SnapshotState::Protected,
        ));
        click_command(&mut app, command);
        assert_eq!(
            app.world().resource::<CampaignStore>().campaign(),
            Some(&exhausted)
        );
        assert!(app.world().resource::<Phone>().feedback.contains("1 PA"));
    }

    #[test]
    fn support_and_free_crisis_pages_work_under_blackout_block_and_zero_pa() {
        let mut c = Campaign::new(5);
        for _ in 1..6 {
            c.apply(Action::Sleep).unwrap();
            c.apply(Action::AcknowledgeReport).unwrap();
        }
        for _ in 0..6 {
            c.apply(Action::Wait).unwrap();
        }
        c.apply(Action::BlockBetting).unwrap();
        assert!(!c.has_power());
        let mut app = app();
        app.insert_resource(CampaignStore::for_test(
            Some(c.clone()),
            SnapshotState::Protected,
        ));
        opened(&mut app, PhonePage::Support);
        for page in [
            PhonePage::Contacts,
            PhonePage::Bank,
            PhonePage::History,
            PhonePage::DailyCap,
        ] {
            let (_, choices) = phone_content(app.world().resource::<Phone>(), &c);
            assert!(
                choices
                    .iter()
                    .any(|(_, command)| *command == PhoneCommand::Page(page))
            );
            click_command(&mut app, PhoneCommand::Page(page));
            assert_eq!(app.world().resource::<Phone>().page, page);
            assert_eq!(app.world().resource::<CampaignStore>().campaign(), Some(&c));
            opened(&mut app, PhonePage::Support);
        }
        let (copy, choices) = page_content(PhonePage::Support, &c);
        assert!(
            copy.contains("invented")
                && copy.contains("never a cure")
                && copy.contains("Planning:")
        );
        assert!(copy.contains("Blocked until morning"));
        assert!(
            !choices
                .iter()
                .any(|(_, command)| *command == PhoneCommand::Apply(Action::BlockBetting))
        );
    }

    #[test]
    fn entering_support_ends_session_and_block_keeps_the_committed_result() {
        let mut app = app();
        opened(&mut app, PhonePage::Roulette);
        key(&mut app, KeyCode::Digit1);
        let before = app
            .world()
            .resource::<CampaignStore>()
            .campaign()
            .unwrap()
            .clone();
        let result = app.world().resource::<Phone>().settled_round.clone();
        click_command(&mut app, PhoneCommand::Page(PhonePage::Home));
        click_command(&mut app, PhoneCommand::Page(PhonePage::Support));
        assert!(
            app.world()
                .resource::<CampaignStore>()
                .campaign()
                .unwrap()
                .betting
                .session
                .is_none()
        );
        click_command(&mut app, PhoneCommand::Apply(Action::BlockBetting));
        let after = app.world().resource::<CampaignStore>().campaign().unwrap();
        assert!(after.support.app_blocked_today);
        assert_eq!(after.betting.history, before.betting.history);
        assert_eq!(after.rng, before.rng);
        assert_eq!(after.balance_cents, before.balance_cents);
        assert_eq!(app.world().resource::<Phone>().settled_round, result);
    }
}
