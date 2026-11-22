//! Apartment contexts and the shared input gate used by the handheld phone.
use bevy::{
    camera::primitives::MeshAabb,
    input::mouse::{MouseScrollUnit, MouseWheel},
    prelude::*,
};
use bevy_rapier3d::prelude::{Collider, ColliderDisabled, QueryFilter, ReadRapierContext, Sensor};

use super::{
    audio::Sound,
    camera::MainCamera,
    campaign::*,
    campaign_save::{ActionFeedback, CampaignStore},
    hud::{self, HudSkin},
    player::Player,
};
use crate::{localization::{Localization, format_text}, state::GameState};

pub struct RoutinePlugin;

const SLEEP_FADE_SECS: f64 = 0.8;
const FEEDBACK_SECS: f64 = 3.0;
const MODAL_DIM: f32 = 0.45;
const LANDLORD_LAST_PAGE: usize = 6;
const BILL_TABLE_Y: f32 = 0.909;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OverlayKind {
    Context,
    Phone,
    Landlord { page: usize, resume_phone: bool },
    MissedCall { contact: Contact, resume_phone: bool },
    Transition,
    Wake { bedside: bool },
}

#[derive(Resource, Default)]
pub struct GameplayOverlay {
    pub active: Option<OverlayKind>,
    pub campaign_modal: bool,
    pub escape_consumed: bool,
    pub(super) phone_hint: bool,
}
impl GameplayOverlay {
    pub fn blocks_world(&self) -> bool {
        self.active.is_some() || self.campaign_modal
    }

    pub(super) fn in_dialogue(&self) -> bool {
        matches!(
            self.active,
            Some(OverlayKind::Landlord { .. } | OverlayKind::MissedCall { .. })
        )
    }
}

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct OverlayInput;

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub(super) struct RoutineView;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TargetKind {
    Door,
    Bed,
    Food,
    Tv,
    Cleaning,
    Pipoca,
    Bill(BillKind),
}
impl TargetKind {
    fn label(self, localization: &Localization) -> &'static str {
        match self {
            Self::Door => localization.choose("Front door / MeFood", "Porta de entrada / MeFood"),
            Self::Bed => localization.choose("Bed", "Cama"),
            Self::Food => localization.choose("Kitchen / food", "Cozinha / comida"),
            Self::Tv => "TV",
            Self::Cleaning => localization.choose("Bathroom trash can", "Lixeira do banheiro"),
            Self::Pipoca => "Pipoca",
            Self::Bill(BillKind::Electricity) => localization.choose("Electricity bill", "Conta de luz"),
            Self::Bill(BillKind::Rent) => localization.choose("Rent bill", "Aluguel"),
            Self::Bill(BillKind::Water) => localization.choose("Water bill", "Conta de água"),
        }
    }
}
#[derive(Component)]
struct Scanned;
#[derive(Component)]
struct RoutineTarget {
    kind: TargetKind,
    center: Vec3,
}
#[derive(Resource, Default)]
struct Context {
    nearby: Option<TargetKind>,
    opened: Option<TargetKind>,
    message: String,
    transition_until: f64,
    work_dialogue_until: f64,
    sleep_fade_until: f64,
    knock_until: f64,
    show_log: bool,
    action_feedback: Option<(ActionFeedback, f64)>,
    missed_call: Option<Contact>,
}
#[derive(Component)]
pub(super) struct RoutineUi;
#[derive(Component)]
struct RoutineScroll;
#[derive(Component)]
struct SleepFade;
#[derive(Component)]
struct RoutineDialogue;
#[derive(Component)]
struct TableBill(BillKind);
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
enum UiCommand {
    Apply(Action),
    Close,
    RetrySave,
    ShowLog(bool),
    ReturnTitle,
}

impl Plugin for RoutinePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GameplayOverlay>()
            .init_resource::<Context>()
            .init_resource::<HudSkin>()
            .add_observer(reset_new_campaign)
            .add_systems(
                PreUpdate,
                context_input
                    .in_set(OverlayInput)
                    .after(bevy::input::InputSystems)
                    .run_if(in_state(GameState::Playing)),
            )
            .add_systems(
                Update,
                (
                    tag_targets,
                    handle_actions,
                    dialogue_input.after(super::phone::PhoneView),
                    sync_table_bills,
                    scroll,
                    present.in_set(RoutineView),
                )
                    .chain()
                    .run_if(in_state(GameState::Playing)),
            )
            .add_systems(OnExit(GameState::Playing), hide_ui);
    }
}

pub(crate) fn spawn_bills(mut commands: Commands, assets: Res<AssetServer>) {
    let paper = assets.load(
        GltfAssetLabel::Scene(0).from_asset("models/Paper by Quaternius - 4qDtAbPYnT.glb"),
    );
    // shortcut: fitted to the baked tabletop, recalibrate if the table or paper model changes.
    for (kind, x, z, angle) in [
        (BillKind::Electricity, 2.63, 3.03, -0.12),
        (BillKind::Rent, 2.96, 3.03, 0.12),
        (BillKind::Water, 2.80, 3.32, 0.0),
    ] {
        let target = TargetKind::Bill(kind);
        commands
            .spawn((
                Name::new(target.label(&Localization::default())),
                TableBill(kind),
                RoutineTarget {
                    kind: target,
                    center: Vec3::ZERO,
                },
                Collider::cuboid(0.125, 0.016, 0.13),
                Sensor,
                Visibility::Inherited,
                Transform::from_xyz(x, BILL_TABLE_Y, z)
                    .with_rotation(Quat::from_rotation_y(angle)),
            ))
            .with_child((
                WorldAssetRoot(paper.clone()),
                Transform::from_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2))
                    .with_scale(Vec3::new(0.48, 0.50, 0.08)),
            ));
    }
}

