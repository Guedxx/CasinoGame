//! Apartment contexts and the shared input gate used by the handheld phone.
use bevy::{
    camera::primitives::MeshAabb,
    input::mouse::{MouseScrollUnit, MouseWheel},
    prelude::*,
};
use bevy_rapier3d::prelude::{QueryFilter, ReadRapierContext};

use super::{
    audio::Sound,
    camera::MainCamera,
    campaign::*,
    campaign_save::CampaignStore,
    hud::{self, HudSkin},
    player::Player,
};
use crate::state::GameState;

pub struct RoutinePlugin;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OverlayKind {
    Context,
    Phone,
    Transition,
}

#[derive(Resource, Default)]
pub struct GameplayOverlay {
    pub active: Option<OverlayKind>,
    pub campaign_modal: bool,
    pub escape_consumed: bool,
}
impl GameplayOverlay {
    pub fn blocks_world(&self) -> bool {
        self.active.is_some() || self.campaign_modal
    }
}

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct OverlayInput;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TargetKind {
    Door,
    Bed,
    Food,
    Tv,
    Cleaning,
    Pipoca,
}
impl TargetKind {
    fn label(self) -> &'static str {
        match self {
            Self::Door => "Front door / MeFood",
            Self::Bed => "Bed",
            Self::Food => "Kitchen / food",
            Self::Tv => "TV",
            Self::Cleaning => "Cleaning",
            Self::Pipoca => "Pipoca",
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
    records: Option<RecordView>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RecordView {
    History,
    Reports,
}
#[derive(Component)]
pub(super) struct RoutineUi;
#[derive(Component)]
struct RoutineScroll;
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
enum UiCommand {
    Apply(Action),
    Close,
    RetrySave,
    Records(Option<RecordView>),
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
                (tag_targets, handle_actions, scroll, present)
                    .chain()
                    .run_if(in_state(GameState::Playing)),
            )
            .add_systems(OnExit(GameState::Playing), hide_ui);
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
        || name.contains("stove")
        || name.contains("cookware")
        || name.contains("casserole")
        || name.contains("skillet")
        || name.contains("stewpot")
    {
        Some(TargetKind::Food)
    } else if name.contains("tv.") {
        Some(TargetKind::Tv)
    } else if name.contains("trash_can") || name.contains("kitchen.") {
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
            center: Vec3::new(0.0, 0.8, 0.0),
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
    if overlay.campaign_modal {
        overlay.active = None;
    }
    if overlay.active == Some(OverlayKind::Transition)
        && time.elapsed_secs_f64() >= context.transition_until
    {
        overlay.active = None;
    }
    if keys.just_pressed(KeyCode::Escape) && overlay.blocks_world() {
        overlay.escape_consumed = true;
        context.records = None;
        if !overlay.campaign_modal && overlay.active != Some(OverlayKind::Transition) {
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
    if keys.just_pressed(KeyCode::KeyE) && context.nearby.is_some() {
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
) -> Vec<(String, UiCommand)> {
    let mut result = vec![];
    if campaign.stage != CampaignStage::Playing {
        if context.records.is_some() {
            result.push((
                if cfg!(target_arch = "wasm32") {
                    "Back to current report / ending [Backspace]"
                } else {
                    "Back to current report / ending [Backspace / Esc]"
                }
                .into(),
                UiCommand::Records(None),
            ));
        } else if campaign.stage == CampaignStage::DailyReport {
            result.push((
                if campaign.clock.day == DAYS {
                    "View conclusion [Enter]".into()
                } else {
                    format!("Continue to day {} [Enter]", campaign.clock.day + 1)
                },
                UiCommand::Apply(Action::AcknowledgeReport),
            ));
        }
        result.extend([
            (
                "[1] Financial and betting history".into(),
                UiCommand::Records(Some(RecordView::History)),
            ),
            (
                "[2] All daily reports".into(),
                UiCommand::Records(Some(RecordView::Reports)),
            ),
            ("[3] Return to Title".into(), UiCommand::ReturnTitle),
        ]);
    } else if campaign.stage == CampaignStage::Playing
        && overlay.active == Some(OverlayKind::Context)
    {
        let actions: &[(Action, &str)] = match context.opened {
            Some(TargetKind::Door) => &[
                (Action::Work, "MeFood shift: 2 PA / earn R$80"),
                (Action::Groceries, "Buy groceries: 1 PA / R$40 / 3 portions"),
                (Action::Wait, "Wait: 1 PA"),
                (Action::OrderFood, "Prepared-food delivery: 1 PA / R$20"),
            ],
            Some(TargetKind::Bed) => &[(Action::Sleep, "Sleep / end day (unused PA still elapse)")],
            Some(TargetKind::Food) => &[
                (Action::Cook, "Cook and eat: 1 PA / 1 portion"),
                (Action::Groceries, "Buy groceries: 1 PA / R$40 / 3 portions"),
                (Action::OrderFood, "Prepared-food delivery: 1 PA / R$20"),
                (Action::Clean, "Clean: 1 PA"),
            ],
            Some(TargetKind::Tv) if campaign.household.owns(HouseholdItem::Tv) => {
                &[(Action::WatchTv, "Watch TV: 1 PA")]
            }
            Some(TargetKind::Pipoca) if campaign.household.owns(HouseholdItem::Pipoca) => &[(
                Action::TimeWithPipoca,
                "Time with Pipoca: 1 PA / stress -8 / boredom -5",
            )],
            Some(TargetKind::Cleaning) => &[
                (Action::Clean, "Clean: 1 PA"),
                (Action::OrderFood, "Prepared-food delivery: 1 PA / R$20"),
            ],
            _ => &[],
        };
        let mut actions = actions.to_vec();
        if context.opened == Some(TargetKind::Door) && campaign.commitment_due(Contact::Rafa) {
            actions.push((
                Action::ReceiveVisit(Contact::Rafa),
                "Answer Rafa's visit: 1 PA",
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
                "Close"
            } else {
                "Close [Esc]"
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
        result.push(("Retry saving".into(), UiCommand::RetrySave));
    }
    result
}

fn handle_actions(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    time: Res<Time<Real>>,
    mut store: ResMut<CampaignStore>,
    mut overlay: ResMut<GameplayOverlay>,
    mut context: ResMut<Context>,
    buttons: Query<(&UiCommand, &Interaction), (With<Button>, Changed<Interaction>)>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    mut next: ResMut<NextState<GameState>>,
) {
    if overlay.escape_consumed
        || !windows.single().is_ok_and(|window| window.focused)
        || matches!(*next, NextState::Pending(state) | NextState::PendingIfNeq(state) if state != GameState::Playing)
    {
        return;
    }
    let Some(campaign) = store.campaign() else {
        return;
    };
    let choices = options(campaign, &overlay, &context, &store.notice);
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
                && context.records.is_none()
                && keys.just_pressed(KeyCode::Enter)
            {
                Some(UiCommand::Apply(Action::AcknowledgeReport))
            } else if campaign.stage != CampaignStage::Playing {
                if keys.just_pressed(KeyCode::Backspace) && context.records.is_some() {
                    Some(UiCommand::Records(None))
                } else if keys.just_pressed(KeyCode::Digit1) {
                    Some(UiCommand::Records(Some(RecordView::History)))
                } else if keys.just_pressed(KeyCode::Digit2) {
                    Some(UiCommand::Records(Some(RecordView::Reports)))
                } else if keys.just_pressed(KeyCode::Digit3) {
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
        UiCommand::Records(view) => context.records = view,
        UiCommand::ReturnTitle => {
            context.records = None;
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
                    Action::Work => "MeFood shift complete. Back home with R$80 earned.",
                    Action::Groceries => "Groceries collected. Back home with 3 food portions.",
                    Action::Cook | Action::OrderFood => {
                        "Meal finished. Hunger reduced; the meal left some waste."
                    }
                    Action::WatchTv => "A little TV helped with boredom and stress.",
                    Action::Clean => "The apartment is clean.",
                    Action::TimeWithPipoca => "You spent a quiet moment with Pipoca.",
                    Action::Wait => "1 PA elapsed.",
                    Action::ReceiveVisit(Contact::Rafa) => {
                        "Rafa came in for the planned visit. You made time for each other."
                    }
                    _ => "",
                }
                .into();
                if matches!(action, Action::Work | Action::Groceries) {
                    context.transition_until = time.elapsed_secs_f64() + 0.8;
                    overlay.active = Some(OverlayKind::Transition);
                }
                if matches!(action, Action::Sleep | Action::AcknowledgeReport) {
                    overlay.active = None;
                    overlay.campaign_modal = store
                        .campaign()
                        .is_some_and(|c| c.stage != CampaignStage::Playing);
                }
            }
            Err(error) => {
                commands.trigger(Sound::Error);
                context.message = error;
            }
        },
    }
}

pub fn reais(cents: i64) -> String {
    let amount = cents.unsigned_abs();
    format!(
        "{}R${}.{:02}",
        if cents < 0 { "-" } else { "" },
        amount / 100,
        amount % 100
    )
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

fn finances_text(f: &DailyFinances) -> String {
    format!(
        "Opening balance: {}\nWork income: +{}\nFood spending: -{}\nGross bets: -{}\nPayouts: +{}\nBetting net: {}\nBill payments: -{}\nSale proceeds: +{}\nClosing balance: {}",
        reais(f.opening_cents),
        reais(f.work_cents),
        reais(f.food_cents),
        reais(f.gross_bets_cents),
        reais(f.payouts_cents),
        reais(f.payouts_cents - f.gross_bets_cents),
        reais(f.bills_cents),
        reais(f.sales_cents),
        reais(f.closing_cents),
    )
}

pub(super) fn report_text(report: &DailyReport) -> String {
    let names = |contacts: &[Contact]| {
        if contacts.is_empty() {
            "None".into()
        } else {
            contacts
                .iter()
                .map(|contact| contact.plan_label())
                .collect::<Vec<_>>()
                .join(", ")
        }
    };
    format!(
        "DAY {} - DAILY REPORT\n\n{}\n\nPA used by actions: {} / 6\nUnused PA elapsed during sleep: {}\nRelationships: {} -> {} (change {:+})\nOverdue at day end: {}\nCommitments kept: {}\nCommitments missed: {}\n\nKey consequences recorded that day\n{}",
        report.day,
        finances_text(&report.finances),
        report.pa_used,
        PA_PER_DAY - report.pa_used,
        report.relationships_opening,
        report.relationships_closing,
        i16::from(report.relationships_closing) - i16::from(report.relationships_opening),
        if report.overdue_bills.is_empty() {
            "None".into()
        } else {
            report
                .overdue_bills
                .iter()
                .map(|kind| kind.label())
                .collect::<Vec<_>>()
                .join(", ")
        },
        names(&report.completed_commitments),
        names(&report.missed_commitments),
        if report.consequences.is_empty() {
            "No additional consequences recorded.".into()
        } else {
            report.consequences.join("\n\n")
        },
    )
}

pub(super) fn reports_text(c: &Campaign) -> String {
    if c.reports.is_empty() {
        return "DAILY REPORTS\n\nNo completed daily reports yet. Reports are recorded when you sleep; a partial-day ending does not elapse unused PA.".into();
    }
    format!(
        "DAILY REPORTS\nSaved day-end facts, newest first. Reading changes nothing.\n\n{}",
        c.reports
            .iter()
            .rev()
            .map(report_text)
            .collect::<Vec<_>>()
            .join("\n\n--------------------\n\n")
    )
}

pub(super) fn history_text(c: &Campaign) -> String {
    let mut text = String::from("FINANCIAL AND BETTING HISTORY\nChronological transactions\n");
    if c.ledger.is_empty() {
        text.push_str("\nNo transactions yet.\n");
    }
    let mut rounds = c.betting.history.iter();
    for entry in &c.ledger {
        let label = match entry.kind {
            LedgerKind::Work => "MeFood work".into(),
            LedgerKind::Food => "Food purchase".into(),
            LedgerKind::BetStake => "Bet stake".into(),
            LedgerKind::BetPayout => "Bet payout (includes returned stake)".into(),
            LedgerKind::BillPayment(kind) => format!("{} payment", kind.label()),
            LedgerKind::Sale(item) => format!("{} sale", item.label()),
        };
        text.push_str(&format!(
            "\nDay {} / {} / PA {}\n{label}: {}\nBalance {}\n",
            entry.clock.day,
            entry.clock.period().label(),
            entry.clock.pa_spent,
            reais(entry.amount_cents),
            reais(entry.balance_after_cents)
        ));
        if entry.kind == LedgerKind::BetStake
            && let Some(round) = rounds.next()
        {
            let choice = match round.choice {
                BetChoice::RouletteRed => "Roulette / red",
                BetChoice::RouletteBlack => "Roulette / black",
                BetChoice::Slots => "Slots",
                BetChoice::FootballHome => "Football / home",
                BetChoice::FootballDraw => "Football / draw",
                BetChoice::FootballAway => "Football / away",
            };
            let outcome = match round.outcome {
                BetOutcome::RouletteRed => "red",
                BetOutcome::RouletteBlack => "black",
                BetOutcome::SlotsWin => "win",
                BetOutcome::SlotsLoss => "loss",
                BetOutcome::FootballHome => "home win",
                BetOutcome::FootballDraw => "draw",
                BetOutcome::FootballAway => "away win",
            };
            text.push_str(&format!("{choice} / result: {outcome}\nStake {} / return {} / net {}\nSession {} / round {} / {} PA consumed\n",
                reais(round.stake_cents), reais(round.total_return_cents), reais(round.net_cents), round.session_id, round.session_round, round.pa_consumed));
        }
    }
    text.push_str("\nDaily financial totals\n");
    for day in 1..=c.clock.day {
        let totals = c
            .daily_finances(day)
            .map(|f| finances_text(&f))
            .unwrap_or_else(|error| format!("Daily totals unavailable: {error}"));
        text.push_str(&format!("\nDay {day}\n{totals}\n"));
    }
    text
}

fn ending_text(c: &Campaign, ending: Ending) -> String {
    let mut text = String::from(match ending {
        Ending::DogSale => {
            "PIPOCA HAS LEFT HOME\n\nYou confirmed Pipoca's permanent sale after selling all six household groups. Alex is alive, but this loss ends the campaign. Pipoca is no longer at home."
        }
        Ending::HousingLoss => {
            "THIS STAY HAS ENDED\n\nDay 9 began with rent unpaid and no extension in place. Alex is alive and must find another home. This campaign ends here; the people in his life can still matter beyond this stay."
        }
        Ending::DayTen => {
            "TEN DAYS COMPLETE\n\nAlex and Pipoca have reached the end of these ten days. What follows comes from the choices recorded in this campaign."
        }
    });
    text.push_str(&format!(
        "\n\nFinances\nBalance remaining: {}.\n",
        reais(c.balance_cents)
    ));
    for bill in &c.bills {
        text.push_str(&format!(
            "{}: {}.\n",
            bill.kind.label(),
            bill.paid_day
                .map(|day| format!("paid on day {day}"))
                .unwrap_or_else(|| format!(
                    "{} remains unpaid; deadline end of day {}",
                    reais(bill.amount_cents),
                    bill.due_day
                ))
        ));
    }
    if c.support.rent_negotiated {
        text.push_str("The one-time rent extension moved the deadline to day 10; it did not cancel the debt.\n");
    }
    if c.betting.history.is_empty() {
        text.push_str("No wagers were placed. The campaign never required a bet.\n");
    } else {
        let net: i128 = c
            .betting
            .history
            .iter()
            .map(|round| i128::from(round.net_cents))
            .sum();
        text.push_str(&format!(
            "{} wagers; lifetime betting net {}R${}.{:02}.\n",
            c.betting.history.len(),
            if net < 0 { "-" } else { "" },
            net.unsigned_abs() / 100,
            net.unsigned_abs() % 100
        ));
    }
    let kept = c
        .commitments
        .iter()
        .filter(|plan| plan.status == CommitmentStatus::Completed)
        .count();
    let missed = c
        .commitments
        .iter()
        .filter(|plan| plan.status == CommitmentStatus::Missed)
        .count();
    text.push_str(&format!("\nConnections\nRelationships: {} / 100. Scheduled plans kept: {kept}; missed: {missed}. {}\n", c.attributes.relationships,
        if c.attributes.relationships >= 80 { "Making time has kept connections strong." }
        else if c.attributes.relationships < 45 { "There is distance to work through; reconnection can take time." }
        else { "There are connections to keep making time for." }));
    let owned: Vec<_> = super::household::HOUSEHOLD_ITEMS
        .into_iter()
        .filter(|item| c.household.owns(*item))
        .map(HouseholdItem::label)
        .collect();
    text.push_str(&format!(
        "\nHome\nHousehold groups retained: {} / 6. {}\nFood remaining: {} portions. Pipoca: {}.\n",
        owned.len(),
        if owned.is_empty() {
            "All six groups were sold.".into()
        } else {
            format!("At home: {}.", owned.join(", "))
        },
        c.household.food_portions,
        if c.household.owns(HouseholdItem::Pipoca) {
            "still with Alex"
        } else {
            "sold"
        }
    ));
    text.push_str(&format!("\nCare and support\nCare conversations: {}. Financial planning steps: {}. {}\nCurrent wagering cap: {}. App block at the end: {}.\nUrge: {} / 100. A number or one conversation cannot establish recovery; care is an ongoing process.\n",
        c.support.care_actions, c.support.planning_actions,
        if c.support.care_actions > 1 { "Repeated care steps were part of these days." }
        else if c.support.care_actions == 1 { "One care step was a beginning." }
        else { "No care conversation was recorded; support information remained available." },
        c.support.daily_cap_cents.map(reais).unwrap_or_else(|| "none".into()),
        if c.support.app_blocked_today { "on" } else { "off" }, c.attributes.urge));
    if ending == Ending::DogSale {
        text.push_str(&format!(
            "\nPartial final day {} / {} PA used\nUnused PA were not elapsed.\n{}\n",
            c.clock.day,
            c.clock.pa_spent,
            c.daily_finances(c.clock.day)
                .map(|f| finances_text(&f))
                .unwrap_or_else(|error| format!("Daily totals unavailable: {error}"))
        ));
    }
    text.push_str("\nThe campaign is now read-only. Financial history and every completed daily report remain available.");
    text
}

fn present(
    mut commands: Commands,
    skin: Res<HudSkin>,
    store: Res<CampaignStore>,
    overlay: Res<GameplayOverlay>,
    context: Res<Context>,
    existing: Query<Entity, With<RoutineUi>>,
    mut previous: Local<String>,
) {
    let Some(campaign) = store.campaign() else {
        return;
    };
    let choices = options(campaign, &overlay, &context, &store.notice);
    let modal = campaign.stage != CampaignStage::Playing
        || overlay.active == Some(OverlayKind::Context)
        || overlay.active == Some(OverlayKind::Transition);
    let heading = match (campaign.stage, context.records) {
        (_, Some(RecordView::History)) => history_text(campaign),
        (_, Some(RecordView::Reports)) => reports_text(campaign),
        (CampaignStage::DailyReport, None) => {
            campaign.reports.last().map(report_text).unwrap_or_default()
        }
        (CampaignStage::Ended(ending), None) => ending_text(campaign, ending),
        (CampaignStage::Playing, None) if overlay.active == Some(OverlayKind::Transition) => {
            context.message.clone()
        }
        (CampaignStage::Playing, None) => format!(
            "{}\n\n{}{}",
            context.opened.map_or("Apartment", TargetKind::label),
            if context.opened == Some(TargetKind::Door) && campaign.commitment_due(Contact::Rafa) {
                "Rafa is waiting for this afternoon's planned visit.\n\n"
            } else {
                ""
            },
            context.message
        ),
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
    if *previous == signature && !existing.is_empty() {
        return;
    }
    *previous = signature;
    for entity in &existing {
        commands.entity(entity).despawn();
    }
    if !modal && overlay.active.is_none() {
        hud::spawn(&mut commands, campaign, &skin);
        if let Some(target) = context.nearby {
            hud::interaction(&mut commands, target.label(), &skin);
        }
    }
    if !store.notice.is_empty() && !modal {
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
                Text::new(store.notice.clone()),
                TextFont::from_font_size(18.0),
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
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.45)),
            ZIndex(30),
        ))
        .with_children(|parent| {
            parent
                .spawn((
                    Node {
                        width: percent(90),
                        max_width: px(760),
                        height: if campaign.stage != CampaignStage::Playing {
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
                                Text::new(format!(
                                    "Wheel / PageUp / PageDown: scroll text\n\n{heading}"
                                )),
                                TextFont::from_font_size(19.0),
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
                                    TextFont::from_font_size(18.0),
                                    TextColor(Color::WHITE),
                                    Pickable::IGNORE,
                                ));
                            });
                    }
                    if !store.notice.is_empty() {
                        panel.spawn((
                            Text::new(store.notice.clone()),
                            TextFont::from_font_size(17.0),
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

    fn healthy_campaign() -> Campaign {
        let mut c = Campaign::new(5);
        for day in 1..=10 {
            if [1, 2, 3, 5, 6, 7, 8].contains(&day) {
                c.apply(Action::Work).unwrap();
            }
            if [4, 9].contains(&day) {
                c.apply(Action::Wait).unwrap();
                c.apply(Action::Wait).unwrap();
                c.apply(Action::ReceiveVisit(Contact::Rafa)).unwrap();
            }
            if [2, 7].contains(&day) {
                c.apply(Action::Call(Contact::Marina)).unwrap();
            }
            if [3, 5, 8].contains(&day) {
                c.apply(Action::Groceries).unwrap();
            }
            c.apply(Action::Cook).unwrap();
            if day == 4 {
                c.apply(Action::PayBill(BillKind::Electricity)).unwrap();
            }
            if day == 7 {
                c.apply(Action::PayBill(BillKind::Rent)).unwrap();
            }
            while c.clock.remaining_pa() > 0 {
                c.apply(Action::WatchTv).unwrap();
            }
            c.apply(Action::Sleep).unwrap();
            c.apply(Action::AcknowledgeReport).unwrap();
            c = super::super::campaign_save::decode_campaign(
                &super::super::campaign_save::encode_campaign(&c).unwrap(),
            )
            .unwrap();
        }
        c
    }

    #[test]
    fn healthy_ten_day_reports_and_ending_use_actual_facts_and_readable_names() {
        let c = healthy_campaign();
        assert_eq!(
            (
                c.balance_cents,
                c.household.food_portions,
                c.attributes.relationships,
                c.attributes.urge
            ),
            (4_000, 2, 97, 10)
        );
        assert_eq!(c.rng, Campaign::new(5).rng);
        let ending = ending_text(&c, Ending::DayTen);
        for expected in [
            "R$40.00",
            "Relationships: 97",
            "kept: 4; missed: 0",
            "retained: 6 / 6",
            "No wagers were placed",
            "Urge: 10",
        ] {
            assert!(ending.contains(expected), "{expected}: {ending}");
        }
        let reports = reports_text(&c);
        for day in 1..=10 {
            assert!(reports.contains(&format!("DAY {day} - DAILY REPORT")));
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
        assert!(reports.contains("Marina's phone call"));
        assert!(reports.contains("Rafa's visit at the door"));
        assert!(!reports.contains("[]"));
        assert!(ending.is_ascii() && reports.is_ascii());
    }

    #[test]
    fn history_names_unavailable_daily_totals_instead_of_omitting_the_day() {
        let mut c = Campaign::new(5);
        for _ in 0..2 {
            c.ledger.push(LedgerEntry {
                clock: c.clock,
                kind: LedgerKind::BetPayout,
                amount_cents: i64::MAX,
                balance_after_cents: 0,
            });
        }
        let text = history_text(&c);
        assert!(text.contains("Day 1\nDaily totals unavailable: Daily total is too large."));
    }

    #[test]
    fn endings_keep_partial_days_debts_and_gradual_care_truthful() {
        let mut dog = Campaign::new(5);
        dog.attributes.stress = 90;
        for item in super::super::household::HOUSEHOLD_ITEMS {
            dog.apply(Action::Sell(item)).unwrap();
        }
        dog.apply(Action::Sleep).unwrap();
        dog.apply(Action::AcknowledgeReport).unwrap();
        dog.apply(Action::Sell(HouseholdItem::Pipoca)).unwrap();
        let text = ending_text(&dog, Ending::DogSale);
        assert!(text.contains("Partial final day 2 / 1 PA used"));
        assert!(text.contains("Unused PA were not elapsed"));
        assert!(text.contains("Sale proceeds: +R$200.00"));
        assert_eq!(dog.reports.len(), 1);
        let mut lost = Campaign::new(5);
        for _ in 1..9 {
            lost.apply(Action::Sleep).unwrap();
            lost.apply(Action::AcknowledgeReport).unwrap();
        }
        assert_eq!(lost.stage, CampaignStage::Ended(Ending::HousingLoss));
        assert!(ending_text(&lost, Ending::HousingLoss).contains("Day 9 began with rent unpaid"));
        let mut extended = Campaign::new(5);
        extended.apply(Action::NegotiateRent).unwrap();
        extended.apply(Action::CareConversation).unwrap();
        extended.apply(Action::CareConversation).unwrap();
        assert_eq!(extended.attributes.urge, 0);
        for _ in 1..=10 {
            extended.apply(Action::Sleep).unwrap();
            extended.apply(Action::AcknowledgeReport).unwrap();
        }
        let text = ending_text(&extended, Ending::DayTen);
        assert!(text.contains("R$420.00 remains unpaid; deadline end of day 10"));
        assert!(text.contains("Repeated care steps"));
        assert!(text.contains("cannot establish recovery"));
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
        c.apply(Action::Work).unwrap();
        c.apply(Action::Sleep).unwrap();
        for original in [c, healthy_campaign()] {
            let mut app = App::new();
            app.add_plugins(MinimalPlugins)
                .init_resource::<ButtonInput<KeyCode>>()
                .init_resource::<ButtonInput<MouseButton>>()
                .init_resource::<Context>()
                .init_resource::<GameplayOverlay>()
                .init_resource::<NextState<GameState>>()
                .insert_resource(CampaignStore::for_test(
                    Some(original.clone()),
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
            for key in [
                KeyCode::Digit1,
                KeyCode::Enter,
                KeyCode::Digit2,
                KeyCode::Enter,
            ] {
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
            } else {
                assert_eq!(
                    app.world().resource::<CampaignStore>().campaign(),
                    Some(&original)
                );
                press(&mut app, KeyCode::Digit3);
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
    }

    #[test]
    fn door_and_kitchen_keep_delivery_available_and_money_format_keeps_sign() {
        let mut campaign = Campaign::new(5);
        campaign.household.sold.push(HouseholdItem::Fridge);
        campaign.household.food_portions = 0;
        campaign
            .record_money(
                LedgerKind::Sale(HouseholdItem::Fridge),
                HouseholdItem::Fridge.sale_cents(),
            )
            .unwrap();
        let overlay = GameplayOverlay {
            active: Some(OverlayKind::Context),
            ..default()
        };
        for kind in [TargetKind::Door, TargetKind::Food, TargetKind::Cleaning] {
            let context = Context {
                opened: Some(kind),
                ..default()
            };
            assert!(
                options(&campaign, &overlay, &context, "")
                    .iter()
                    .any(|(_, command)| matches!(command, UiCommand::Apply(Action::OrderFood)))
            );
        }
        assert_eq!(reais(-80), "-R$0.80");
        assert_eq!(reais(12_345), "R$123.45");
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
    fn visit_dispatch_requires_the_current_door_and_a_fresh_press() {
        let mut campaign = Campaign::new(5);
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
    fn fifth_door_shortcut_answers_the_visit_and_then_disappears() {
        let mut campaign = Campaign::new(5);
        for _ in 1..4 {
            campaign.apply(Action::Sleep).unwrap();
            campaign.apply(Action::AcknowledgeReport).unwrap();
        }
        campaign.apply(Action::Wait).unwrap();
        campaign.apply(Action::Wait).unwrap();
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
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
            .press(KeyCode::Digit5);
        app.update();
        let campaign = app.world().resource::<CampaignStore>().campaign().unwrap();
        assert_eq!(campaign.clock.pa_spent, 3);
        assert_eq!(campaign.commitments[1].status, CommitmentStatus::Completed);
        let choices = options(
            campaign,
            app.world().resource::<GameplayOverlay>(),
            app.world().resource::<Context>(),
            "",
        );
        assert!(
            !choices
                .iter()
                .any(|(_, command)| *command
                    == UiCommand::Apply(Action::ReceiveVisit(Contact::Rafa)))
        );
    }
}
