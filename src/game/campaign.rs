//! Authoritative campaign data. Gameplay changes enter through `Campaign::apply`.
use serde::{Deserialize, Serialize};

pub const DAYS: u8 = 10;
pub const PA_PER_DAY: u8 = 6;
pub const INITIAL_BALANCE_CENTS: i64 = 12_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Period {
    Morning,
    Afternoon,
    Night,
}

impl Period {
    pub fn label(self) -> &'static str {
        match self {
            Self::Morning => "Morning",
            Self::Afternoon => "Afternoon",
            Self::Night => "Night",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Clock {
    pub day: u8,
    pub pa_spent: u8,
}

impl Clock {
    pub fn period(self) -> Period {
        match self.pa_spent {
            0..=1 => Period::Morning,
            2..=3 => Period::Afternoon,
            _ => Period::Night,
        }
    }

    pub fn remaining_pa(self) -> u8 {
        PA_PER_DAY.saturating_sub(self.pa_spent)
    }

    fn validate(self) -> Result<(), String> {
        if !(1..=DAYS).contains(&self.day) || self.pa_spent > PA_PER_DAY {
            return Err("Campaign clock is outside the ten-day calendar.".into());
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Attributes {
    pub hunger: u8,
    pub boredom: u8,
    pub stress: u8,
    pub urge: u8,
    pub relationships: u8,
}

impl Attributes {
    pub fn adjust(value: &mut u8, change: i16) {
        *value = i16::from(*value).saturating_add(change).clamp(0, 100) as u8;
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Ending {
    DogSale,
    HousingLoss,
    DayTen,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CampaignStage {
    Playing,
    DailyReport,
    Ended(Ending),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BillKind {
    Electricity,
    Rent,
}

impl BillKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Electricity => "Electricity",
            Self::Rent => "Rent",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Bill {
    pub kind: BillKind,
    pub amount_cents: i64,
    pub due_day: u8,
    pub paid_day: Option<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Contact {
    Marina,
    Rafa,
}

impl Contact {
    pub fn label(self) -> &'static str {
        match self {
            Self::Marina => "Marina",
            Self::Rafa => "Rafa",
        }
    }
    pub fn plan_label(self) -> &'static str {
        match self {
            Self::Marina => "Marina's phone call",
            Self::Rafa => "Rafa's visit at the door",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CommitmentStatus {
    Scheduled,
    Completed,
    Missed,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Commitment {
    pub contact: Contact,
    pub day: u8,
    pub period: Period,
    pub status: CommitmentStatus,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum HouseholdItem {
    Tv,
    Fridge,
    Stove,
    Cookware,
    Couch,
    Books,
    Pipoca,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Household {
    pub food_portions: u16,
    pub mess: u8,
    pub sold: Vec<HouseholdItem>,
}

impl Household {
    pub fn owns(&self, item: HouseholdItem) -> bool {
        !self.sold.contains(&item)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PendingCap {
    pub cents: Option<i64>,
    pub effective_day: u8,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SupportTools {
    pub daily_cap_cents: Option<i64>,
    pub pending_cap: Option<PendingCap>,
    pub app_blocked_today: bool,
    pub rent_negotiated: bool,
    pub care_actions: u16,
    pub planning_actions: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BetChoice {
    RouletteRed,
    RouletteBlack,
    Slots,
    FootballHome,
    FootballDraw,
    FootballAway,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BetOutcome {
    RouletteRed,
    RouletteBlack,
    SlotsWin,
    SlotsLoss,
    FootballHome,
    FootballDraw,
    FootballAway,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BetRound {
    pub choice: BetChoice,
    pub outcome: BetOutcome,
    pub stake_cents: i64,
    pub total_return_cents: i64,
    pub net_cents: i64,
    pub session_id: u32,
    pub session_round: u16,
    pub clock: Clock,
    pub pa_consumed: u8,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BetSession {
    pub id: u32,
    pub rounds: u16,
    pub paid_rounds_remaining: u8,
    pub last_stake_cents: i64,
    pub last_was_loss: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TemporaryRelief {
    pub day: u8,
    pub period: Period,
    pub boredom: u8,
    pub stress: u8,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Betting {
    pub session: Option<BetSession>,
    pub history: Vec<BetRound>,
    pub next_session_id: u32,
    pub relief: Option<TemporaryRelief>,
    pub last_session_relief_period: Option<(u8, Period)>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum LedgerKind {
    Work,
    Food,
    BetStake,
    BetPayout,
    BillPayment(BillKind),
    Sale(HouseholdItem),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LedgerEntry {
    pub clock: Clock,
    pub kind: LedgerKind,
    pub amount_cents: i64,
    pub balance_after_cents: i64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DailyFinances {
    pub opening_cents: i64,
    pub work_cents: i64,
    pub food_cents: i64,
    pub gross_bets_cents: i64,
    pub payouts_cents: i64,
    pub bills_cents: i64,
    pub sales_cents: i64,
    pub closing_cents: i64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DailyReport {
    pub day: u8,
    pub finances: DailyFinances,
    pub pa_used: u8,
    pub relationships_opening: u8,
    pub relationships_closing: u8,
    pub overdue_bills: Vec<BillKind>,
    pub completed_commitments: Vec<Contact>,
    pub missed_commitments: Vec<Contact>,
    pub consequences: Vec<String>,
}

/// SplitMix64's complete state; rejection sampling avoids modulo bias.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CampaignRng {
    state: u64,
}

impl CampaignRng {
    pub fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9e3779b97f4a7c15);
        let mut value = self.state;
        value = (value ^ (value >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94d049bb133111eb);
        value ^ (value >> 31)
    }

    pub fn sample_below(&mut self, upper: u64) -> Result<u64, String> {
        if upper == 0 {
            return Err("Random outcome partition must not be empty.".into());
        }
        let threshold = upper.wrapping_neg() % upper;
        loop {
            let value = self.next_u64();
            if value >= threshold {
                return Ok(value % upper);
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Campaign {
    pub clock: Clock,
    pub stage: CampaignStage,
    pub balance_cents: i64,
    pub attributes: Attributes,
    pub household: Household,
    pub bills: Vec<Bill>,
    pub commitments: Vec<Commitment>,
    pub worked_days: Vec<u8>,
    pub support: SupportTools,
    pub betting: Betting,
    pub ledger: Vec<LedgerEntry>,
    pub reports: Vec<DailyReport>,
    pub day_opening_relationships: u8,
    pub rng: CampaignRng,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Wait,
    Bet { choice: BetChoice, stake_cents: i64 },
    EndBetSession,
    Work,
    Groceries,
    Cook,
    OrderFood,
    WatchTv,
    TimeWithPipoca,
    Sell(HouseholdItem),
    Clean,
    PayBill(BillKind),
    NegotiateRent,
    Call(Contact),
    ReceiveVisit(Contact),
    CareConversation,
    FinancialPlanning,
    SetDailyCap(Option<i64>),
    BlockBetting,
    Sleep,
    AcknowledgeReport,
}

impl Campaign {
    pub fn new(seed: u64) -> Self {
        Self {
            clock: Clock {
                day: 1,
                pa_spent: 0,
            },
            stage: CampaignStage::Playing,
            balance_cents: INITIAL_BALANCE_CENTS,
            attributes: Attributes {
                hunger: 25,
                boredom: 30,
                stress: 25,
                urge: 10,
                relationships: 65,
            },
            household: Household {
                food_portions: 3,
                mess: 0,
                sold: vec![],
            },
            bills: vec![
                Bill {
                    kind: BillKind::Electricity,
                    amount_cents: 10_000,
                    due_day: 4,
                    paid_day: None,
                },
                Bill {
                    kind: BillKind::Rent,
                    amount_cents: 42_000,
                    due_day: 7,
                    paid_day: None,
                },
            ],
            commitments: [
                (Contact::Marina, 2),
                (Contact::Rafa, 4),
                (Contact::Marina, 7),
                (Contact::Rafa, 9),
            ]
            .into_iter()
            .map(|(contact, day)| Commitment {
                contact,
                day,
                period: Period::Afternoon,
                status: CommitmentStatus::Scheduled,
            })
            .collect(),
            worked_days: vec![],
            support: SupportTools {
                daily_cap_cents: None,
                pending_cap: None,
                app_blocked_today: false,
                rent_negotiated: false,
                care_actions: 0,
                planning_actions: 0,
            },
            betting: Betting {
                session: None,
                history: vec![],
                next_session_id: 1,
                relief: None,
                last_session_relief_period: None,
            },
            ledger: vec![],
            reports: vec![],
            day_opening_relationships: 65,
            rng: CampaignRng::new(seed),
        }
    }

    /// A rejected action leaves every field, including RNG state, untouched.
    pub fn apply(&mut self, action: Action) -> Result<(), String> {
        self.validate()?;
        match (self.stage, action) {
            (CampaignStage::Playing, Action::AcknowledgeReport) => {
                return Err("Sleep to open the daily report first.".into());
            }
            (CampaignStage::Playing, _)
            | (CampaignStage::DailyReport, Action::AcknowledgeReport) => {}
            (CampaignStage::DailyReport, _) => {
                return Err("Continue from the daily report before taking another action.".into());
            }
            (CampaignStage::Ended(_), _) => {
                return Err(
                    "This campaign has ended. History and reports remain available.".into(),
                );
            }
        }
        let mut next = self.clone();
        if !matches!(action, Action::Bet { .. } | Action::EndBetSession) {
            next.betting.session = None;
        }
        match action {
            Action::Bet {
                choice,
                stake_cents,
            } => next.settle_bet(choice, stake_cents)?,
            Action::EndBetSession => next.betting.session = None,
            Action::Wait => next.spend_pa(1)?,
            Action::Work => {
                if ![1, 2, 3, 5, 6, 7, 8].contains(&next.clock.day) {
                    return Err(
                        "MeFood has no shift today. Shifts run on days 1, 2, 3, 5, 6, 7 and 8."
                            .into(),
                    );
                }
                if next.worked_days.contains(&next.clock.day) {
                    return Err("Today's MeFood shift is already complete.".into());
                }
                next.adjust_stress(4);
                next.spend_pa(2)?;
                next.record_money(LedgerKind::Work, 8_000)?;
                next.worked_days.push(next.clock.day);
            }
            Action::Groceries => {
                next.require_kitchen()?;
                next.spend_pa(1)?;
                next.record_money(LedgerKind::Food, -4_000)?;
                next.household.food_portions = next
                    .household
                    .food_portions
                    .checked_add(3)
                    .ok_or("There is no more room for groceries.")?;
            }
            Action::Cook => {
                next.require_kitchen()?;
                if next.household.food_portions == 0 {
                    return Err(
                        "No food portions left. Buy groceries or order prepared food.".into(),
                    );
                }
                next.spend_pa(1)?;
                next.household.food_portions -= 1;
                next.eat();
            }
            Action::OrderFood => {
                next.spend_pa(1)?;
                next.record_money(LedgerKind::Food, -2_000)?;
                next.eat();
            }
            Action::WatchTv => {
                if !next.household.owns(HouseholdItem::Tv) {
                    return Err("The TV has been sold. Try another activity.".into());
                }
                if !next.has_power() {
                    return Err("The power is off. Pay the electricity bill to use the TV.".into());
                }
                next.adjust_stress(-3);
                next.spend_pa(1)?;
                next.adjust_boredom(-20);
            }
            Action::TimeWithPipoca => {
                if !next.household.owns(HouseholdItem::Pipoca) {
                    return Err("Pipoca is no longer at home.".into());
                }
                next.adjust_stress(-8);
                next.spend_pa(1)?;
                next.adjust_boredom(-5);
            }
            Action::Sell(item) => next.sell(item)?,
            Action::Clean => {
                next.adjust_stress(-4);
                next.spend_pa(1)?;
                next.household.mess = 0;
            }
            Action::PayBill(kind) => next.pay_bill(kind)?,
            Action::NegotiateRent => next.negotiate_rent()?,
            Action::Call(contact) => next.call_contact(contact)?,
            Action::ReceiveVisit(contact) => next.receive_visit(contact)?,
            Action::CareConversation => next.care_conversation()?,
            Action::FinancialPlanning => next.financial_planning()?,
            Action::SetDailyCap(cents) => next.set_daily_cap(cents)?,
            Action::BlockBetting => {
                if next.support.app_blocked_today {
                    return Err("Betting is already blocked until tomorrow morning.".into());
                }
                next.support.app_blocked_today = true;
            }
            Action::Sleep => next.finish_day()?,
            Action::AcknowledgeReport => next.start_next_day()?,
        }
        next.validate()?;
        *self = next;
        Ok(())
    }

    pub(super) fn spend_pa(&mut self, cost: u8) -> Result<(), String> {
        if cost > self.clock.remaining_pa() {
            return Err(format!(
                "This action needs {cost} PA. Sleep to start the next day; free actions are still available."
            ));
        }
        for _ in 0..cost {
            let previous_period = self.clock.period();
            self.clock.pa_spent += 1;
            if self.betting.relief.as_ref().is_some_and(|relief| {
                (relief.day, relief.period) != (self.clock.day, self.clock.period())
            }) {
                self.expire_relief();
            }
            Attributes::adjust(&mut self.attributes.hunger, 8);
            self.adjust_boredom(3);
            if previous_period != self.clock.period() || self.clock.pa_spent == PA_PER_DAY {
                self.end_period(previous_period);
            }
        }
        Ok(())
    }

    pub fn has_power(&self) -> bool {
        self.clock.day < 6
            || self
                .bills
                .iter()
                .any(|bill| bill.kind == BillKind::Electricity && bill.paid_day.is_some())
    }

    pub(super) fn require_kitchen(&self) -> Result<(), String> {
        if [
            HouseholdItem::Fridge,
            HouseholdItem::Stove,
            HouseholdItem::Cookware,
        ]
        .into_iter()
        .all(|item| self.household.owns(item))
        {
            Ok(())
        } else {
            Err("Cooking and grocery storage need the fridge, stove and cookware. Order prepared food instead.".into())
        }
    }

    fn eat(&mut self) {
        Attributes::adjust(&mut self.attributes.hunger, -45);
        self.household.mess = self.household.mess.saturating_add(1).min(3);
    }

    fn end_period(&mut self, period: Period) {
        let mut missed = 0;
        for commitment in &mut self.commitments {
            if commitment.day == self.clock.day
                && commitment.period == period
                && commitment.status == CommitmentStatus::Scheduled
            {
                commitment.status = CommitmentStatus::Missed;
                Attributes::adjust(&mut self.attributes.relationships, -12);
                missed += 1;
            }
        }
        self.adjust_stress(missed * 6);
        if self.attributes.stress >= 60 || self.attributes.urge >= 60 {
            self.household.mess = self.household.mess.saturating_add(1).min(3);
        }
    }

    fn finish_day(&mut self) -> Result<(), String> {
        let pa_used = self.clock.pa_spent;
        self.spend_pa(self.clock.remaining_pa())?;
        self.expire_relief();
        self.betting.session = None;
        let overdue_bills: Vec<_> = self
            .bills
            .iter()
            .filter(|bill| bill.paid_day.is_none() && bill.due_day <= self.clock.day)
            .map(|bill| bill.kind)
            .collect();
        for _ in &overdue_bills {
            self.adjust_stress(8);
        }
        let mut consequences = vec![];
        if pa_used < PA_PER_DAY {
            consequences.push(format!(
                "Slept early: {} unused PA elapsed for needs and commitments.",
                PA_PER_DAY - pa_used
            ));
        }
        if !self.has_power() {
            consequences
                .push("Electricity is disconnected; paying the bill restores power.".into());
        }
        if self.clock.day == 5 && overdue_bills.contains(&BillKind::Electricity) {
            consequences.push("Electricity warning: cutoff starts on day 6 if unpaid.".into());
        }
        if self.clock.day == 8 && overdue_bills.contains(&BillKind::Rent) {
            consequences.push("Rent warning: unresolved rent causes housing loss at the start of day 9. A one-time negotiation can extend the deadline.".into());
        }
        consequences.push(format!(
            "At day end: hunger {}, boredom {}, stress {}, urge {} (0-100). Food: {} portions. Mess: {}/3.",
            self.attributes.hunger, self.attributes.boredom, self.attributes.stress,
            self.attributes.urge, self.household.food_portions, self.household.mess,
        ));
        for bill in &self.bills {
            consequences.push(format!(
                "{}: {}.",
                bill.kind.label(),
                bill.paid_day
                    .map(|day| format!("paid on day {day}"))
                    .unwrap_or_else(|| format!("unpaid, due end of day {}", bill.due_day))
            ));
        }
        let sold_today: Vec<_> = self
            .ledger
            .iter()
            .filter_map(|row| {
                if row.clock.day == self.clock.day
                    && let LedgerKind::Sale(item) = row.kind
                {
                    Some(item.label())
                } else {
                    None
                }
            })
            .collect();
        if !sold_today.is_empty() {
            consequences.push(format!(
                "Permanently sold today: {}.",
                sold_today.join(", ")
            ));
        }
        consequences.push(if self.household.sold.is_empty() {
            "All six household groups and Pipoca remain at home.".into()
        } else {
            format!(
                "Sold by day end: {}. Pipoca remains at home.",
                self.household
                    .sold
                    .iter()
                    .map(|item| item.label())
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        });
        if self.require_kitchen().is_err() {
            consequences.push("Cooking and grocery storage are unavailable; prepared-food delivery remains available.".into());
        }
        consequences.push(format!("Care conversations so far: {}. Financial planning steps so far: {}. Care is gradual; these counts do not prove recovery.",
            self.support.care_actions, self.support.planning_actions));
        consequences.push(format!(
            "Wagering cap at day end: {}. Betting app: {}.",
            self.support
                .daily_cap_cents
                .map(|cents| format!("R${}.{:02}", cents / 100, cents % 100))
                .unwrap_or_else(|| "none".into()),
            if self.support.app_blocked_today {
                "blocked until morning"
            } else {
                "available"
            }
        ));
        if let Some(cap) = &self.support.pending_cap {
            consequences.push(format!(
                "Pending cap for day {} morning: {}.",
                cap.effective_day,
                cap.cents
                    .map(|cents| format!("R${}.{:02}", cents / 100, cents % 100))
                    .unwrap_or_else(|| "no cap".into())
            ));
        }
        if self.betting.history.is_empty() {
            consequences
                .push("No wagers have been placed. Urge does not rise with the calendar.".into());
        }
        let contacts = |status| {
            self.commitments
                .iter()
                .filter(|commitment| {
                    commitment.day == self.clock.day && commitment.status == status
                })
                .map(|commitment| commitment.contact)
                .collect()
        };
        self.reports.push(DailyReport {
            day: self.clock.day,
            finances: self.daily_finances(self.clock.day)?,
            pa_used,
            relationships_opening: self.day_opening_relationships,
            relationships_closing: self.attributes.relationships,
            overdue_bills,
            completed_commitments: contacts(CommitmentStatus::Completed),
            missed_commitments: contacts(CommitmentStatus::Missed),
            consequences,
        });
        self.stage = CampaignStage::DailyReport;
        Ok(())
    }

    fn start_next_day(&mut self) -> Result<(), String> {
        if self
            .reports
            .last()
            .is_none_or(|report| report.day != self.clock.day)
        {
            return Err("The daily report is missing; the next day cannot start.".into());
        }
        if self.clock.day == DAYS {
            self.stage = CampaignStage::Ended(Ending::DayTen);
            return Ok(());
        }
        self.clock.day += 1;
        self.clock.pa_spent = 0;
        self.day_opening_relationships = self.attributes.relationships;
        self.support.app_blocked_today = false;
        if self
            .support
            .pending_cap
            .as_ref()
            .is_some_and(|cap| cap.effective_day == self.clock.day)
        {
            self.support.daily_cap_cents = self.support.pending_cap.take().unwrap().cents;
        }
        self.stage = if self.clock.day >= 9
            && self.bills.iter().any(|bill| {
                bill.kind == BillKind::Rent
                    && bill.paid_day.is_none()
                    && bill.due_day < self.clock.day
            }) {
            CampaignStage::Ended(Ending::HousingLoss)
        } else {
            CampaignStage::Playing
        };
        Ok(())
    }

    pub(super) fn expire_relief(&mut self) {
        if let Some(relief) = self.betting.relief.take() {
            Attributes::adjust(&mut self.attributes.boredom, i16::from(relief.boredom));
            Attributes::adjust(&mut self.attributes.stress, i16::from(relief.stress));
        }
    }

    /// Only reducers call this; the enclosing action clone provides atomicity.
    pub(super) fn record_money(
        &mut self,
        kind: LedgerKind,
        amount_cents: i64,
    ) -> Result<(), String> {
        validate_ledger_amount(kind, amount_cents)?;
        let balance = self
            .balance_cents
            .checked_add(amount_cents)
            .filter(|balance| *balance >= 0)
            .ok_or("Insufficient balance or amount is too large.")?;
        self.ledger.push(LedgerEntry {
            clock: self.clock,
            kind,
            amount_cents,
            balance_after_cents: balance,
        });
        self.balance_cents = balance;
        Ok(())
    }

    pub fn daily_finances(&self, day: u8) -> Result<DailyFinances, String> {
        if !(1..=self.clock.day).contains(&day) {
            return Err("Choose a day that has started.".into());
        }
        let opening = self
            .ledger
            .iter()
            .take_while(|row| row.clock.day < day)
            .last()
            .map_or(INITIAL_BALANCE_CENTS, |row| row.balance_after_cents);
        let mut result = DailyFinances {
            opening_cents: opening,
            closing_cents: opening,
            ..Default::default()
        };
        for row in self.ledger.iter().filter(|row| row.clock.day == day) {
            let total = match row.kind {
                LedgerKind::Work => &mut result.work_cents,
                LedgerKind::Food => &mut result.food_cents,
                LedgerKind::BetStake => &mut result.gross_bets_cents,
                LedgerKind::BetPayout => &mut result.payouts_cents,
                LedgerKind::BillPayment(_) => &mut result.bills_cents,
                LedgerKind::Sale(_) => &mut result.sales_cents,
            };
            *total = total
                .checked_add(
                    row.amount_cents
                        .checked_abs()
                        .ok_or("Invalid ledger amount.")?,
                )
                .ok_or("Daily total is too large.")?;
            result.closing_cents = row.balance_after_cents;
        }
        Ok(result)
    }

    pub fn validate(&self) -> Result<(), String> {
        self.clock.validate()?;
        if self.balance_cents < 0
            || [
                self.attributes.hunger,
                self.attributes.boredom,
                self.attributes.stress,
                self.attributes.urge,
                self.attributes.relationships,
                self.day_opening_relationships,
            ]
            .into_iter()
            .any(|value| value > 100)
        {
            return Err("Campaign balance or attributes are invalid.".into());
        }
        if self.household.mess > 3
            || self
                .household
                .sold
                .iter()
                .enumerate()
                .any(|(i, item)| self.household.sold[..i].contains(item))
            || (!self.household.owns(HouseholdItem::Fridge) && self.household.food_portions != 0)
        {
            return Err("Household inventory is invalid.".into());
        }
        let mut balance = INITIAL_BALANCE_CENTS;
        let mut previous_clock = Clock {
            day: 1,
            pa_spent: 0,
        };
        for row in &self.ledger {
            row.clock.validate()?;
            validate_ledger_amount(row.kind, row.amount_cents)?;
            balance = balance
                .checked_add(row.amount_cents)
                .filter(|amount| *amount >= 0)
                .ok_or("Ledger balance is invalid.")?;
            if row.clock < previous_clock
                || row.clock > self.clock
                || row.balance_after_cents != balance
            {
                return Err("Financial ledger does not reconcile.".into());
            }
            previous_clock = row.clock;
        }
        if balance != self.balance_cents {
            return Err("Campaign balance does not match the financial ledger.".into());
        }
        self.daily_finances(self.clock.day).map_err(|_| {
            "Daily financial totals exceed the supported range. Reduce the stake or finish the day before betting again."
        })?;
        if self.bills.len() != 2 {
            return Err("The campaign must contain electricity and rent bills.".into());
        }
        for (kind, amount, due) in [
            (BillKind::Electricity, 10_000, 4),
            (
                BillKind::Rent,
                42_000,
                if self.support.rent_negotiated { 10 } else { 7 },
            ),
        ] {
            if self
                .bills
                .iter()
                .filter(|bill| {
                    bill.kind == kind
                        && bill.amount_cents == amount
                        && bill.due_day == due
                        && bill
                            .paid_day
                            .is_none_or(|day| (1..=self.clock.day).contains(&day))
                })
                .count()
                != 1
            {
                return Err("Campaign bill amount, deadline or payment day is invalid.".into());
            }
        }
        if self.commitments.len() != 4
            || [
                (Contact::Marina, 2),
                (Contact::Rafa, 4),
                (Contact::Marina, 7),
                (Contact::Rafa, 9),
            ]
            .into_iter()
            .any(|(contact, day)| {
                self.commitments
                    .iter()
                    .filter(|commitment| {
                        commitment.contact == contact
                            && commitment.day == day
                            && commitment.period == Period::Afternoon
                    })
                    .count()
                    != 1
            })
        {
            return Err("The social calendar is invalid.".into());
        }
        if self.worked_days.iter().enumerate().any(|(i, day)| {
            ![1, 2, 3, 5, 6, 7, 8].contains(day)
                || *day > self.clock.day
                || self.worked_days[..i].contains(day)
        }) {
            return Err("Work history is invalid.".into());
        }
        self.validate_support()?;
        self.validate_household()?;
        self.validate_obligations()?;
        self.validate_betting()?;
        for (i, report) in self.reports.iter().enumerate() {
            if report.day as usize != i + 1
                || report.day > self.clock.day
                || report.pa_used > PA_PER_DAY
                || report.relationships_opening > 100
                || report.relationships_closing > 100
                || report.finances != self.daily_finances(report.day)?
            {
                return Err("A daily report does not match the campaign history.".into());
            }
        }
        Ok(())
    }
}

fn validate_ledger_amount(kind: LedgerKind, amount: i64) -> Result<(), String> {
    let income = matches!(
        kind,
        LedgerKind::Work | LedgerKind::BetPayout | LedgerKind::Sale(_)
    );
    if amount == 0 || amount == i64::MIN || (amount > 0) != income {
        return Err("Financial entry has an invalid amount or direction.".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_consequences_keep_day_end_facts_after_later_changes_and_reload() {
        use crate::game::campaign_save::{decode_campaign, encode_campaign};
        let mut c = Campaign::new(5);
        c.attributes.stress = 90;
        c.apply(Action::Sell(HouseholdItem::Tv)).unwrap();
        c.apply(Action::CareConversation).unwrap();
        c.apply(Action::FinancialPlanning).unwrap();
        c.apply(Action::SetDailyCap(Some(2_000))).unwrap();
        c.apply(Action::BlockBetting).unwrap();
        c.apply(Action::Sleep).unwrap();
        let original = c.reports[0].clone();
        let facts = original.consequences.join("\n");
        assert!(facts.contains("Electricity: unpaid, due end of day 4"));
        assert!(facts.contains("Permanently sold today: TV"));
        assert!(facts.contains("Care conversations so far: 1"));
        assert!(facts.contains("R$20.00. Betting app: blocked until morning"));
        assert_eq!(original.pa_used, 3);
        c.apply(Action::AcknowledgeReport).unwrap();
        c.apply(Action::PayBill(BillKind::Electricity)).unwrap();
        c.apply(Action::CareConversation).unwrap();
        c.apply(Action::SetDailyCap(Some(5_000))).unwrap();
        c.attributes.stress = 90;
        c.apply(Action::Sell(HouseholdItem::Fridge)).unwrap();
        c = decode_campaign(&encode_campaign(&c).unwrap()).unwrap();
        assert_eq!(c.reports[0], original);
        assert_eq!(c.support.care_actions, 2);
        assert_eq!(c.household.food_portions, 0);
        assert_eq!(c.bills[0].paid_day, Some(2));
    }

    #[test]
    fn initial_resources_and_action_clock() {
        let mut campaign = Campaign::new(5);
        assert_eq!(campaign.balance_cents, 12_000);
        assert_eq!(campaign.household.food_portions, 3);
        assert_eq!(
            campaign.attributes,
            Attributes {
                hunger: 25,
                boredom: 30,
                stress: 25,
                urge: 10,
                relationships: 65
            }
        );
        for expected in [
            Period::Morning,
            Period::Afternoon,
            Period::Afternoon,
            Period::Night,
            Period::Night,
            Period::Night,
        ] {
            campaign.apply(Action::Wait).unwrap();
            assert_eq!(campaign.clock.period(), expected);
        }
        assert_eq!(campaign.attributes.hunger, 73);
        assert_eq!(campaign.attributes.boredom, 48);
        let before = campaign.clone();
        assert!(campaign.apply(Action::Wait).unwrap_err().contains("Sleep"));
        assert_eq!(campaign, before);
    }

    #[test]
    fn invalid_and_finished_campaigns_reject_without_mutation() {
        for stage in [
            CampaignStage::DailyReport,
            CampaignStage::Ended(Ending::DayTen),
        ] {
            let mut campaign = Campaign::new(5);
            campaign.stage = stage;
            let before = campaign.clone();
            assert!(campaign.apply(Action::Wait).is_err());
            assert_eq!(campaign, before);
        }
        let mut campaign = Campaign::new(5);
        campaign.balance_cents = -1;
        let before = campaign.clone();
        assert!(campaign.apply(Action::Wait).is_err());
        assert_eq!(campaign, before);
    }

    #[test]
    fn action_clock_expires_old_relief_at_the_period_boundary() {
        let mut campaign = Campaign::new(5);
        campaign.attributes.boredom -= 8;
        campaign.attributes.stress -= 4;
        campaign.betting.relief = Some(TemporaryRelief {
            day: 1,
            period: Period::Morning,
            boredom: 8,
            stress: 4,
        });
        campaign.apply(Action::Wait).unwrap();
        assert!(campaign.betting.relief.is_some());
        campaign.apply(Action::Wait).unwrap();
        assert!(campaign.betting.relief.is_none());
        assert_eq!(campaign.attributes.boredom, 36);
        assert_eq!(campaign.attributes.stress, 25);
    }

    #[test]
    fn financial_entries_and_daily_totals_reconcile_in_cents() {
        let mut campaign = Campaign::new(5);
        campaign.clock.pa_spent = 1;
        campaign.betting.next_session_id = 2;
        campaign.betting.history.push(BetRound {
            choice: BetChoice::RouletteRed,
            outcome: BetOutcome::RouletteRed,
            stake_cents: 100,
            total_return_cents: 180,
            net_cents: 80,
            session_id: 1,
            session_round: 1,
            clock: campaign.clock,
            pa_consumed: 1,
        });
        for (kind, amount) in [
            (LedgerKind::Work, 8_000),
            (LedgerKind::Food, -4_000),
            (LedgerKind::BetStake, -100),
            (LedgerKind::BetPayout, 180),
            (LedgerKind::BillPayment(BillKind::Electricity), -10_000),
            (LedgerKind::Sale(HouseholdItem::Books), 2_000),
        ] {
            campaign.record_money(kind, amount).unwrap();
        }
        campaign.bills[0].paid_day = Some(1);
        campaign.household.sold.push(HouseholdItem::Books);
        campaign.validate().unwrap();
        assert_eq!(
            campaign.daily_finances(1).unwrap(),
            DailyFinances {
                opening_cents: 12_000,
                work_cents: 8_000,
                food_cents: 4_000,
                gross_bets_cents: 100,
                payouts_cents: 180,
                bills_cents: 10_000,
                sales_cents: 2_000,
                closing_cents: 8_080
            }
        );
        campaign.clock.day = 2;
        assert_eq!(campaign.daily_finances(2).unwrap().opening_cents, 8_080);
        let before = campaign.clone();
        assert!(campaign.record_money(LedgerKind::Food, -8_081).is_err());
        assert!(campaign.record_money(LedgerKind::Work, -1).is_err());
        assert!(campaign.record_money(LedgerKind::Food, i64::MIN).is_err());
        assert_eq!(campaign, before);
        campaign.ledger[0].balance_after_cents += 1;
        assert!(campaign.validate().is_err());
    }

    #[test]
    fn rng_has_stable_vectors_and_rejects_empty_partitions_atomically() {
        let mut rng = CampaignRng::new(0);
        assert_eq!(rng.next_u64(), 0xe220a8397b1dcdaf);
        assert_eq!(rng.next_u64(), 0x6e789e6aa1b965f4);
        let before = rng.clone();
        assert!(rng.sample_below(0).is_err());
        assert_eq!(rng, before);
        let mut first = CampaignRng::new(42);
        let mut second = first.clone();
        for upper in [1, 2, 10, 100, u64::MAX / 2 + 2, u64::MAX] {
            for _ in 0..100 {
                let value = first.sample_below(upper).unwrap();
                assert!(value < upper);
                assert_eq!(value, second.sample_below(upper).unwrap());
            }
        }
    }

    #[test]
    fn trust_boundary_rejects_invalid_clock_stats_inventory_and_calendar() {
        let base = Campaign::new(5);
        let mut variants = vec![base.clone(); 7];
        variants[0].clock.day = 0;
        variants[1].clock.pa_spent = 7;
        variants[2].attributes.stress = 101;
        variants[3].household.mess = 4;
        variants[4].household.sold.push(HouseholdItem::Fridge);
        variants[5].bills[0].amount_cents = -1;
        variants[6].commitments.pop();
        for campaign in variants {
            assert!(campaign.validate().is_err());
        }
        let mut value = 99;
        Attributes::adjust(&mut value, 8);
        assert_eq!(value, 100);
        Attributes::adjust(&mut value, -150);
        assert_eq!(value, 0);
    }

    fn next_day(campaign: &mut Campaign) {
        campaign.apply(Action::Sleep).unwrap();
        campaign.apply(Action::AcknowledgeReport).unwrap();
    }

    #[test]
    fn routine_budget_costs_and_all_seven_workdays() {
        let mut campaign = Campaign::new(5);
        for action in [
            Action::Work,
            Action::Groceries,
            Action::Cook,
            Action::WatchTv,
            Action::Clean,
        ] {
            campaign.apply(action).unwrap();
        }
        assert_eq!(campaign.clock.remaining_pa(), 0);
        assert_eq!(campaign.balance_cents, 16_000);
        assert_eq!(campaign.household.food_portions, 5);
        assert_eq!(campaign.household.mess, 0);
        assert_eq!(campaign.attributes.hunger, 28);
        assert_eq!(campaign.attributes.boredom, 28);
        assert_eq!(campaign.attributes.stress, 22);
        let finances = campaign.daily_finances(1).unwrap();
        assert_eq!(finances.work_cents, 8_000);
        assert_eq!(finances.food_cents, 4_000);
        assert_eq!(
            finances.closing_cents,
            finances.opening_cents + finances.work_cents - finances.food_cents
        );

        let mut campaign = Campaign::new(8);
        campaign.support.rent_negotiated = true;
        campaign.bills[1].due_day = 10;
        for day in 1..=DAYS {
            if [1, 2, 3, 5, 6, 7, 8].contains(&day) {
                campaign.apply(Action::Work).unwrap();
                let before = campaign.clone();
                assert!(
                    campaign
                        .apply(Action::Work)
                        .unwrap_err()
                        .contains("already complete")
                );
                assert_eq!(campaign, before);
            } else {
                let before = campaign.clone();
                assert!(
                    campaign
                        .apply(Action::Work)
                        .unwrap_err()
                        .contains("no shift")
                );
                assert_eq!(campaign, before);
            }
            next_day(&mut campaign);
        }
        assert_eq!(campaign.worked_days, [1, 2, 3, 5, 6, 7, 8]);
        assert_eq!(campaign.balance_cents, 68_000);
        assert_eq!(campaign.reports.len(), 10);
        assert_eq!(campaign.clock.day, 10);
        assert_eq!(campaign.stage, CampaignStage::Ended(Ending::DayTen));
        let before = campaign.clone();
        assert!(campaign.apply(Action::AcknowledgeReport).is_err());
        assert!(campaign.apply(Action::Sleep).is_err());
        assert_eq!(campaign, before);
    }

    #[test]
    fn failed_routines_leave_money_time_inventory_and_stats_untouched() {
        let mut no_money = Campaign::new(5);
        no_money.record_money(LedgerKind::Food, -12_000).unwrap();
        for action in [Action::Groceries, Action::OrderFood] {
            let before = no_money.clone();
            assert!(no_money.apply(action).is_err());
            assert_eq!(no_money, before);
        }
        let mut campaign = Campaign::new(5);
        campaign.household.food_portions = 0;
        let before = campaign.clone();
        assert!(
            campaign
                .apply(Action::Cook)
                .unwrap_err()
                .contains("No food")
        );
        assert_eq!(campaign, before);
        for _ in 0..5 {
            campaign.apply(Action::Wait).unwrap();
        }
        let before = campaign.clone();
        assert!(campaign.apply(Action::Work).unwrap_err().contains("2 PA"));
        assert_eq!(campaign, before);
        campaign.apply(Action::Wait).unwrap();
        for action in [
            Action::Wait,
            Action::Groceries,
            Action::OrderFood,
            Action::WatchTv,
            Action::Clean,
        ] {
            let before = campaign.clone();
            assert!(campaign.apply(action).is_err());
            assert_eq!(campaign, before);
        }
    }

    #[test]
    fn sold_kitchen_still_allows_prepared_food_and_tv_checks_power() {
        for item in [
            HouseholdItem::Fridge,
            HouseholdItem::Stove,
            HouseholdItem::Cookware,
        ] {
            let mut campaign = Campaign::new(5);
            campaign.household.sold.push(item);
            campaign.household.food_portions = 0;
            campaign
                .record_money(LedgerKind::Sale(item), item.sale_cents())
                .unwrap();
            for action in [Action::Cook, Action::Groceries] {
                let before = campaign.clone();
                assert!(
                    campaign
                        .apply(action)
                        .unwrap_err()
                        .contains("Order prepared food")
                );
                assert_eq!(campaign, before);
            }
            campaign.attributes.hunger = 80;
            campaign.apply(Action::OrderFood).unwrap();
            assert_eq!(campaign.attributes.hunger, 43);
            assert_eq!(campaign.household.food_portions, 0);
            assert_eq!(campaign.household.mess, 1);
            assert_eq!(campaign.balance_cents, 10_000 + item.sale_cents());
        }
        let mut campaign = Campaign::new(5);
        campaign.household.sold.push(HouseholdItem::Tv);
        campaign
            .record_money(
                LedgerKind::Sale(HouseholdItem::Tv),
                HouseholdItem::Tv.sale_cents(),
            )
            .unwrap();
        let before = campaign.clone();
        assert!(
            campaign
                .apply(Action::WatchTv)
                .unwrap_err()
                .contains("sold")
        );
        assert_eq!(campaign, before);
        let mut campaign = Campaign::new(5);
        while campaign.clock.day < 6 {
            next_day(&mut campaign);
        }
        let before = campaign.clone();
        assert!(!campaign.has_power());
        assert!(
            campaign
                .apply(Action::WatchTv)
                .unwrap_err()
                .contains("power is off")
        );
        assert_eq!(campaign, before);
        campaign
            .apply(Action::PayBill(BillKind::Electricity))
            .unwrap();
        assert!(campaign.has_power());
        campaign.apply(Action::WatchTv).unwrap();
    }

    #[test]
    fn early_and_zero_pa_sleep_report_before_advancing_and_count_commitments_once() {
        let mut campaign = Campaign::new(5);
        next_day(&mut campaign);
        let hunger = campaign.attributes.hunger;
        campaign.apply(Action::Sleep).unwrap();
        assert_eq!(
            campaign.clock,
            Clock {
                day: 2,
                pa_spent: 6
            }
        );
        assert_eq!(
            campaign.attributes.hunger,
            hunger.saturating_add(48).min(100)
        );
        assert_eq!(campaign.attributes.relationships, 53);
        assert_eq!(campaign.commitments[0].status, CommitmentStatus::Missed);
        assert_eq!(campaign.reports[1].missed_commitments, [Contact::Marina]);
        assert_eq!(campaign.reports[1].pa_used, 0);
        assert_eq!(campaign.reports[1].relationships_opening, 65);
        assert_eq!(campaign.reports[1].relationships_closing, 53);
        assert_eq!(campaign.stage, CampaignStage::DailyReport);
        let before = campaign.clone();
        assert!(campaign.apply(Action::Wait).is_err());
        assert!(campaign.apply(Action::Sleep).is_err());
        assert_eq!(campaign, before);
        campaign.apply(Action::AcknowledgeReport).unwrap();
        assert_eq!(
            campaign.clock,
            Clock {
                day: 3,
                pa_spent: 0
            }
        );
        assert_eq!(campaign.day_opening_relationships, 53);

        let mut campaign = Campaign::new(5);
        next_day(&mut campaign);
        for _ in 0..3 {
            campaign.apply(Action::Wait).unwrap();
        }
        assert_eq!(campaign.commitments[0].status, CommitmentStatus::Scheduled);
        campaign.apply(Action::Wait).unwrap();
        assert_eq!(campaign.commitments[0].status, CommitmentStatus::Missed);
        campaign.apply(Action::Wait).unwrap();
        campaign.apply(Action::Wait).unwrap();
        let attributes = campaign.attributes.clone();
        campaign.apply(Action::Sleep).unwrap();
        assert_eq!(campaign.attributes, attributes);
        assert_eq!(campaign.reports[1].pa_used, 6);
        assert_eq!(campaign.reports[1].missed_commitments, [Contact::Marina]);
    }

    #[test]
    fn period_mess_is_state_driven_capped_and_cleanable() {
        let mut campaign = Campaign::new(5);
        campaign.attributes.stress = 60;
        campaign.apply(Action::Wait).unwrap();
        assert_eq!(campaign.household.mess, 0);
        campaign.apply(Action::Wait).unwrap();
        assert_eq!(campaign.household.mess, 1);
        campaign.apply(Action::Sleep).unwrap();
        assert_eq!(campaign.household.mess, 3);
        campaign.apply(Action::AcknowledgeReport).unwrap();
        campaign.apply(Action::Clean).unwrap();
        assert_eq!(campaign.household.mess, 0);
        assert_eq!(campaign.attributes.stress, 56);
        campaign.attributes.urge = 60;
        campaign.apply(Action::Wait).unwrap();
        assert_eq!(campaign.household.mess, 1);
        let mut campaign = Campaign::new(5);
        campaign.apply(Action::Wait).unwrap();
        campaign.attributes.stress = 60;
        campaign.apply(Action::WatchTv).unwrap();
        assert_eq!(campaign.household.mess, 0);
    }

    #[test]
    fn day_resets_preserve_history_and_apply_only_due_support_tools() {
        let mut campaign = Campaign::new(5);
        campaign.support.daily_cap_cents = Some(3_000);
        campaign.support.pending_cap = Some(PendingCap {
            cents: Some(5_000),
            effective_day: 2,
        });
        campaign.support.app_blocked_today = true;
        campaign.support.care_actions = 2;
        campaign.support.app_blocked_today = false;
        campaign
            .apply(Action::Bet {
                choice: BetChoice::RouletteRed,
                stake_cents: 100,
            })
            .unwrap();
        campaign.support.app_blocked_today = true;
        campaign.expire_relief();
        campaign.betting.relief = Some(TemporaryRelief {
            day: 1,
            period: Period::Morning,
            boredom: 8,
            stress: 4,
        });
        campaign.attributes.boredom -= 8;
        campaign.attributes.stress -= 4;
        let rng = campaign.rng.clone();
        campaign.apply(Action::Sleep).unwrap();
        assert_eq!(campaign.support.daily_cap_cents, Some(3_000));
        assert!(campaign.support.app_blocked_today);
        assert!(campaign.betting.session.is_none());
        assert!(campaign.betting.relief.is_none());
        campaign.apply(Action::AcknowledgeReport).unwrap();
        assert_eq!(campaign.support.daily_cap_cents, Some(5_000));
        assert!(campaign.support.pending_cap.is_none());
        assert!(!campaign.support.app_blocked_today);
        assert_eq!(campaign.support.care_actions, 2);
        assert_eq!(campaign.betting.next_session_id, 2);
        assert_eq!(campaign.rng, rng);
        assert_eq!(
            campaign.reports[0].finances.closing_cents,
            campaign.daily_finances(2).unwrap().opening_cents
        );
    }

    #[test]
    fn due_bill_stress_and_housing_deadline_wait_for_report_acknowledgement() {
        let mut campaign = Campaign::new(5);
        while campaign.clock.day < 4 {
            next_day(&mut campaign);
        }
        campaign.attributes.stress = 0;
        campaign.apply(Action::Sleep).unwrap();
        assert_eq!(campaign.attributes.stress, 14); // Rafa missed +6, electricity due +8.
        assert_eq!(campaign.reports[3].overdue_bills, [BillKind::Electricity]);
        campaign.apply(Action::AcknowledgeReport).unwrap();
        while campaign.clock.day < 8 {
            next_day(&mut campaign);
        }
        let mut extended = campaign.clone();
        extended.support.rent_negotiated = true;
        extended.bills[1].due_day = 10;
        extended.apply(Action::Sleep).unwrap();
        campaign.apply(Action::Sleep).unwrap();
        assert_eq!(campaign.clock.day, 8);
        assert_eq!(campaign.stage, CampaignStage::DailyReport);
        assert_eq!(
            campaign.reports[7].overdue_bills,
            [BillKind::Electricity, BillKind::Rent]
        );
        campaign.apply(Action::AcknowledgeReport).unwrap();
        assert_eq!(campaign.clock.day, 9);
        assert_eq!(campaign.stage, CampaignStage::Ended(Ending::HousingLoss));
        extended.apply(Action::AcknowledgeReport).unwrap();
        assert_eq!(extended.clock.day, 9);
        assert_eq!(extended.stage, CampaignStage::Playing);
    }
}