fn sync_table_bills(
    mut commands: Commands,
    store: Res<CampaignStore>,
    mut papers: Query<(Entity, &TableBill, &mut Visibility, Has<ColliderDisabled>)>,
) {
    for (entity, TableBill(kind), mut visibility, disabled) in &mut papers {
        let unpaid = store.campaign().is_some_and(|campaign| {
            campaign
                .bills
                .iter()
                .any(|bill| bill.kind == *kind && bill.paid_day.is_none())
        });
        let desired = if unpaid {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *visibility != desired {
            *visibility = desired;
        }
        if !unpaid && !disabled {
            commands.entity(entity).insert(ColliderDisabled);
        } else if unpaid && disabled {
            commands.entity(entity).remove::<ColliderDisabled>();
        }
    }
}

fn reset_new_campaign(
    _: On<super::campaign_save::CampaignReplaced>,
    mut overlay: ResMut<GameplayOverlay>,
    mut context: ResMut<Context>,
) {
    *overlay = GameplayOverlay::default();
    *context = Context::default();
}

fn kind_for_name(name: &str) -> Option<TargetKind> {
    let name = name.to_ascii_lowercase();
    if !name.starts_with("apartment_") {
        return None;
    }
    if name.contains("solid_entry_door") || name.contains("handle_door") {
        Some(TargetKind::Door)
    } else if name.contains("bed_") {
        Some(TargetKind::Bed)
    } else if name.contains("refrigerator")
        || name.contains("kitchen.")
        || name.contains("stove")
        || name.contains("cookware")
        || name.contains("casserole")
        || name.contains("skillet")
        || name.contains("stewpot")
    {
        Some(TargetKind::Food)
    } else if name.contains("tv.") {
        Some(TargetKind::Tv)
    } else if name == "apartment_trash_can.002" {
        Some(TargetKind::Cleaning)
    } else {
        None
    }
}

fn tag_targets(
    mut commands: Commands,
    meshes: Res<Assets<Mesh>>,
    unscanned: Query<(Entity, &Mesh3d), Without<Scanned>>,
    hierarchy: Query<(Option<&Name>, Option<&ChildOf>)>,
    dogs: Query<Entity, (With<super::household::Pipoca>, Without<RoutineTarget>)>,
) {
    for dog in &dogs {
        commands.entity(dog).insert(RoutineTarget {
            kind: TargetKind::Pipoca,
            center: Vec3::new(0.0, 1.4, 0.3),
        });
    }
    for (entity, mesh) in &unscanned {
        let Some(bounds) = meshes.get(mesh).and_then(Mesh::compute_aabb) else {
            continue;
        };
        let mut ancestor = Some(entity);
        let mut kind = None;
        while let Some(current) = ancestor {
            let Ok((name, parent)) = hierarchy.get(current) else {
                break;
            };
            kind = name.and_then(|name| kind_for_name(name.as_str()));
            if kind.is_some() {
                break;
            }
            ancestor = parent.map(ChildOf::parent);
        }
        commands.entity(entity).insert(Scanned);
        if let Some(kind) = kind {
            commands.entity(entity).insert(RoutineTarget {
                kind,
                center: bounds.center.into(),
            });
        }
    }
}

fn context_input(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Option<Res<ButtonInput<MouseButton>>>,
    cursor: Option<Res<super::cursor::GameCursor>>,
    time: Res<Time<Real>>,
    store: Res<CampaignStore>,
    next: Res<NextState<GameState>>,
    mut overlay: ResMut<GameplayOverlay>,
    mut context: ResMut<Context>,
    camera: Query<&GlobalTransform, With<MainCamera>>,
    player: Query<Entity, With<Player>>,
    targets: Query<(&RoutineTarget, &GlobalTransform)>,
    physics: ReadRapierContext,
) {
    overlay.escape_consumed = false;
    if matches!(*next, NextState::Pending(s) | NextState::PendingIfNeq(s) if s != GameState::Playing)
    {
        return;
    }
    overlay.campaign_modal = store
        .campaign()
        .is_some_and(|c| c.stage != CampaignStage::Playing);
    if overlay.campaign_modal && !overlay.in_dialogue() {
        overlay.active = None;
    }
    if overlay.active == Some(OverlayKind::Transition)
        && time.elapsed_secs_f64() >= context.transition_until
    {
        overlay.active = None;
    }
    if keys.just_pressed(KeyCode::Escape) && overlay.blocks_world() {
        overlay.escape_consumed = true;
        context.show_log = false;
        if !overlay.campaign_modal
            && !overlay.in_dialogue()
            && !matches!(
                overlay.active,
                Some(OverlayKind::Transition | OverlayKind::Wake { .. })
            )
        {
            if overlay.active == Some(OverlayKind::Context) {
                commands.trigger(Sound::Back);
            }
            overlay.active = None;
        }
    }
    if overlay.blocks_world() || overlay.escape_consumed {
        return;
    }
    context.nearby = None;
    let (Ok(camera), Ok(player), Ok(physics)) =
        (camera.single(), player.single(), physics.single())
    else {
        return;
    };
    if let Some((entity, _)) = physics.cast_ray(camera.translation(), *camera.forward(), 2.75, true,
        QueryFilter::default().exclude_collider(player))
        && let Ok((target, transform)) = targets.get(entity)
        // GLB furniture vertices are baked into world space; entity translations are often zero.
        && camera.translation().distance(transform.transform_point(target.center)) <= 3.5
    {
        context.nearby = Some(target.kind);
    }
    let click_bill = matches!(context.nearby, Some(TargetKind::Bill(_)))
        && cursor.is_some_and(|cursor| cursor.captured && !cursor.suppress_look)
        && mouse.is_some_and(|buttons| buttons.just_pressed(MouseButton::Left));
    if (keys.just_pressed(KeyCode::KeyE) || click_bill) && context.nearby.is_some() {
        context.opened = context.nearby;
        context.message.clear();
        overlay.active = Some(OverlayKind::Context);
        commands.trigger(Sound::Open);
    }
}

fn options(
    campaign: &Campaign,
    overlay: &GameplayOverlay,
    context: &Context,
    notice: &str,
    localization: &Localization,
) -> Vec<(String, UiCommand)> {
    let mut result = vec![];
    if campaign.stage != CampaignStage::Playing {
        if context.show_log {
            result.push((
                if cfg!(target_arch = "wasm32") {
                    localization.choose("Back [Backspace]", "Voltar [Backspace]")
                } else {
                    localization.choose("Back [Backspace / Esc]", "Voltar [Backspace / Esc]")
                }
                .into(),
                UiCommand::ShowLog(false),
            ));
        } else if campaign.stage == CampaignStage::DailyReport {
            result.push((
                if campaign.clock.day == campaign.last_day() {
                    localization.choose("View conclusion [Enter]", "Ver conclusão [Enter]").into()
                } else {
                    format_text!(localization, "Continue to day {} [Enter]", "Continuar para o dia {} [Enter]", campaign.clock.day + 1)
                },
                UiCommand::Apply(Action::AcknowledgeReport),
            ));
        }
        if !context.show_log {
            result.push((localization.choose("[1] All days log", "[1] Histórico de todos os dias").into(), UiCommand::ShowLog(true)));
        }
        result.push((localization.choose("[2] Return to Title", "[2] Voltar ao menu principal").into(), UiCommand::ReturnTitle));
    } else if campaign.stage == CampaignStage::Playing
        && overlay.active == Some(OverlayKind::Context)
    {
        let mut actions = match context.opened {
            Some(TargetKind::Door) => vec![(Action::Work, localization.choose("MeFood delivery: 2 PA", "Fazer entregas pelo MeFood · 2 PA"))],
            Some(TargetKind::Bed) => vec![(Action::Sleep, localization.choose("Sleep / end day (unused PA still elapse)", "Dormir e encerrar o dia (os PA restantes serão gastos)"))],
            Some(TargetKind::Food) => vec![
                (Action::Cook, localization.choose("Cook and eat: 1 PA / 1 portion", "Cozinhar e comer · 1 PA / 1 porção")),
                (Action::Groceries, localization.choose("Buy groceries: 1 PA / R$40 / 3 portions", "Comprar mantimentos · 1 PA / R$40 / 3 porções")),
                (Action::OrderFood, localization.choose("Prepared-food delivery: 1 PA / R$20", "Pedir comida · 1 PA / R$20")),
            ],
            Some(TargetKind::Tv) if campaign.household.owns(HouseholdItem::Tv) => {
                vec![(Action::WatchTv, localization.choose("Watch TV: 1 PA", "Assistir à TV · 1 PA"))]
            }
            Some(TargetKind::Pipoca) if campaign.household.owns(HouseholdItem::Pipoca) => {
                vec![(Action::TimeWithPipoca, localization.choose("Time with Pipoca: 1 PA", "Passar um tempo com o Pipoca · 1 PA"))]
            }
            Some(TargetKind::Cleaning) => vec![(Action::Clean, localization.choose("Clean apartment: 1 PA", "Limpar o apartamento · 1 PA"))],
            _ => Vec::new(),
        };
        if let Some(TargetKind::Bill(kind)) = context.opened
            && let Some(bill) = campaign
                .bills
                .iter()
                .find(|bill| bill.kind == kind && bill.paid_day.is_none())
        {
            result.push((
                format_text!(localization, "[1] Pay {} · {}", "[1] Pagar {} · {}", localization.bill(kind), localization.reais(bill.amount_cents)),
                UiCommand::Apply(Action::PayBill(kind)),
            ));
            if kind == BillKind::Rent
                && campaign.is_legacy()
                && !campaign.support.rent_negotiated
                && campaign.clock.day < 9
            {
                result.push((
                    localization.choose("[2] Extend rent to day 10: 1 PA", "[2] Prorrogar o aluguel até o dia 10 · 1 PA").into(),
                    UiCommand::Apply(Action::NegotiateRent),
                ));
            }
        }
        if context.opened == Some(TargetKind::Door)
            && !campaign.uses_phone_for(Contact::Rafa)
            && campaign.commitment_due(Contact::Rafa)
        {
            actions.push((
                Action::ReceiveVisit(Contact::Rafa),
                localization.choose("Answer Rafa's visit: 1 PA", "Receber o Rafa · 1 PA"),
            ));
        }
        result.extend(
            actions
                .iter()
                .filter(|(action, _)| {
                    !matches!(action, Action::Cook | Action::Groceries)
                        || campaign.require_kitchen().is_ok()
                })
                .enumerate()
                .map(|(i, (action, text))| {
                    (format!("[{}] {text}", i + 1), UiCommand::Apply(*action))
                }),
        );
        result.push((
            if cfg!(target_arch = "wasm32") {
                localization.choose("Close", "Fechar")
            } else {
                localization.choose("Close [Esc]", "Fechar [Esc]")
            }
            .into(),
            UiCommand::Close,
        ));
    }
    if !notice.is_empty()
        && (campaign.stage != CampaignStage::Playing
            || matches!(
                overlay.active,
                Some(OverlayKind::Context | OverlayKind::Transition)
            ))
    {
        result.push((localization.choose("Retry saving", "Tentar salvar novamente").into(), UiCommand::RetrySave));
    }
    result
}

fn handle_actions(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    time: Res<Time<Real>>,
    localization: Res<Localization>,
    virtual_time: Res<Time<Virtual>>,
    mut store: ResMut<CampaignStore>,
    mut overlay: ResMut<GameplayOverlay>,
    mut context: ResMut<Context>,
    buttons: Query<(&UiCommand, &Interaction), (With<Button>, Changed<Interaction>)>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    mut next: ResMut<NextState<GameState>>,
) {
    if context.sleep_fade_until > 0.0
        || overlay.in_dialogue()
        || overlay.escape_consumed
        || !windows.single().is_ok_and(|window| window.focused)
        || matches!(*next, NextState::Pending(state) | NextState::PendingIfNeq(state) if state != GameState::Playing)
    {
        return;
    }
    let Some(campaign) = store.campaign() else {
        return;
    };
    if campaign.stage == CampaignStage::DailyReport && campaign.landlord_intro_due() {
        return;
    }
    let balance_before = campaign.balance_cents;
    let choices = options(campaign, &overlay, &context, &store.notice, &localization);
    let command = buttons
        .iter()
        .find(|(command, interaction)| {
            mouse.just_pressed(MouseButton::Left)
                && **interaction == Interaction::Pressed
                && choices.iter().any(|(_, allowed)| allowed == *command)
        })
        .map(|(command, _)| *command)
        .or_else(|| {
            if campaign.stage == CampaignStage::DailyReport
                && !context.show_log
                && keys.just_pressed(KeyCode::Enter)
            {
                Some(UiCommand::Apply(Action::AcknowledgeReport))
            } else if campaign.stage != CampaignStage::Playing {
                if keys.just_pressed(KeyCode::Backspace) && context.show_log {
                    Some(UiCommand::ShowLog(false))
                } else if keys.just_pressed(KeyCode::Digit1) && !context.show_log {
                    Some(UiCommand::ShowLog(true))
                } else if keys.just_pressed(KeyCode::Digit2) {
                    Some(UiCommand::ReturnTitle)
                } else {
                    None
                }
            } else if overlay.active == Some(OverlayKind::Context) {
                [
                    KeyCode::Digit1,
                    KeyCode::Digit2,
                    KeyCode::Digit3,
                    KeyCode::Digit4,
                    KeyCode::Digit5,
                ]
                .iter()
                .position(|key| keys.just_pressed(*key))
                .and_then(|i| choices.get(i).map(|(_, command)| *command))
            } else {
                None
            }
        });
    let Some(command) = command else { return };
    if !matches!(command, UiCommand::Apply(_) | UiCommand::RetrySave) {
        commands.trigger(Sound::Click);
    }
    match command {
        UiCommand::ShowLog(open) => context.show_log = open,
        UiCommand::ReturnTitle => {
            context.show_log = false;
            next.set(GameState::Title);
        }
        UiCommand::Close => overlay.active = None,
        UiCommand::RetrySave => {
            context.message.clear();
            commands.trigger(if store.save().is_ok() {
                Sound::Confirm
            } else {
                Sound::Error
            });
        }
        UiCommand::Apply(action) => match store.apply(action) {
            Ok(_) => {
                if let Some(sound) = Sound::action(action) {
                    commands.trigger(sound);
                }
                context.message = match action {
                    Action::Work => format_text!(
                        localization,
                        "MeFood delivery complete. Back home with {} earned.",
                        "Terminei as entregas do MeFood. Voltei para casa com {}.",
                        localization.reais(store.campaign().unwrap().balance_cents - balance_before)
                    ),
                    Action::Groceries => {
                        localization.choose("Groceries collected. Back home with 3 food portions.", "Voltei das compras com 3 porções de comida.").into()
                    }
                    Action::Cook | Action::OrderFood => {
                        localization.choose("Meal finished. Hunger reduced; the meal left some waste.", "Comi, aliviei a fome e deixei um pouco de lixo.").into()
                    }
                    Action::WatchTv => localization.choose("A little TV helped with boredom and stress.", "Assistir à TV aliviou o tédio e o estresse.").into(),
                    Action::Clean => localization.choose("The apartment is clean.", "O apartamento está limpo.").into(),
                    Action::TimeWithPipoca => localization.choose("You spent a quiet moment with Pipoca.", "Passei um tempo tranquilo com o Pipoca.").into(),
                    Action::ReceiveVisit(Contact::Rafa) => {
                        localization.choose("Rafa came in for the planned visit. You made time for each other.", "O Rafa veio me visitar. Conseguimos passar um tempo juntos.").into()
                    }
                    _ => String::new(),
                };
                if matches!(action, Action::Work | Action::Groceries) {
                    context.transition_until = time.elapsed_secs_f64() + 0.8;
                    overlay.active = Some(OverlayKind::Transition);
                }
                if matches!(action, Action::PayBill(_)) {
                    overlay.active = None;
                    context.opened = None;
                    context.nearby = None;
                }
                if matches!(action, Action::Sleep | Action::AcknowledgeReport) {
                    context.work_dialogue_until = 0.0;
                    overlay.active = None;
                    overlay.campaign_modal = store
                        .campaign()
                        .is_some_and(|c| c.stage != CampaignStage::Playing);
                    if action == Action::Sleep && !store.campaign().unwrap().landlord_intro_due() {
                        context.sleep_fade_until = virtual_time.elapsed_secs_f64() + SLEEP_FADE_SECS;
                    } else if action == Action::AcknowledgeReport && !overlay.campaign_modal {
                        commands.trigger(super::intro::WakeRequested);
                    }
                }
            }
            Err(error) if error == WORK_AT_NIGHT => {
                context.message.clear();
                context.work_dialogue_until = virtual_time.elapsed_secs_f64() + 3.0;
                overlay.active = None;
            }
            Err(error) => {
                commands.trigger(Sound::Error);
                context.message = localization.action_error(&error);
            }
        },
    }
}

fn scroll(
    mut wheel: MessageReader<MouseWheel>,
    keys: Res<ButtonInput<KeyCode>>,
    mut panels: Query<(&mut ScrollPosition, &ComputedNode), With<RoutineScroll>>,
) {
    let mut delta: f32 = wheel
        .read()
        .map(|event| {
            -event.y
                * if event.unit == MouseScrollUnit::Line {
                    32.0
                } else {
                    1.0
                }
        })
        .sum();
    if keys.just_pressed(KeyCode::PageDown) {
        delta += 180.0;
    }
    if keys.just_pressed(KeyCode::PageUp) {
        delta -= 180.0;
    }
    for (mut position, node) in &mut panels {
        let max = ((node.content_size().y - node.size().y) * node.inverse_scale_factor()).max(0.0);
        position.y = (position.y + delta).clamp(0.0, max);
    }
}

fn dialogue_input(
    mut commands: Commands,
    mut store: ResMut<CampaignStore>,
    mut overlay: ResMut<GameplayOverlay>,
    mut context: ResMut<Context>,
    time: Res<Time<Virtual>>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    phone: Option<Res<super::phone::Phone>>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    next: Res<NextState<GameState>>,
) {
    if !windows.single().is_ok_and(|window| window.focused)
        || matches!(*next, NextState::Pending(s) | NextState::PendingIfNeq(s) if s != GameState::Playing)
    {
        return;
    }
    let elapsed = time.elapsed_secs_f64();
    let advance = keys.just_pressed(KeyCode::Enter) || mouse.just_pressed(MouseButton::Left);
    if let Some(OverlayKind::MissedCall { resume_phone, .. }) = overlay.active {
        if advance {
            overlay.active = resume_phone.then_some(OverlayKind::Phone);
            if context.sleep_fade_until > 0.0 {
                context.sleep_fade_until = elapsed + SLEEP_FADE_SECS;
            }
        }
        return;
    }
    let Some(campaign) = store.campaign().filter(|campaign| campaign.landlord_intro_due()) else {
        return;
    };
    if let Some(OverlayKind::Landlord { page, resume_phone }) = overlay.active {
        if page == 0 {
            if elapsed >= context.knock_until {
                overlay.active = Some(OverlayKind::Landlord { page: 1, resume_phone });
            }
        } else if advance {
            if page < LANDLORD_LAST_PAGE {
                overlay.active = Some(OverlayKind::Landlord { page: page + 1, resume_phone });
            } else {
                if campaign.stage == CampaignStage::DailyReport {
                    context.sleep_fade_until = elapsed + SLEEP_FADE_SECS;
                }
                store.finish_landlord_intro();
                overlay.active = resume_phone.then_some(OverlayKind::Phone);
                overlay.phone_hint = true;
            }
        }
    } else if !matches!(overlay.active, Some(OverlayKind::Transition | OverlayKind::Wake { .. }))
        && !phone.is_some_and(|phone| phone.animating())
    {
        context.knock_until = elapsed + 0.7;
        context.work_dialogue_until = 0.0;
        overlay.active = Some(OverlayKind::Landlord {
            page: 0,
            resume_phone: overlay.active == Some(OverlayKind::Phone),
        });
        commands.trigger(Sound::Knock);
    }
}

fn landlord_text(
    campaign: &Campaign,
    localization: &crate::localization::Localization,
    page: usize,
) -> String {
    let line = match page {
        0 => return localization.text("landlord_knock").to_owned(),
        1 => {
            let mut bills: Vec<_> = campaign.bills.iter().collect();
            bills.sort_by_key(|bill| bill.due_day);
            let details = bills
                .into_iter()
                .map(|bill| {
                    let name = localization.bill(bill.kind);
                    format!(
                        "{name} · {} · {} {}",
                        localization.reais(bill.amount_cents),
                        localization.text("day"),
                        bill.due_day
                    )
                })
                .collect::<Vec<_>>()
                .join("\n");
            format!("{}\n\n{details}", localization.text("landlord_bills"))
        }
        2 => localization.text("landlord_hunger").to_owned(),
        3 => localization.text("landlord_boredom").to_owned(),
        4 => localization.text("landlord_stress").to_owned(),
        5 => localization.text("landlord_urge").to_owned(),
        _ => localization.text("landlord_relationships").to_owned(),
    };
    format!(
        "{}: {line}\n\n{}",
        localization.text("landlord_speaker"),
        localization.text("dialogue_continue")
    )
}

pub(super) fn day_money_text(campaign: &Campaign, day: u8, localization: &Localization) -> String {
    let mut groups: Vec<(LedgerKind, i128)> = Vec::new();
    for entry in campaign.ledger.iter().filter(|entry| entry.clock.day == day) {
        let amount = i128::from(entry.amount_cents);
        if let Some((_, total)) = groups.iter_mut().find(|(kind, _)| *kind == entry.kind) {
            *total += amount;
        } else {
            groups.push((entry.kind, amount));
        }
    }

    let mut text = format!("{} {day}", localization.text("day"));
    for (incoming, label) in [(true, localization.choose("Received", "Recebido")), (false, localization.choose("Spent", "Gasto"))] {
        let entries = groups.iter().filter(|(_, amount)| (*amount > 0) == incoming);
        let total = entries.clone().map(|(_, amount)| amount.abs()).sum::<i128>();
        text.push_str(&format!("\n{label}: {}", localization.reais(total)));
        for (kind, amount) in entries {
            let item = match kind {
                LedgerKind::Work => localization.choose("MeFood deliveries", "Entregas pelo MeFood").into(),
                LedgerKind::Food => localization.choose("Food", "Comida").into(),
                LedgerKind::BetStake => localization.choose("Bets", "Apostas").into(),
                LedgerKind::BetPayout => localization.choose("Bet payouts (including returned stakes)", "Retornos das apostas (incluindo a devolução do valor apostado)").into(),
                LedgerKind::BillPayment(kind) => localization.bill(*kind).into(),
                LedgerKind::Sale(item) => format_text!(localization, "{} sale", "Venda de item ({})", localization.item(*item)),
            };
            text.push_str(&format!("\n  {item}: {}", localization.reais(amount.abs())));
        }
    }
    text
}

fn money_log(campaign: &Campaign, localization: &Localization) -> String {
    (1..=campaign.clock.day)
        .rev()
        .map(|day| day_money_text(campaign, day, localization))
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn ending_text(ending: Ending, localization: &Localization) -> &'static str {
    match ending {
        Ending::DogSale => localization.choose("Alex sold Pipoca. The campaign has ended.", "Alex vendeu o Pipoca. A campanha terminou."),
        Ending::HousingLoss => localization.choose("The rent deadline passed with rent unpaid. Alex must find another home.", "O aluguel venceu sem ser pago. Alex precisa encontrar outro lugar para morar."),
        Ending::FinalDay => localization.choose("Alex and Pipoca reached the end of the campaign.", "Alex e Pipoca chegaram ao fim da campanha."),
    }
}

fn present(
    mut commands: Commands,
    skin: Res<HudSkin>,
    mut store: ResMut<CampaignStore>,
    mut overlay: ResMut<GameplayOverlay>,
    mut context: ResMut<Context>,
    phone: Option<Res<super::phone::Phone>>,
    time: Res<Time<Virtual>>,
    existing: Query<(Entity, Has<hud::StatusHud>), With<RoutineUi>>,
    mut dialogue_box: Query<(Entity, &mut Text), With<RoutineDialogue>>,
    mut sleep_fade: Query<&mut BackgroundColor, With<SleepFade>>,
    mut previous: Local<String>,
    mut previous_delta: Local<ActionFeedback>,
    localization: Res<crate::localization::Localization>,
) {
    let elapsed = time.elapsed_secs_f64();
    let body_font = |size| TextFont::from_font_size(size).with_font(skin.dialogue_font.clone());
    let waking = matches!(overlay.active, Some(OverlayKind::Wake { .. }));
    let reveal_pending = phone.is_some_and(|phone| phone.animating());
    if !reveal_pending {
        let mut delta = std::mem::take(&mut store.bypass_change_detection().feedback);
        if let Some(contact) = delta.missed_call.take() {
            context.missed_call = Some(contact);
        }
        if delta != ActionFeedback::default() {
            context.action_feedback = Some((delta, elapsed + FEEDBACK_SECS));
        }
    }
    if !reveal_pending
        && !overlay.in_dialogue()
        && !matches!(
            overlay.active,
            Some(OverlayKind::Transition | OverlayKind::Wake { .. })
        )
        && let Some(contact) = context.missed_call.take()
    {
        overlay.active = Some(OverlayKind::MissedCall {
            contact,
            resume_phone: overlay.active == Some(OverlayKind::Phone),
        });
        context.work_dialogue_until = 0.0;
    }
    let reflection = match overlay.active {
        Some(OverlayKind::MissedCall { contact, .. }) => Some(contact),
        _ => None,
    };
    let dialogue = reflection
        .map(|contact| {
            let line = localization.text("missed_call").replace("{name}", contact.label());
            format!("{line}\n\n{}", localization.text("dialogue_continue"))
        })
        .or_else(|| {
            (elapsed < context.work_dialogue_until)
                .then(|| localization.text("work_at_night").to_owned())
        })
        .or_else(|| {
            (overlay.phone_hint
                && !overlay.campaign_modal
                && matches!(overlay.active, None | Some(OverlayKind::Phone)))
            .then(|| localization.text("phone_hint").to_owned())
        });
    if let Some(line) = dialogue {
        if let Ok((_, mut text)) = dialogue_box.single_mut() {
            if text.0 != line {
                text.0 = line;
            }
        } else {
            commands.spawn((
                RoutineDialogue,
                DespawnOnExit(GameState::Playing),
                super::intro::dialogue_box(line, skin.dialogue_font.clone()),
            ));
        }
    } else if let Ok((entity, _)) = dialogue_box.single_mut() {
        commands.entity(entity).despawn();
    }
    let delta = context
        .action_feedback
        .filter(|&(_, until)| !waking && !reveal_pending && elapsed < until)
        .map_or(ActionFeedback::default(), |(delta, _)| delta);
    let Some(campaign) = store.campaign() else {
        return;
    };
    if reflection.is_none()
        && campaign.stage == CampaignStage::DailyReport
        && elapsed < context.sleep_fade_until
    {
        let remaining = ((context.sleep_fade_until - elapsed) / SLEEP_FADE_SECS) as f32;
        let shade = BackgroundColor(Color::srgba(
            0.0,
            0.0,
            0.0,
            1.0 - remaining * (1.0 - MODAL_DIM),
        ));
        if let Ok(mut background) = sleep_fade.single_mut() {
            *background = shade;
        } else {
            previous.clear();
            for (entity, _) in &existing {
                commands.entity(entity).despawn();
            }
            commands.spawn((
                RoutineUi,
                SleepFade,
                Node {
                    position_type: PositionType::Absolute,
                    width: percent(100),
                    height: percent(100),
                    ..default()
                },
                shade,
                ZIndex(30),
            ));
        }
        return;
    } else if reflection.is_none() {
        context.sleep_fade_until = 0.0;
    }
    let choices = options(campaign, &overlay, &context, &store.notice, &localization);
    let landlord_page = match overlay.active {
        Some(OverlayKind::Landlord { page, .. }) => Some(page),
        _ => None,
    };
    let attributes_visible = if let Some(page) = landlord_page {
        page.saturating_sub(1)
    } else if campaign.character_status_visible() {
        5
    } else {
        0
    };
    let modal = reflection.is_none()
        && (campaign.stage != CampaignStage::Playing
            || overlay.active == Some(OverlayKind::Context)
            || overlay.active == Some(OverlayKind::Transition));
    let heading = match campaign.stage {
        _ if context.show_log => money_log(campaign, &localization),
        CampaignStage::DailyReport => day_money_text(campaign, campaign.clock.day, &localization),
        CampaignStage::Ended(ending) => ending_text(ending, &localization).into(),
        CampaignStage::Playing if overlay.active == Some(OverlayKind::Transition) => {
            context.message.clone()
        }
        CampaignStage::Playing => {
            if let Some(TargetKind::Bill(kind)) = context.opened {
                let bill = campaign.bills.iter().find(|bill| bill.kind == kind).unwrap();
                let status = bill.paid_day.map_or_else(
                    || format_text!(localization, "Due on day {}", "Vence no dia {}", bill.due_day),
                    |day| format_text!(localization, "Paid on day {day}", "Pago no dia {day}"),
                );
                format!(
                    "{}\n\n{} · {status}\n{}",
                    TargetKind::Bill(kind).label(&localization),
                    localization.reais(bill.amount_cents),
                    context.message
                )
            } else {
                format!(
                    "{}\n\n{}{}",
                    context.opened.map_or(localization.choose("Apartment", "Apartamento"), |target| target.label(&localization)),
                    if context.opened == Some(TargetKind::Door)
                        && !campaign.uses_phone_for(Contact::Rafa)
                        && campaign.commitment_due(Contact::Rafa)
                    {
                        localization.choose("Rafa is waiting for this afternoon's planned visit.\n\n", "O Rafa está na porta para a visita marcada para esta tarde.\n\n")
                    } else {
                        ""
                    },
                    context.message
                )
            }
        }
    };
    let signature = format!(
        "{:?}{}{:?}{:?}{heading}{choices:?}{modal}{}{:?}",
        campaign.clock,
        campaign.balance_cents,
        campaign.attributes,
        context.nearby,
        store.notice,
        overlay.active
    );
    let redraw = *previous != signature || existing.is_empty() || localization.is_changed();
    if !redraw && *previous_delta == delta {
        return;
    }
    *previous = signature;
    *previous_delta = delta;
    for (entity, is_status) in &existing {
        if redraw || is_status {
            commands.entity(entity).despawn();
        }
    }
    let show_summary = (!modal && overlay.active.is_none()) || overlay.in_dialogue();
    if show_summary
        || (campaign.stage == CampaignStage::Playing && delta != ActionFeedback::default())
    {
        hud::spawn(
            &mut commands,
            campaign,
            &skin,
            &localization,
            delta,
            show_summary,
            attributes_visible,
        );
    }
    if !redraw {
        return;
    }
    if let Some(page) = landlord_page {
        commands.spawn((
            RoutineUi,
            super::intro::dialogue_box(
                landlord_text(campaign, &localization, page),
                skin.dialogue_font.clone(),
            ),
        ));
        return;
    }
    if show_summary && reflection.is_none() && let Some(target) = context.nearby {
        hud::interaction(&mut commands, target.label(&localization), &skin, &localization);
    }
    if !store.notice.is_empty() && !modal && !waking {
        commands.spawn((
            RoutineUi,
            Name::new("Save notice"),
            Node {
                position_type: PositionType::Absolute,
                bottom: px(24),
                left: percent(10),
                padding: UiRect::all(px(10)),
                max_width: percent(80),
                ..default()
            },
            BackgroundColor(Color::srgba(0.025, 0.03, 0.04, 0.88)),
            ZIndex(20),
            children![(
                Text::new(localization.campaign_notice(&store.notice)),
                body_font(18.0),
                TextColor(Color::WHITE),
            )],
        ));
    }
    if !modal {
        return;
    }
    commands
        .spawn((
            RoutineUi,
            Node {
                position_type: PositionType::Absolute,
                width: percent(100),
                height: percent(100),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(if campaign.stage == CampaignStage::DailyReport {
                Color::BLACK
            } else {
                Color::srgba(0.0, 0.0, 0.0, MODAL_DIM)
            }),
            ZIndex(30),
        ))
        .with_children(|parent| {
            parent
                .spawn((
                    Node {
                        width: percent(90),
                        max_width: px(760),
                        height: if context.show_log {
                            percent(84)
                        } else {
                            auto()
                        },
                        max_height: percent(84),
                        padding: UiRect::all(px(22)),
                        flex_direction: FlexDirection::Column,
                        row_gap: px(10),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.035, 0.045, 0.06)),
                ))
                .with_children(|panel| {
                    panel
                        .spawn((
                            RoutineScroll,
                            ScrollPosition::default(),
                            Node {
                                min_height: px(0),
                                flex_grow: 1.0,
                                overflow: Overflow::scroll_y(),
                                ..default()
                            },
                        ))
                        .with_children(|body| {
                            body.spawn((
                                Text::new(heading),
                                body_font(19.0),
                                TextColor(Color::WHITE),
                                Pickable::IGNORE,
                            ));
                        });
                    for (label, command) in choices.iter().cloned() {
                        panel
                            .spawn((
                                Button,
                                command,
                                Node {
                                    min_height: px(42),
                                    flex_shrink: 0.0,
                                    padding: UiRect::all(px(10)),
                                    ..default()
                                },
                                BackgroundColor(Color::srgb(0.12, 0.20, 0.27)),
                            ))
                            .with_children(|button| {
                                button.spawn((
                                    Text::new(label),
                                    body_font(18.0),
                                    TextColor(Color::WHITE),
                                    Pickable::IGNORE,
                                ));
                            });
                    }
                    if !store.notice.is_empty() {
                        panel.spawn((
                            Text::new(localization.campaign_notice(&store.notice)),
                            body_font(17.0),
                            TextColor(Color::srgb(1.0, 0.8, 0.45)),
                        ));
                    }
                });
        });
}

fn hide_ui(mut commands: Commands, existing: Query<Entity, With<RoutineUi>>) {
    for entity in &existing {
        commands.entity(entity).despawn();
    }
}

#[cfg(test)]
mod tests {
    use super::super::campaign_save::SnapshotState;
    use super::*;

    #[test]
    fn portuguese_action_buttons_use_the_font_with_accents() {
        use bevy::text::FontSource;

        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::asset::AssetPlugin::default()))
            .init_asset::<Font>()
            .init_asset::<Image>()
            .init_resource::<HudSkin>()
            .init_resource::<Localization>()
            .init_resource::<Context>()
            .insert_resource(GameplayOverlay {
                active: Some(OverlayKind::Context),
                ..default()
            })
            .insert_resource(CampaignStore::for_test(
                Some(Campaign::new(5)),
                SnapshotState::Protected,
            ))
            .add_systems(Update, present);

        let mut labels = app.world_mut().query::<(&Text, &TextFont, &ChildOf)>();
        for (target, expected) in [
            (TargetKind::Tv, "[1] Assistir à TV · 1 PA"),
            (TargetKind::Food, "[1] Cozinhar e comer · 1 PA / 1 porção"),
            (TargetKind::Food, "[2] Comprar mantimentos · 1 PA / R$40 / 3 porções"),
        ] {
            app.world_mut().resource_mut::<Context>().opened = Some(target);
            app.update();
            let (_, font, parent) = labels
                .iter(app.world())
                .find(|(text, _, _)| text.0 == expected)
                .unwrap();
            assert!(app.world().get::<Button>(parent.parent()).is_some());
            let FontSource::Handle(handle) = &font.font else {
                panic!("Expected a bundled font");
            };
            assert_eq!(
                app.world().resource::<AssetServer>().get_path(handle.id()).unwrap().path(),
                std::path::Path::new("fonts/LiberationSans-Regular.ttf"),
            );
        }
    }

    #[test]
    fn portuguese_actions_errors_and_reports_use_the_same_language() {
        let localization = Localization { language: crate::localization::Language::Portuguese };
        let mut campaign = Campaign::new(5);
        let overlay = GameplayOverlay { active: Some(OverlayKind::Context), ..default() };
        let context = Context { opened: Some(TargetKind::Pipoca), ..default() };
        assert_eq!(options(&campaign, &overlay, &context, "", &localization)[0].0,
            "[1] Passar um tempo com o Pipoca · 1 PA");

        let context = Context { opened: Some(TargetKind::Bill(BillKind::Rent)), ..default() };
        assert_eq!(options(&campaign, &overlay, &context, "", &localization)[0].0,
            "[1] Pagar Aluguel · R$110,00");
        let error = campaign.apply(Action::PayBill(BillKind::Rent)).unwrap_err();
        assert_eq!(localization.action_error(&error), "Saldo insuficiente ou valor acima do permitido.");

        campaign.apply(Action::OrderFood).unwrap();
        campaign.apply(Action::Sleep).unwrap();
        assert_eq!(day_money_text(&campaign, 1, &localization),
            "Dia 1\nRecebido: R$0,00\nGasto: R$20,00\n  Comida: R$20,00");
        assert!(ending_text(Ending::HousingLoss, &localization).contains("aluguel venceu"));
    }

    fn healthy_campaign() -> Campaign {
        let mut c = Campaign::previous_economy(5);
        for day in 1..=c.last_day() {
            if day <= 3 {
                c.apply(Action::Work).unwrap();
            }
            if day == 4 {
                c.apply(Action::Wait).unwrap();
                c.apply(Action::Wait).unwrap();
                c.apply(Action::ReceiveVisit(Contact::Rafa)).unwrap();
            }
            if day == 2 {
                c.apply(Action::Call(Contact::Marina)).unwrap();
            }
            if day == 3 {
                c.apply(Action::Groceries).unwrap();
            }
            c.apply(Action::Cook).unwrap();
            while c.clock.remaining_pa() > 0 {
                c.apply(Action::WatchTv).unwrap();
            }
            if day == c.last_day() {
                c.apply(Action::PayBill(BillKind::Electricity)).unwrap();
                c.apply(Action::PayBill(BillKind::Rent)).unwrap();
            }
            c.apply(Action::Sleep).unwrap();
            if day == c.last_day() {
                let choices = options(&c, &GameplayOverlay::default(), &Context::default(), "", &Localization::ENGLISH);
                assert_eq!(choices[0].0, "View conclusion [Enter]");
            }
            c.apply(Action::AcknowledgeReport).unwrap();
            c = super::super::campaign_save::decode_campaign(
                &super::super::campaign_save::encode_campaign(&c).unwrap(),
            )
            .unwrap();
        }
        c
    }

    #[test]
    fn previous_economy_keeps_paid_bills_and_reports_for_four_day_saves() {
        let c = healthy_campaign();
        assert_eq!(c.clock.day, 4);
        assert_eq!(c.stage, CampaignStage::Ended(Ending::FinalDay));
        assert_eq!(c.reports.len(), 4);
        assert!(
            c.bills
                .iter()
                .all(|b| b.due_day == 4 && b.paid_day == Some(4))
        );
        assert_eq!(
            (
                c.balance_cents,
                c.household.food_portions,
                c.attributes.relationships,
                c.attributes.urge
            ),
            (4_000, 2, 81, 10)
        );
        assert_eq!(c.rng, Campaign::new(5).rng);
        let log = money_log(&c, &Localization::ENGLISH);
        assert_eq!(
            day_money_text(&c, 4, &Localization::ENGLISH),
            "Day 4\nReceived: R$0.00\nSpent: R$520.00\n  Electricity: R$100.00\n  Rent: R$420.00"
        );
        assert!(log.contains("Day 3\nReceived: R$80.00\n  MeFood deliveries: R$80.00\nSpent: R$40.00\n  Food: R$40.00"));
        for day in 1..=c.last_day() {
            assert!(log.contains(&format!("Day {day}\nReceived:")));
            let report = &c.reports[usize::from(day - 1)];
            assert_eq!(report.finances, c.daily_finances(day).unwrap());
            let f = &report.finances;
            assert_eq!(
                i128::from(f.opening_cents) + i128::from(f.work_cents)
                    - i128::from(f.food_cents)
                    - i128::from(f.gross_bets_cents)
                    + i128::from(f.payouts_cents)
                    - i128::from(f.bills_cents)
                    + i128::from(f.sales_cents),
                i128::from(f.closing_cents)
            );
        }
    }

    #[test]
    fn money_log_sums_large_amounts_without_overflow() {
        let mut c = Campaign::new(5);
        for _ in 0..2 {
            c.ledger.push(LedgerEntry {
                clock: c.clock,
                kind: LedgerKind::BetPayout,
                amount_cents: i64::MAX,
                balance_after_cents: 0,
            });
        }
        assert_eq!(
            money_log(&c, &Localization::ENGLISH),
            "Day 1\nReceived: R$184467440737095516.14\n  Bet payouts (including returned stakes): R$184467440737095516.14\nSpent: R$0.00"
        );
    }

    #[test]
    fn endings_keep_the_outcome_and_the_log_includes_a_partial_final_day() {
        let mut dog = Campaign::new(5);
        dog.attributes.urge = 70;
        for item in super::super::household::HOUSEHOLD_ITEMS {
            dog.apply(Action::Sell(item)).unwrap();
        }
        dog.apply(Action::Sleep).unwrap();
        dog.apply(Action::AcknowledgeReport).unwrap();
        dog.apply(Action::Sell(HouseholdItem::Pipoca)).unwrap();
        assert_eq!(ending_text(Ending::DogSale, &Localization::ENGLISH), "Alex sold Pipoca. The campaign has ended.");
        assert!(money_log(&dog, &Localization::ENGLISH).contains("Day 2\nReceived: R$200.00\n  Pipoca sale: R$200.00"));
        assert_eq!(dog.reports.len(), 1);
        let mut lost = Campaign::new(5);
        for _ in 1..=lost.last_day() {
            lost.apply(Action::Sleep).unwrap();
            if lost.clock.day == lost.last_day() {
                assert_eq!(lost.stage, CampaignStage::DailyReport);
            }
            lost.apply(Action::AcknowledgeReport).unwrap();
        }
        assert_eq!(lost.stage, CampaignStage::Ended(Ending::HousingLoss));
        assert_eq!(lost.clock.day, lost.last_day());
        assert_eq!(
            lost.reports.last().unwrap().overdue_bills,
            [BillKind::Electricity, BillKind::Rent, BillKind::Water]
        );
        assert!(
            ending_text(Ending::HousingLoss, &Localization::ENGLISH).contains("deadline passed with rent unpaid")
        );
        let mut extended = Campaign::legacy(5);
        extended.apply(Action::NegotiateRent).unwrap();
        extended.apply(Action::CareConversation).unwrap();
        extended.apply(Action::CareConversation).unwrap();
        assert_eq!(extended.attributes.urge, 0);
        for _ in 1..=10 {
            extended.apply(Action::Sleep).unwrap();
            extended.apply(Action::AcknowledgeReport).unwrap();
        }
        assert_eq!(ending_text(Ending::FinalDay, &Localization::ENGLISH), "Alex and Pipoca reached the end of the campaign.");
        for c in [&dog, &lost, &extended] {
            let restored = super::super::campaign_save::decode_campaign(
                &super::super::campaign_save::encode_campaign(c).unwrap(),
            )
            .unwrap();
            assert_eq!(&restored, c);
            for action in [
                Action::Wait,
                Action::Work,
                Action::Groceries,
                Action::Cook,
                Action::OrderFood,
                Action::WatchTv,
                Action::TimeWithPipoca,
                Action::Clean,
                Action::PayBill(BillKind::Rent),
                Action::NegotiateRent,
                Action::Call(Contact::Marina),
                Action::ReceiveVisit(Contact::Rafa),
                Action::CareConversation,
                Action::FinancialPlanning,
                Action::SetDailyCap(Some(0)),
                Action::BlockBetting,
                Action::Sleep,
                Action::AcknowledgeReport,
                Action::EndBetSession,
                Action::Sell(HouseholdItem::Pipoca),
                Action::Bet {
                    choice: BetChoice::Slots,
                    stake_cents: 100,
                },
            ] {
                let mut next = restored.clone();
                assert!(next.apply(action).is_err());
                assert_eq!(next, restored);
            }
        }
    }

    #[test]
    fn modal_records_are_read_only_and_only_current_report_enter_advances() {
        let mut c = Campaign::new(5);
        c.landlord_intro_seen = Some(true);
        c.apply(Action::Work).unwrap();
        c.apply(Action::Sleep).unwrap();
        for original in [c, healthy_campaign()] {
            let mut app = App::new();
            app.add_plugins(MinimalPlugins)
            .init_resource::<Localization>()
                .init_resource::<ButtonInput<KeyCode>>()
                .init_resource::<ButtonInput<MouseButton>>()
                .init_resource::<Context>()
                .init_resource::<GameplayOverlay>()
                .init_resource::<NextState<GameState>>()
                .insert_resource(CampaignStore::for_test(
                    Some(original.clone()),
                    SnapshotState::Protected,
                ))
                .add_observer(
                    |_: On<super::super::intro::WakeRequested>,
                     mut overlay: ResMut<GameplayOverlay>| {
                        overlay.active = Some(OverlayKind::Wake { bedside: false });
                    },
                )
                .add_systems(Update, handle_actions);
            app.world_mut().spawn((
                Window {
                    focused: true,
                    ..default()
                },
                bevy::window::PrimaryWindow,
            ));
            let press = |app: &mut App, key| {
                app.world_mut()
                    .resource_mut::<ButtonInput<KeyCode>>()
                    .press(key);
                app.update();
                app.world_mut()
                    .resource_mut::<ButtonInput<KeyCode>>()
                    .reset(key);
                app.update();
            };
            for key in [KeyCode::Digit1, KeyCode::Enter] {
                press(&mut app, key);
                assert_eq!(
                    app.world().resource::<CampaignStore>().campaign(),
                    Some(&original)
                );
            }
            let stale = app
                .world_mut()
                .spawn((
                    Button,
                    UiCommand::Apply(Action::AcknowledgeReport),
                    Interaction::Pressed,
                ))
                .id();
            app.world_mut()
                .resource_mut::<ButtonInput<MouseButton>>()
                .press(MouseButton::Left);
            app.update();
            app.world_mut()
                .resource_mut::<ButtonInput<MouseButton>>()
                .reset(MouseButton::Left);
            app.world_mut().despawn(stale);
            assert_eq!(
                app.world().resource::<CampaignStore>().campaign(),
                Some(&original)
            );
            press(&mut app, KeyCode::Backspace);
            press(&mut app, KeyCode::Enter);
            if original.stage == CampaignStage::DailyReport {
                assert_eq!(
                    app.world()
                        .resource::<CampaignStore>()
                        .campaign()
                        .unwrap()
                        .clock
                        .day,
                    2
                );
                assert_eq!(
                    app.world().resource::<GameplayOverlay>().active,
                    Some(OverlayKind::Wake { bedside: false })
                );
            } else {
                assert!(app.world().resource::<GameplayOverlay>().active.is_none());
                assert_eq!(
                    app.world().resource::<CampaignStore>().campaign(),
                    Some(&original)
                );
                press(&mut app, KeyCode::Digit2);
                assert!(matches!(
                    *app.world().resource::<NextState<GameState>>(),
                    NextState::Pending(GameState::Title)
                ));
                assert_eq!(
                    app.world().resource::<CampaignStore>().campaign(),
                    Some(&original)
                );
            }
        }
    }

    fn input_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<Localization>()
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<NextState<GameState>>()
            .init_resource::<GameplayOverlay>()
            .init_resource::<Context>()
            .insert_resource(CampaignStore::for_test(
                Some(Campaign::new(5)),
                SnapshotState::Available,
            ))
            .add_systems(Update, context_input);
        app
    }

    #[test]
    fn escape_closes_context_and_phone_before_pause_but_keeps_report_modal() {
        for kind in [OverlayKind::Context, OverlayKind::Phone] {
            let mut app = input_app();
            app.world_mut().resource_mut::<GameplayOverlay>().active = Some(kind);
            app.world_mut()
                .resource_mut::<ButtonInput<KeyCode>>()
                .press(KeyCode::Escape);
            app.update();
            let overlay = app.world().resource::<GameplayOverlay>();
            assert_eq!(overlay.active, None);
            assert!(overlay.escape_consumed);
        }
        let mut campaign = Campaign::new(5);
        campaign.apply(Action::Sleep).unwrap();
        let mut app = input_app();
        app.insert_resource(CampaignStore::for_test(
            Some(campaign),
            SnapshotState::Available,
        ));
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Escape);
        app.update();
        let overlay = app.world().resource::<GameplayOverlay>();
        assert!(overlay.campaign_modal && overlay.escape_consumed && overlay.blocks_world());
    }

    #[test]
    fn supplied_mesh_names_and_baked_bounds_locate_every_routine_group() {
        let bytes = std::fs::read("assets/models/apartment_windows.glb").unwrap();
        let length = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
        let gltf: serde_json::Value = serde_json::from_slice(&bytes[20..20 + length]).unwrap();
        for (name, kind, expected) in [
            (
                "Apartment_Bed_01.001",
                TargetKind::Bed,
                Vec3::new(2.217, 0.592, 8.386),
            ),
            (
                "Apartment_Refrigerator.002",
                TargetKind::Food,
                Vec3::new(0.361, 1.047, 5.646),
            ),
            (
                "Apartment_Stove.002",
                TargetKind::Food,
                Vec3::new(2.271, 0.653, 5.761),
            ),
            (
                "Apartment_TV.002",
                TargetKind::Tv,
                Vec3::new(0.337, 1.06, -0.352),
            ),
            (
                "Apartment_solid_entry_door",
                TargetKind::Door,
                Vec3::new(-0.004, 1.1914, 2.876),
            ),
            (
                "Apartment_trash_can.002",
                TargetKind::Cleaning,
                Vec3::new(2.977, 0.212, -3.538),
            ),
        ] {
            let node = gltf["nodes"]
                .as_array()
                .unwrap()
                .iter()
                .find(|node| node["name"] == name)
                .unwrap();
            assert_eq!(kind_for_name(name), Some(kind));
            let mesh = node["mesh"].as_u64().unwrap() as usize;
            let accessor = gltf["meshes"][mesh]["primitives"][0]["attributes"]["POSITION"]
                .as_u64()
                .unwrap() as usize;
            let read_vec = |value: &serde_json::Value| {
                Vec3::new(
                    value[0].as_f64().unwrap() as f32,
                    value[1].as_f64().unwrap() as f32,
                    value[2].as_f64().unwrap() as f32,
                )
            };
            let min = read_vec(&gltf["accessors"][accessor]["min"]);
            let max = read_vec(&gltf["accessors"][accessor]["max"]);
            let translation = node.get("translation").map_or(Vec3::ZERO, read_vec);
            assert!(
                ((min + max) * 0.5 + translation).distance(expected) < 0.002,
                "{name}"
            );
        }
        assert_eq!(kind_for_name("Apartment_Wall"), None);
        assert_eq!(kind_for_name("Urban_Stove"), None);
        assert_eq!(kind_for_name("Apartment_trash_can.001"), None);
        assert_eq!(
            kind_for_name("Apartment_kitchen.002"),
            Some(TargetKind::Food)
        );
    }

    #[test]
    fn each_apartment_context_offers_only_its_own_actions() {
        let mut campaign = Campaign::new(5);
        let overlay = GameplayOverlay {
            active: Some(OverlayKind::Context),
            ..default()
        };
        for (kind, expected) in [
            (TargetKind::Door, [Action::Work].as_slice()),
            (TargetKind::Bed, &[Action::Sleep]),
            (
                TargetKind::Food,
                &[Action::Cook, Action::Groceries, Action::OrderFood],
            ),
            (TargetKind::Cleaning, &[Action::Clean]),
            (TargetKind::Pipoca, &[Action::TimeWithPipoca]),
        ] {
            let context = Context {
                opened: Some(kind),
                ..default()
            };
            let choices = options(&campaign, &overlay, &context, "", &Localization::ENGLISH);
            let actions: Vec<_> = choices
                .iter()
                .filter_map(|(_, command)| match command {
                    UiCommand::Apply(action) => Some(*action),
                    _ => None,
                })
                .collect();
            assert_eq!(actions, expected, "{kind:?}");
            if kind == TargetKind::Door {
                assert_eq!(choices[0].0, "[1] MeFood delivery: 2 PA");
            }
            if kind == TargetKind::Pipoca {
                assert_eq!(choices[0].0, "[1] Time with Pipoca: 1 PA");
            }
        }
        campaign.household.sold.push(HouseholdItem::Fridge);
        let kitchen = Context {
            opened: Some(TargetKind::Food),
            ..default()
        };
        let choices = options(&campaign, &overlay, &kitchen, "", &Localization::ENGLISH);
        assert_eq!(choices[0].1, UiCommand::Apply(Action::OrderFood));
        assert_eq!(choices[1].1, UiCommand::Close);
        assert_eq!(Localization::ENGLISH.reais(-80), "-R$0.80");
        assert_eq!(Localization::ENGLISH.reais(12_345), "R$123.45");
    }

    #[test]
    fn dismissed_unfocused_and_stale_buttons_cannot_dispatch_actions() {
        for (active, consumed, focused, pending_pause, stale) in [
            (Some(OverlayKind::Context), true, true, false, Action::Cook),
            (None, false, true, false, Action::Cook),
            (
                Some(OverlayKind::Context),
                false,
                false,
                false,
                Action::Cook,
            ),
            (Some(OverlayKind::Context), false, true, true, Action::Cook),
            (
                Some(OverlayKind::Context),
                false,
                true,
                false,
                Action::Sleep,
            ),
            (
                Some(OverlayKind::Transition),
                false,
                true,
                false,
                Action::Groceries,
            ),
        ] {
            let campaign = Campaign::new(5);
            let mut app = App::new();
            app.add_plugins(MinimalPlugins)
            .init_resource::<Localization>()
                .init_resource::<ButtonInput<KeyCode>>()
                .init_resource::<ButtonInput<MouseButton>>()
                .insert_resource(GameplayOverlay {
                    active,
                    escape_consumed: consumed,
                    ..default()
                })
                .insert_resource(Context {
                    opened: Some(TargetKind::Food),
                    transition_until: 100.0,
                    ..default()
                })
                .insert_resource(CampaignStore::for_test(
                    Some(campaign.clone()),
                    SnapshotState::Protected,
                ))
                .insert_resource(if pending_pause {
                    NextState::Pending(GameState::Menu)
                } else {
                    NextState::<GameState>::Unchanged
                })
                .add_systems(PreUpdate, context_input)
                .add_systems(Update, handle_actions);
            app.world_mut().spawn((
                Window {
                    focused,
                    ..default()
                },
                bevy::window::PrimaryWindow,
            ));
            app.world_mut()
                .spawn((Button, UiCommand::Apply(stale), Interaction::Pressed));
            app.world_mut()
                .resource_mut::<ButtonInput<MouseButton>>()
                .press(MouseButton::Left);
            if consumed {
                app.world_mut()
                    .resource_mut::<ButtonInput<KeyCode>>()
                    .press(KeyCode::Escape);
            }
            if !focused || pending_pause {
                app.world_mut()
                    .resource_mut::<ButtonInput<KeyCode>>()
                    .press(KeyCode::Digit1);
            }
            app.update();
            assert_eq!(
                app.world().resource::<CampaignStore>().campaign(),
                Some(&campaign)
            );
            assert!(app.world().resource::<CampaignStore>().notice.is_empty());
        }
    }

    #[test]
    fn failed_retry_keeps_persistence_warning_only_in_current_store_notice() {
        let mut app = App::new();
        let mut store = CampaignStore::for_test(Some(Campaign::new(5)), SnapshotState::Protected);
        store.notice = "Storage unavailable; session only.".into();
        app.add_plugins(MinimalPlugins)
            .init_resource::<Localization>()
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<ButtonInput<MouseButton>>()
            .insert_resource(GameplayOverlay {
                active: Some(OverlayKind::Context),
                ..default()
            })
            .insert_resource(Context {
                opened: Some(TargetKind::Food),
                message: "Old session-only warning".into(),
                ..default()
            })
            .insert_resource(store)
            .init_resource::<NextState<GameState>>()
            .add_systems(Update, handle_actions);
        app.world_mut().spawn((
            Window {
                focused: true,
                ..default()
            },
            bevy::window::PrimaryWindow,
        ));
        app.world_mut()
            .spawn((Button, UiCommand::RetrySave, Interaction::Pressed));
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        app.update();
        assert!(app.world().resource::<Context>().message.is_empty());
        assert!(
            app.world()
                .resource::<CampaignStore>()
                .notice
                .contains("protected")
        );
    }

    #[test]
    fn night_work_attempt_shows_dialogue_without_cost_and_afternoon_work_still_commits() {
        let mut campaign = Campaign::new(5);
        for _ in 0..3 {
            campaign.apply(Action::Wait).unwrap();
        }
        let mut afternoon = campaign.clone();
        afternoon.apply(Action::Work).unwrap();
        assert_eq!(afternoon.clock.period(), Period::Night);
        assert!((4_000..=8_000).contains(&(afternoon.balance_cents - campaign.balance_cents)));
        campaign.apply(Action::Wait).unwrap();

        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::asset::AssetPlugin::default()))
            .init_asset::<Image>()
            .init_asset::<Font>()
            .init_resource::<HudSkin>()
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<ButtonInput<MouseButton>>()
            .init_resource::<NextState<GameState>>()
            .insert_resource(crate::localization::Localization {
                language: crate::localization::Language::Portuguese,
            })
            .insert_resource(GameplayOverlay {
                active: Some(OverlayKind::Context),
                ..default()
            })
            .insert_resource(Context {
                opened: Some(TargetKind::Door),
                ..default()
            })
            .insert_resource(CampaignStore::for_test(
                Some(campaign.clone()),
                SnapshotState::Protected,
            ))
            .add_systems(Update, (handle_actions, present).chain());
        app.world_mut().spawn((
            Window {
                focused: true,
                ..default()
            },
            bevy::window::PrimaryWindow,
        ));
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Digit1);
        app.update();
        let store = app.world().resource::<CampaignStore>();
        assert_eq!(store.campaign(), Some(&campaign));
        assert!(store.notice.is_empty());
        assert!(!app.world().resource::<GameplayOverlay>().blocks_world());
        let mut dialogue = app
            .world_mut()
            .query_filtered::<&Text, With<RoutineDialogue>>();
        assert_eq!(
            dialogue.single(app.world()).unwrap().0,
            "Está tarde demais, talvez amanhã..."
        );
        app.world_mut()
            .resource_mut::<Time<Virtual>>()
            .advance_by(std::time::Duration::from_secs(3));
        app.update();
        assert!(dialogue.iter(app.world()).next().is_none());
    }

    #[test]
    fn sleep_shows_the_missed_call_then_fades_to_the_daily_report() {
        use std::time::Duration;

        let mut campaign = Campaign::new(5);
        campaign.landlord_intro_seen = Some(true);
        campaign.apply(Action::Sleep).unwrap();
        campaign.apply(Action::AcknowledgeReport).unwrap();
        campaign.apply(Action::Work).unwrap();
        let received = Localization::ENGLISH.reais(campaign.balance_cents - INITIAL_BALANCE_CENTS);
        let report = format!("Day 2\nReceived: {received}\n  MeFood deliveries: {received}\nSpent: R$0.00");
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::asset::AssetPlugin::default()))
            .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(Duration::ZERO))
            .init_asset::<Image>()
            .init_asset::<Font>()
            .init_resource::<HudSkin>()
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<ButtonInput<MouseButton>>()
            .init_resource::<NextState<GameState>>()
            .insert_resource(Localization::ENGLISH)
            .insert_resource(GameplayOverlay {
                active: Some(OverlayKind::Context),
                ..default()
            })
            .insert_resource(Context {
                opened: Some(TargetKind::Bed),
                ..default()
            })
            .insert_resource(CampaignStore::for_test(
                Some(campaign),
                SnapshotState::Protected,
            ))
            .add_systems(Update, (handle_actions, dialogue_input, present).chain());
        app.world_mut().spawn((
            Window {
                focused: true,
                ..default()
            },
            bevy::window::PrimaryWindow,
        ));
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Digit1);
        app.update();
        let mut fade = app
            .world_mut()
            .query_filtered::<&BackgroundColor, With<SleepFade>>();
        let mut text = app.world_mut().query::<&Text>();
        assert!(text.iter(app.world()).any(|text| text.0.contains("missed my call with Marina")));
        assert!(!text.iter(app.world()).any(|text| text.0 == report));
        app.world_mut()
            .resource_mut::<Time<Virtual>>()
            .advance_by(Duration::from_secs(10));
        app.update();
        assert!(text.iter(app.world()).any(|text| text.0.contains("missed my call with Marina")));
        assert!(fade.iter(app.world()).next().is_none());
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Enter);
        app.update();
        let now = app.world().resource::<Time<Virtual>>().elapsed_secs_f64();
        app.world_mut().resource_mut::<Context>().action_feedback = Some((
            ActionFeedback {
                attributes: [8, 3, 0, 0, 0],
                ..default()
            },
            now + FEEDBACK_SECS,
        ));
        assert!((fade.single(app.world()).unwrap().0.alpha() - MODAL_DIM).abs() < 0.001);
        let step = Duration::from_secs_f64(SLEEP_FADE_SECS / 2.0);
        for halfway in [true, false] {
            app.world_mut()
                .resource_mut::<Time<Virtual>>()
                .advance_by(step);
            let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
            keys.reset_all();
            keys.press(KeyCode::Enter);
            app.update();
            assert_eq!(
                app.world()
                    .resource::<CampaignStore>()
                    .campaign()
                    .unwrap()
                    .stage,
                CampaignStage::DailyReport
            );
            if halfway {
                let shade = fade.single(app.world()).unwrap();
                assert!((shade.0.alpha() - (1.0 + MODAL_DIM) / 2.0).abs() < 0.001);
                assert!(
                    !text
                        .iter(app.world())
                        .any(|text| text.0 == report)
                );
            }
        }
        assert!(fade.iter(app.world()).next().is_none());
        assert!(
            text.iter(app.world())
                .any(|text| text.0 == report)
        );
        let mut status = app.world_mut().query_filtered::<Entity, With<hud::StatusHud>>();
        assert!(status.iter(app.world()).next().is_none());
        let mut roots = app
            .world_mut()
            .query_filtered::<&BackgroundColor, With<RoutineUi>>();
        assert!(roots.iter(app.world()).any(|color| color.0 == Color::BLACK));
    }

    #[test]
    fn rent_extension_is_offered_only_on_the_eligible_legacy_bill() {
        let overlay = GameplayOverlay {
            active: Some(OverlayKind::Context),
            ..default()
        };
        let context = Context {
            opened: Some(TargetKind::Bill(BillKind::Rent)),
            ..default()
        };
        let can_extend = |campaign: &Campaign| {
            options(campaign, &overlay, &context, "", &Localization::ENGLISH)
                .iter()
                .any(|(_, command)| *command == UiCommand::Apply(Action::NegotiateRent))
        };
        let mut old = Campaign::legacy(1);
        assert!(can_extend(&old));
        old.apply(Action::NegotiateRent).unwrap();
        assert!(!can_extend(&old));
        assert!(!can_extend(&Campaign::new(1)));
    }

    #[test]
    fn table_bill_is_free_and_first_paid_action_starts_the_gradual_landlord_visit() {
        use std::time::Duration;

        let campaign = Campaign::new(5);
        let pt = crate::localization::Localization {
            language: crate::localization::Language::Portuguese,
        };
        assert!(landlord_text(&campaign, &pt, 1).starts_with("Proprietária do apartamento: Alex?"));
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::asset::AssetPlugin::default()))
            .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(Duration::ZERO))
            .init_asset::<Image>()
            .init_asset::<Font>()
            .init_resource::<HudSkin>()
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<ButtonInput<MouseButton>>()
            .init_resource::<NextState<GameState>>()
            .insert_resource(Localization::ENGLISH)
            .insert_resource(GameplayOverlay {
                active: Some(OverlayKind::Context),
                ..default()
            })
            .insert_resource(Context {
                opened: Some(TargetKind::Bill(BillKind::Water)),
                ..default()
            })
            .insert_resource(CampaignStore::for_test(
                Some(campaign),
                SnapshotState::Protected,
            ))
            .add_systems(Update, (handle_actions, dialogue_input, sync_table_bills, present).chain());
        let water = app.world_mut().spawn((TableBill(BillKind::Water), Visibility::Inherited)).id();
        let rent = app.world_mut().spawn((TableBill(BillKind::Rent), Visibility::Inherited)).id();
        app.world_mut().spawn((
            Window { focused: true, ..default() },
            bevy::window::PrimaryWindow,
        ));
        app.update();
        let press = |app: &mut App, key| {
            let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
            keys.reset_all();
            keys.press(key);
            app.update();
        };
        press(&mut app, KeyCode::Digit1);
        let paid = app.world().resource::<CampaignStore>().campaign().unwrap();
        assert_eq!(
            paid.bills.iter().find(|bill| bill.kind == BillKind::Water).unwrap().paid_day,
            Some(1)
        );
        assert_eq!(paid.balance_cents, 0);
        assert_eq!(paid.clock.pa_spent, 0);
        assert!(!paid.landlord_intro_due());
        assert_eq!(app.world().get::<Visibility>(water), Some(&Visibility::Hidden));
        assert!(app.world().get::<ColliderDisabled>(water).is_some());
        assert_eq!(app.world().get::<Visibility>(rent), Some(&Visibility::Inherited));
        let mut lines = app.world_mut().query::<&Text>();
        assert!(lines.iter(app.world()).any(|text| text.0 == "-R$60.00"));

        app.world_mut().resource_mut::<Context>().opened = Some(TargetKind::Tv);
        app.world_mut().resource_mut::<GameplayOverlay>().active = Some(OverlayKind::Context);
        press(&mut app, KeyCode::Digit1);
        assert!(matches!(
            app.world().resource::<GameplayOverlay>().active,
            Some(OverlayKind::Landlord { page: 0, .. })
        ));
        let mut names = app.world_mut().query::<&Name>();
        app.world_mut().resource_mut::<Time<Virtual>>().advance_by(Duration::from_secs(1));
        app.update();
        assert!(lines.iter(app.world()).any(|text| {
            text.0.starts_with("Landlady: Alex?")
                && text.0.contains("living-room table")
                && text.0.contains("Water · R$60.00 · Day 2")
                && text.0.contains("Electricity · R$90.00 · Day 3")
                && text.0.contains("Rent · R$110.00 · Day 4")
        }));
        for count in 0..=5 {
            assert_eq!(
                names.iter(app.world()).filter(|name| name.as_str().ends_with(" / 100")).count(),
                count
            );
            if count == 5 {
                assert!(lines.iter(app.world()).any(|text| text.0.contains("pick up your phone")));
                assert!(!lines.iter(app.world()).any(|text| text.0.contains("[Tab]")));
            }
            press(&mut app, KeyCode::Enter);
        }
        assert!(lines.iter(app.world()).any(|text| text.0.contains("[Tab]")));
        let finished = app.world().resource::<CampaignStore>().campaign().unwrap();
        assert_eq!(finished.clock.pa_spent, 1);
        assert_eq!(finished.landlord_intro_seen, Some(true));
        assert!(app.world().resource::<GameplayOverlay>().active.is_none());

        app.insert_resource(CampaignStore::for_test(
            Some(Campaign::previous_economy(5)),
            SnapshotState::Protected,
        ));
        app.update();
        assert_eq!(app.world().get::<Visibility>(water), Some(&Visibility::Hidden));
        assert!(app.world().get::<ColliderDisabled>(water).is_some());

        app.insert_resource(CampaignStore::for_test(
            Some(Campaign::new(5)),
            SnapshotState::Protected,
        ));
        app.update();
        assert_eq!(
            app.world().get::<Visibility>(water),
            Some(&Visibility::Inherited)
        );
        assert!(app.world().get::<ColliderDisabled>(water).is_none());
    }

    #[test]
    fn visit_dispatch_requires_the_current_door_and_a_fresh_press() {
        let mut campaign = Campaign::previous_economy(5);
        for _ in 1..4 {
            campaign.apply(Action::Sleep).unwrap();
            campaign.apply(Action::AcknowledgeReport).unwrap();
        }
        for _ in 0..3 {
            campaign.apply(Action::Wait).unwrap();
        }
        for (kind, fresh, expect_visit) in [
            (TargetKind::Bed, true, false),
            (TargetKind::Food, true, false),
            (TargetKind::Door, false, false),
            (TargetKind::Door, true, true),
        ] {
            let mut app = App::new();
            app.add_plugins(MinimalPlugins)
            .init_resource::<Localization>()
                .init_resource::<ButtonInput<KeyCode>>()
                .init_resource::<ButtonInput<MouseButton>>()
                .init_resource::<NextState<GameState>>()
                .insert_resource(GameplayOverlay {
                    active: Some(OverlayKind::Context),
                    ..default()
                })
                .insert_resource(Context {
                    opened: Some(kind),
                    ..default()
                })
                .insert_resource(CampaignStore::for_test(
                    Some(campaign.clone()),
                    SnapshotState::Protected,
                ))
                .add_systems(Update, handle_actions);
            app.world_mut().spawn((
                Window {
                    focused: true,
                    ..default()
                },
                bevy::window::PrimaryWindow,
            ));
            app.world_mut().spawn((
                Button,
                UiCommand::Apply(Action::ReceiveVisit(Contact::Rafa)),
                Interaction::Pressed,
            ));
            app.world_mut()
                .resource_mut::<ButtonInput<MouseButton>>()
                .press(MouseButton::Left);
            if !fresh {
                app.world_mut()
                    .resource_mut::<ButtonInput<MouseButton>>()
                    .clear();
            }
            app.update();
            let result = app.world().resource::<CampaignStore>().campaign().unwrap();
            if expect_visit {
                assert_eq!(result.clock.pa_spent, 4);
                assert_eq!(result.commitments[1].status, CommitmentStatus::Completed);
                let settled = result.clone();
                app.world_mut()
                    .resource_mut::<ButtonInput<MouseButton>>()
                    .clear();
                // A recreated stale button under a held pointer cannot dispatch again.
                app.world_mut().spawn((
                    Button,
                    UiCommand::Apply(Action::ReceiveVisit(Contact::Rafa)),
                    Interaction::Pressed,
                ));
                app.update();
                assert_eq!(
                    app.world().resource::<CampaignStore>().campaign(),
                    Some(&settled)
                );
            } else {
                assert_eq!(result, &campaign);
            }
        }
    }

    #[test]
    fn second_door_shortcut_answers_the_visit_and_then_disappears() {
        let mut campaign = Campaign::previous_economy(5);
        for _ in 1..4 {
            campaign.apply(Action::Sleep).unwrap();
            campaign.apply(Action::AcknowledgeReport).unwrap();
        }
        campaign.apply(Action::Wait).unwrap();
        campaign.apply(Action::Wait).unwrap();
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<Localization>()
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<ButtonInput<MouseButton>>()
            .init_resource::<NextState<GameState>>()
            .insert_resource(GameplayOverlay {
                active: Some(OverlayKind::Context),
                ..default()
            })
            .insert_resource(Context {
                opened: Some(TargetKind::Door),
                ..default()
            })
            .insert_resource(CampaignStore::for_test(
                Some(campaign),
                SnapshotState::Protected,
            ))
            .add_systems(Update, handle_actions);
        app.world_mut().spawn((
            Window {
                focused: true,
                ..default()
            },
            bevy::window::PrimaryWindow,
        ));
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Digit2);
        app.update();
        let campaign = app.world().resource::<CampaignStore>().campaign().unwrap();
        assert_eq!(campaign.clock.pa_spent, 3);
        assert_eq!(campaign.commitments[1].status, CommitmentStatus::Completed);
        let choices = options(
            campaign,
            app.world().resource::<GameplayOverlay>(),
            app.world().resource::<Context>(),
            "",
            &Localization::ENGLISH,
        );
        assert!(
            !choices
                .iter()
                .any(|(_, command)| *command
                    == UiCommand::Apply(Action::ReceiveVisit(Contact::Rafa)))
        );
    }
}
