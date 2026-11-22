//! Authoritative campaign data. Gameplay changes enter through `Campaign::apply`.
use serde::{Deserialize, Serialize};

pub const DAYS: u8 = 4;
pub const PA_PER_DAY: u8 = 6;
pub const INITIAL_BALANCE_CENTS: i64 = 6_000;
const LEGACY_DAYS: u8 = 10;
const LEGACY_BALANCE_CENTS: i64 = 12_000;
const OLD_PAY_REAIS: i64 = 80;
const WORK_DAYS: [u8; 7] = [1, 2, 3, 5, 6, 7, 8];
pub(super) const WORK_AT_NIGHT: &str = "Work shifts must start before night.";
pub(super) const TOO_HUNGRY: &str = "Too hungry. Eat something first.";
pub(super) const TOO_STRESSED: &str = "I'm too stressed. I need to calm down first.";
pub(super) const HUNGER_RESTRICTION: u8 = 80;
pub(super) const STRESS_RESTRICTION: u8 = 80;
pub(super) const SALE_PRESSURE: u8 = 70;
const BILLS: [(BillKind, i64, i64, u8); 2] = [
    (BillKind::Electricity, 12_000, 10_000, 4),
    (BillKind::Rent, 15_000, 42_000, 7),
];
const STAGGERED_BILLS: [(BillKind, i64, u8); 3] = [
    (BillKind::Electricity, 10_000, 3),
    (BillKind::Rent, 12_000, 4),
    (BillKind::Water, 10_000, 2),
];
const COMMITMENTS: [(Contact, u8); 4] = [
    (Contact::Marina, 2),
    (Contact::Rafa, 4),
    (Contact::Marina, 7),
    (Contact::Rafa, 9),
];

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

    fn validate(self, last_day: u8) -> Result<(), String> {
        if !(1..=last_day).contains(&self.day) || self.pa_spent > PA_PER_DAY {
            return Err("Campaign clock is outside the campaign calendar.".into());
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
    pub(super) fn values(&self) -> [u8; 5] {
        [
            self.hunger,
            self.boredom,
            self.stress,
            self.urge,
            self.relationships,
        ]
    }

    pub fn adjust(value: &mut u8, change: i16) {
        *value = i16::from(*value).saturating_add(change).clamp(0, 100) as u8;
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Ending {
    DogSale,
    HousingLoss,
    #[serde(rename = "DayTen")]
    FinalDay,
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
    Water,
}

impl BillKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Electricity => "Electricity",
            Self::Rent => "Rent",
            Self::Water => "Water",
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
    // Legacy snapshots omit the day limit so their checksums stay valid.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    last_day: Option<u8>,
    // Old snapshots omit this field and retain their original starting balance and pay.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    delivery_rng: Option<CampaignRng>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) landlord_intro_seen: Option<bool>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    revised_rules: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    staggered_bills: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    #[cfg(test)]
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
    TherapySession,
    FinancialPlanning,
    SetDailyCap(Option<i64>),
    BlockBetting,
    Sleep,
    AcknowledgeReport,
}

impl Campaign {
    pub fn new(seed: u64) -> Self {
        Self::with_last_day(seed, Some(DAYS), true, true)
    }

    #[cfg(test)]
    pub(crate) fn legacy(seed: u64) -> Self {
        Self::with_last_day(seed, None, false, false)
    }

    #[cfg(test)]
    pub(crate) fn previous_economy(seed: u64) -> Self {
        Self::with_last_day(seed, Some(4), false, false)
    }

    fn starting_balance(last_day: Option<u8>, variable_pay: bool, revised_rules: bool) -> i64 {
        if revised_rules {
            INITIAL_BALANCE_CENTS
        } else if variable_pay {
            10_000
        } else if last_day.is_some() {
            36_000
        } else {
            LEGACY_BALANCE_CENTS
        }
    }

    fn delivery_pay_range(&self) -> (i64, i64) {
        if self.revised_rules {
            (40, 70)
        } else if self.delivery_rng.is_some() {
            (20, 60)
        } else {
            (OLD_PAY_REAIS, OLD_PAY_REAIS)
        }
    }

    pub(super) fn uses_phone_for(&self, contact: Contact) -> bool {
        contact == Contact::Marina || self.revised_rules
    }

    fn commitment_period(contact: Contact, revised_rules: bool) -> Period {
        if contact == Contact::Rafa && revised_rules {
            Period::Night
        } else {
            Period::Afternoon
        }
    }

    pub fn last_day(&self) -> u8 {
        self.last_day.unwrap_or(LEGACY_DAYS)
    }

    fn has_work_shift(&self, day: u8) -> bool {
        day == self.last_day() || WORK_DAYS.contains(&day)
    }

    pub(super) fn is_legacy(&self) -> bool {
        self.last_day.is_none()
    }

    pub(super) fn landlord_intro_due(&self) -> bool {
        !self.character_status_visible()
            && self.clock.pa_spent > 0
            && !matches!(self.stage, CampaignStage::Ended(_))
    }

    pub(super) fn character_status_visible(&self) -> bool {
        self.clock.day != 1 || self.landlord_intro_seen != Some(false)
    }

    fn bill_terms(&self) -> impl Iterator<Item = (BillKind, i64, u8)> + '_ {
        BILLS.into_iter()
            .filter(|_| !self.staggered_bills)
            .map(|(kind, new_amount, old_amount, legacy_due)| {
                let amount = if self.revised_rules { new_amount } else { old_amount };
                let due = if kind == BillKind::Rent && self.support.rent_negotiated {
                    LEGACY_DAYS
                } else {
                    self.last_day.unwrap_or(legacy_due)
                };
                (kind, amount, due)
            })
            .chain(
                STAGGERED_BILLS.into_iter()
                    .filter(|_| self.staggered_bills)
                    .map(|(kind, amount, due)| (kind, amount, due.min(self.last_day()))),
            )
    }

    fn initial_bills(&self) -> Vec<Bill> {
        self.bill_terms()
            .map(|(kind, amount_cents, due_day)| Bill {
                kind,
                amount_cents,
                due_day,
                paid_day: None,
            })
            .collect()
    }

    pub(super) fn update_unpaid_bills(&mut self) {
        if self.staggered_bills {
            for (kind, amount, _) in STAGGERED_BILLS {
                if let Some(bill) = self
                    .bills
                    .iter_mut()
                    .find(|bill| bill.kind == kind && bill.paid_day.is_none())
                {
                    bill.amount_cents = amount;
                }
            }
        }
    }

    fn with_last_day(seed: u64, last_day: Option<u8>, variable_pay: bool, revised_rules: bool) -> Self {
        let mut campaign = Self {
            clock: Clock {
                day: 1,
                pa_spent: 0,
            },
            stage: CampaignStage::Playing,
            balance_cents: Self::starting_balance(last_day, variable_pay, revised_rules),
            attributes: Attributes {
                hunger: 25,
                boredom: 30,
                stress: if variable_pay { 30 } else { 25 },
                urge: if variable_pay { 30 } else { 10 },
                relationships: 65,
            },
            household: Household {
                food_portions: 3,
                mess: 0,
                sold: vec![],
            },
            bills: Vec::new(),
            commitments: COMMITMENTS
                .into_iter()
                .filter(|&(_, day)| day <= last_day.unwrap_or(LEGACY_DAYS))
                .map(|(contact, day)| Commitment {
                    contact,
                    day,
                    period: Self::commitment_period(contact, revised_rules),
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
            last_day,
            landlord_intro_seen: variable_pay.then_some(false),
            revised_rules,
            staggered_bills: revised_rules,
            delivery_rng: variable_pay
                .then(|| CampaignRng::new(seed ^ u64::from_be_bytes(*b"delivery"))),
        };
        campaign.bills = campaign.initial_bills();
        campaign
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
        if matches!(
            action,
            Action::Groceries | Action::Cook | Action::OrderFood | Action::NegotiateRent | Action::TherapySession
        ) {
            self.require_low_stress()?;
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
            #[cfg(test)]
            Action::Wait => next.spend_pa(1)?,
            Action::Work => {
                if next.clock.period() == Period::Night {
                    return Err(WORK_AT_NIGHT.into());
                }
                if !next.has_work_shift(next.clock.day) {
                    return Err("MeFood has no shift today.".into());
                }
                if next.worked_days.contains(&next.clock.day) {
                    return Err("Today's MeFood shift is already complete.".into());
                }
                next.require_low_stress()?;
                next.adjust_stress(4);
                next.spend_pa(2)?;
                let (min, max) = next.delivery_pay_range();
                let pay = if let Some(rng) = &mut next.delivery_rng {
                    min + rng.sample_below((max - min + 1) as u64)? as i64
                } else {
                    min
                };
                next.record_money(LedgerKind::Work, pay * 100)?;
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
                next.require_food()?;
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
                next.require_food()?;
                next.adjust_stress(-4);
                next.spend_pa(1)?;
                next.household.mess = 0;
            }
            Action::PayBill(kind) => next.pay_bill(kind)?,
            Action::NegotiateRent => next.negotiate_rent()?,
            Action::Call(contact) => next.call_contact(contact)?,
            Action::ReceiveVisit(contact) => next.receive_visit(contact)?,
            Action::CareConversation => next.care_conversation()?,
            Action::TherapySession => {
                Attributes::adjust(&mut next.attributes.urge, -30);
                next.spend_pa(4)?;
            }
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
        let grace_days = u8::from(self.is_legacy());
        self.bills.iter().any(|bill| {
            bill.kind == BillKind::Electricity
                && (bill.paid_day.is_some()
                    || self.clock.day <= bill.due_day.saturating_add(grace_days))
        })
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

    fn require_food(&self) -> Result<(), String> {
        if self.attributes.hunger >= HUNGER_RESTRICTION {
            return Err(TOO_HUNGRY.into());
        }
        Ok(())
    }

    pub(super) fn require_low_stress(&self) -> Result<(), String> {
        if self.attributes.stress >= STRESS_RESTRICTION {
            return Err(TOO_STRESSED.into());
        }
        Ok(())
    }

    fn end_period(&mut self, period: Period) {
        let mut missed = 0;
        for commitment in &mut self.commitments {
            if commitment.day == self.clock.day
                && commitment.period == period
                && commitment.status == CommitmentStatus::Scheduled
            {
                commitment.status = CommitmentStatus::Missed;
                let penalty = if self.revised_rules { -25 } else { -12 };
                Attributes::adjust(&mut self.attributes.relationships, penalty);
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
        if self.is_legacy() && self.clock.day == 5 && overdue_bills.contains(&BillKind::Electricity)
        {
            consequences.push("Electricity warning: cutoff starts on day 6 if unpaid.".into());
        }
        if self.is_legacy() && self.clock.day == 8 && overdue_bills.contains(&BillKind::Rent) {
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
        let unpaid_rent_due = self.bills.iter().any(|bill| {
            bill.kind == BillKind::Rent && bill.paid_day.is_none() && bill.due_day <= self.clock.day
        });
        if self.clock.day == self.last_day() {
            self.stage = CampaignStage::Ended(if !self.is_legacy() && unpaid_rent_due {
                Ending::HousingLoss
            } else {
                Ending::FinalDay
            });
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
        self.stage = if self.is_legacy() && self.clock.day >= 9 && unpaid_rent_due {
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
            .map_or(
                Self::starting_balance(
                    self.last_day,
                    self.delivery_rng.is_some(),
                    self.revised_rules,
                ),
                |row| row.balance_after_cents,
            );
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
        let last_day = self.last_day();
        if last_day == u8::MAX {
            return Err("Campaign calendar is too long.".into());
        }
        if self.revised_rules && (self.last_day.is_none() || self.delivery_rng.is_none()) {
            return Err("The campaign budget or delivery schedule is invalid.".into());
        }
        self.clock.validate(last_day)?;
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
        let mut balance =
            Self::starting_balance(self.last_day, self.delivery_rng.is_some(), self.revised_rules);
        let mut previous_clock = Clock {
            day: 1,
            pa_spent: 0,
        };
        for row in &self.ledger {
            row.clock.validate(last_day)?;
            validate_ledger_amount(row.kind, row.amount_cents)?;
            if let LedgerKind::BillPayment(kind) = row.kind
                && !self.bills.iter().any(|bill| bill.kind == kind)
            {
                return Err("A payment refers to a bill outside this campaign.".into());
            }
            if self.delivery_rng.is_some() && row.kind == LedgerKind::Work {
                let (min, max) = self.delivery_pay_range();
                // Payments already recorded before the cap changed must still load.
                let max = if self.revised_rules { 90 } else { max };
                if row.amount_cents % 100 != 0
                    || !(min * 100..=max * 100).contains(&row.amount_cents)
                {
                    return Err("A delivery payment is outside the valid range.".into());
                }
            }
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
        if self.bills.len() != self.bill_terms().count() || self.staggered_bills && !self.revised_rules {
            return Err("The campaign has an invalid set of bills.".into());
        }
        if !self.is_legacy() && self.support.rent_negotiated {
            return Err("The final-day rent deadline cannot be extended.".into());
        }
        for (kind, amount, due) in self.bill_terms() {
            if self
                .bills
                .iter()
                .filter(|bill| {
                    bill.kind == kind
                        && (bill.amount_cents == amount
                            || self.staggered_bills
                                && match kind {
                                    BillKind::Electricity => bill.amount_cents == 9_000,
                                    BillKind::Rent => matches!(bill.amount_cents, 11_000 | 13_000),
                                    BillKind::Water => bill.amount_cents == 6_000,
                                })
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
        let commitments = COMMITMENTS.into_iter().filter(|&(_, day)| day <= last_day);
        if self.commitments.len() != commitments.clone().count()
            || commitments.into_iter().any(|(contact, day)| {
                self.commitments
                    .iter()
                    .filter(|commitment| {
                        commitment.contact == contact
                            && commitment.day == day
                            && commitment.period == Self::commitment_period(contact, self.revised_rules)
                    })
                    .count()
                    != 1
            })
        {
            return Err("The social calendar is invalid.".into());
        }
        if self.worked_days.iter().enumerate().any(|(i, day)| {
            !self.has_work_shift(*day) || *day > self.clock.day || self.worked_days[..i].contains(day)
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
        c.attributes.stress = 70;
        c.apply(Action::Sell(HouseholdItem::Tv)).unwrap();
        c.apply(Action::CareConversation).unwrap();
        c.apply(Action::FinancialPlanning).unwrap();
        c.apply(Action::SetDailyCap(Some(2_000))).unwrap();
        c.apply(Action::BlockBetting).unwrap();
        c.apply(Action::Sleep).unwrap();
        let original = c.reports[0].clone();
        let facts = original.consequences.join("\n");
        assert!(facts.contains("Electricity: unpaid, due end of day 3"));
        assert!(facts.contains("Permanently sold today: TV"));
        assert!(facts.contains("Care conversations so far: 1"));
        assert!(facts.contains("R$20.00. Betting app: blocked until morning"));
        assert_eq!(original.pa_used, 3);
        c.apply(Action::AcknowledgeReport).unwrap();
        c.apply(Action::PayBill(BillKind::Electricity)).unwrap();
        c.apply(Action::CareConversation).unwrap();
        c.apply(Action::SetDailyCap(Some(5_000))).unwrap();
        c.attributes.stress = 70;
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
        assert_eq!(campaign.balance_cents, INITIAL_BALANCE_CENTS);
        assert_eq!(campaign.household.food_portions, 3);
        assert_eq!(
            campaign.attributes,
            Attributes {
                hunger: 25,
                boredom: 30,
                stress: 30,
                urge: 30,
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
    fn staggered_deadlines_survive_a_longer_campaign_and_reload() {
        use crate::game::campaign_save::{decode_campaign, encode_campaign};
        let mut campaign = Campaign::with_last_day(5, Some(6), true, true);
        while campaign.clock.day < campaign.last_day() {
            next_day(&mut campaign);
        }
        let snapshot = encode_campaign(&campaign).unwrap();
        let mut restored = decode_campaign(&snapshot).unwrap();
        assert_eq!(restored.last_day(), 6);
        assert_eq!(
            restored.bills.iter().map(|bill| bill.due_day).collect::<Vec<_>>(),
            [3, 4, 2]
        );
        assert!(!restored.has_power());
        assert!(restored.apply(Action::NegotiateRent).is_err());
        assert_eq!(restored, campaign);
        next_day(&mut restored);
        assert_eq!(restored.clock.day, 6);
        assert_eq!(restored.stage, CampaignStage::Ended(Ending::HousingLoss));
    }

    #[test]
    fn invalid_and_finished_campaigns_reject_without_mutation() {
        for stage in [
            CampaignStage::DailyReport,
            CampaignStage::Ended(Ending::FinalDay),
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
        assert_eq!(campaign.attributes.stress, 30);
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
            (LedgerKind::Work, 7_000),
            (LedgerKind::Food, -4_000),
            (LedgerKind::BetStake, -100),
            (LedgerKind::BetPayout, 180),
            (LedgerKind::Sale(HouseholdItem::Books), 2_000),
            (LedgerKind::BillPayment(BillKind::Electricity), -10_000),
        ] {
            campaign.record_money(kind, amount).unwrap();
        }
        campaign.bills[0].paid_day = Some(1);
        campaign.household.sold.push(HouseholdItem::Books);
        campaign.validate().unwrap();
        assert_eq!(
            campaign.daily_finances(1).unwrap(),
            DailyFinances {
                opening_cents: INITIAL_BALANCE_CENTS,
                work_cents: 7_000,
                food_cents: 4_000,
                gross_bets_cents: 100,
                payouts_cents: 180,
                bills_cents: 10_000,
                sales_cents: 2_000,
                closing_cents: INITIAL_BALANCE_CENTS - 4_920
            }
        );
        campaign.clock.day = 2;
        assert_eq!(
            campaign.daily_finances(2).unwrap().opening_cents,
            campaign.balance_cents
        );
        let before = campaign.clone();
        assert!(
            campaign
                .record_money(LedgerKind::Food, -before.balance_cents - 1)
                .is_err()
        );
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
    fn delivery_pay_varies_is_saved_and_does_not_advance_betting_rng() {
        use crate::game::campaign_save::{decode_campaign, encode_campaign};
        let mut campaign = Campaign::new(5);
        let betting_rng = campaign.rng.clone();
        let mut payments = Vec::new();
        for _ in 0..3 {
            let mut restored = decode_campaign(&encode_campaign(&campaign).unwrap()).unwrap();
            campaign.apply(Action::Work).unwrap();
            restored.apply(Action::Work).unwrap();
            assert_eq!(campaign, restored);
            assert_eq!(campaign.rng, betting_rng);
            let pay = campaign.ledger.last().unwrap().amount_cents;
            assert!((4_000..=7_000).contains(&pay));
            assert_eq!(pay % 100, 0);
            payments.push(pay);
            assert!(campaign.apply(Action::Work).is_err());
            assert_eq!(campaign, restored);
            next_day(&mut campaign);
        }
        assert!(payments.windows(2).any(|pair| pair[0] != pair[1]));
        assert_eq!(
            campaign.balance_cents,
            INITIAL_BALANCE_CENTS + payments.into_iter().sum::<i64>()
        );

        let mut invalid = Campaign::new(5);
        invalid.record_money(LedgerKind::Work, 9_100).unwrap();
        assert!(invalid.validate().is_err());
    }

    #[test]
    fn earlier_variable_pay_snapshots_keep_their_balance_bills_and_visits() {
        use crate::game::campaign_save::{decode_campaign, encode_campaign};

        let mut old = Campaign::with_last_day(5, Some(4), true, false);
        assert_eq!(old.balance_cents, 10_000);
        assert_eq!(old.bills[1].amount_cents, 42_000);
        assert_eq!(old.commitments[1].period, Period::Afternoon);
        old.apply(Action::PayBill(BillKind::Electricity)).unwrap();
        old.apply(Action::Work).unwrap();
        let snapshot = encode_campaign(&old).unwrap();
        assert!(!snapshot.contains("revised_rules"));
        assert_eq!(decode_campaign(&snapshot).unwrap(), old);
        assert!((2_000..=6_000).contains(&old.balance_cents));
    }

    #[test]
    fn two_bill_campaigns_from_before_water_keep_their_original_terms() {
        use crate::game::campaign_save::{decode_campaign, encode_campaign};

        let mut old = Campaign::new(5);
        old.staggered_bills = false;
        old.bills = old.initial_bills();
        let snapshot = encode_campaign(&old).unwrap();
        assert!(!snapshot.contains("staggered_bills"));
        let mut restored = decode_campaign(&snapshot).unwrap();
        assert_eq!(restored, old);
        assert_eq!(restored.bills[0].amount_cents, 12_000);
        assert_eq!(restored.bills[1].amount_cents, 15_000);
        assert!(restored.bills.iter().all(|bill| bill.due_day == 4));
        assert!(restored.apply(Action::PayBill(BillKind::Water)).is_err());
        assert_eq!(restored, old);
        restored.record_money(LedgerKind::BillPayment(BillKind::Water), -100).unwrap();
        assert!(restored.validate().is_err());
    }

    #[test]
    fn routine_budget_costs_and_legacy_shifts_include_the_last_day() {
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
        assert_eq!(campaign.household.food_portions, 5);
        assert_eq!(campaign.household.mess, 0);
        assert_eq!(campaign.attributes.hunger, 28);
        assert_eq!(campaign.attributes.boredom, 28);
        assert_eq!(campaign.attributes.stress, 29);
        let finances = campaign.daily_finances(1).unwrap();
        assert!((4_000..=7_000).contains(&finances.work_cents));
        assert_eq!(finances.food_cents, 4_000);
        assert_eq!(
            finances.closing_cents,
            finances.opening_cents + finances.work_cents - finances.food_cents
        );

        let mut campaign = Campaign::legacy(8);
        campaign.support.rent_negotiated = true;
        campaign.bills[1].due_day = 10;
        for day in 1..=campaign.last_day() {
            campaign.attributes.stress = campaign.attributes.stress.min(60);
            if campaign.has_work_shift(day) {
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
        assert_eq!(campaign.worked_days, [1, 2, 3, 5, 6, 7, 8, 10]);
        assert_eq!(campaign.balance_cents, 76_000);
        assert_eq!(campaign.reports.len(), 10);
        assert_eq!(campaign.clock.day, 10);
        assert_eq!(campaign.stage, CampaignStage::Ended(Ending::FinalDay));
        let before = campaign.clone();
        assert!(campaign.apply(Action::AcknowledgeReport).is_err());
        assert!(campaign.apply(Action::Sleep).is_err());
        assert_eq!(campaign, before);
    }

    #[test]
    fn last_day_shift_is_payable_before_night_and_survives_reloading() {
        use crate::game::campaign_save::{decode_campaign, encode_campaign};

        for mut campaign in [Campaign::new(5), Campaign::with_last_day(5, Some(9), true, true)] {
            let day = campaign.last_day();
            while campaign.clock.day < day {
                next_day(&mut campaign);
            }
            campaign.attributes.stress = campaign.attributes.stress.min(60);
            let mut night = campaign.clone();
            for _ in 0..4 {
                night.apply(Action::Wait).unwrap();
            }
            assert_eq!(night.apply(Action::Work).unwrap_err(), WORK_AT_NIGHT);

            campaign.apply(Action::Work).unwrap();
            assert_eq!(campaign.worked_days, [day]);
            assert_eq!(campaign.clock.pa_spent, 2);
            assert!((4_000..=7_000).contains(&(campaign.balance_cents - INITIAL_BALANCE_CENTS)));
            let restored = decode_campaign(&encode_campaign(&campaign).unwrap()).unwrap();
            assert_eq!(restored, campaign);
            assert!(campaign.apply(Action::Work).unwrap_err().contains("already complete"));
        }
    }

    #[test]
    fn failed_routines_leave_money_time_inventory_and_stats_untouched() {
        let mut no_money = Campaign::new(5);
        no_money
            .record_money(LedgerKind::Food, -INITIAL_BALANCE_CENTS)
            .unwrap();
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
        assert_eq!(campaign.apply(Action::Work).unwrap_err(), WORK_AT_NIGHT);
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
            assert_eq!(
                campaign.balance_cents,
                INITIAL_BALANCE_CENTS - 2_000 + item.sale_cents()
            );
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
        let mut campaign = Campaign::legacy(5);
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
        campaign.apply(Action::OrderFood).unwrap();
        campaign.apply(Action::WatchTv).unwrap();
    }

    #[test]
    fn severe_hunger_blocks_household_tasks_but_allows_eating_and_betting() {
        for action in [Action::WatchTv, Action::Clean] {
            let mut campaign = Campaign::new(5);
            campaign.attributes.hunger = 79;
            campaign.clone().apply(action).unwrap();
            campaign.attributes.hunger = 80;
            let before = campaign.clone();
            assert_eq!(campaign.apply(action).unwrap_err(), TOO_HUNGRY);
            assert_eq!(campaign, before);
            campaign.apply(Action::Cook).unwrap();
            campaign.apply(action).unwrap();
        }
        let mut campaign = Campaign::new(5);
        campaign.attributes.hunger = 100;
        campaign
            .apply(Action::Bet {
                choice: BetChoice::Slots,
                stake_cents: 100,
            })
            .unwrap();
        assert_eq!(campaign.betting.history.len(), 1);
    }

    #[test]
    fn critical_stress_blocks_work_and_non_relief_activities() {
        let mut campaign = Campaign::new(5);
        campaign.record_money(LedgerKind::Work, 4_000).unwrap();
        campaign.attributes.stress = STRESS_RESTRICTION - 1;
        campaign.clone().apply(Action::Work).unwrap();
        campaign.attributes.stress = STRESS_RESTRICTION;
        let before = campaign.clone();
        for action in [
            Action::Work,
            Action::Cook,
            Action::OrderFood,
            Action::Groceries,
            Action::NegotiateRent,
            Action::TherapySession,
            Action::Sell(HouseholdItem::Tv),
        ] {
            assert_eq!(campaign.apply(action).unwrap_err(), TOO_STRESSED);
            assert_eq!(campaign, before);
        }
        for action in [
            Action::Clean,
            Action::WatchTv,
            Action::TimeWithPipoca,
            Action::Call(Contact::Marina),
            Action::CareConversation,
            Action::FinancialPlanning,
        ] {
            let mut recovering = campaign.clone();
            recovering.apply(action).unwrap();
            assert!(recovering.attributes.stress < STRESS_RESTRICTION);
            recovering.apply(Action::Work).unwrap();
        }

        campaign.clone().apply(Action::PayBill(BillKind::Water)).unwrap();
        campaign.clone().apply(Action::BlockBetting).unwrap();
        campaign.apply(Action::SetDailyCap(Some(100))).unwrap();
        campaign.apply(Action::Bet { choice: BetChoice::Slots, stake_cents: 100 }).unwrap();
        assert_eq!(campaign.betting.history.len(), 1);
        next_day(&mut campaign);
        assert_eq!(campaign.clock.day, 2);
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
        assert_eq!(campaign.attributes.relationships, 40);
        assert_eq!(campaign.commitments[0].status, CommitmentStatus::Missed);
        assert_eq!(campaign.reports[1].missed_commitments, [Contact::Marina]);
        assert_eq!(campaign.reports[1].pa_used, 0);
        assert_eq!(campaign.reports[1].relationships_opening, 65);
        assert_eq!(campaign.reports[1].relationships_closing, 40);
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
        assert_eq!(campaign.day_opening_relationships, 40);

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
        let mut attributes = campaign.attributes.clone();
        attributes.stress += 10; // The water bill reaches its deadline on day 2.
        campaign.apply(Action::Sleep).unwrap();
        assert_eq!(campaign.attributes, attributes);
        assert_eq!(campaign.reports[1].pa_used, 6);
        assert_eq!(campaign.reports[1].missed_commitments, [Contact::Marina]);
        assert_eq!(campaign.reports[1].overdue_bills, [BillKind::Water]);
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
    fn apartment_mess_increases_stress_gained_but_not_stress_relief() {
        for mess in 0..=3 {
            let mut campaign = Campaign::new(5);
            campaign.household.mess = mess;
            let before = campaign.attributes.stress;
            campaign.apply(Action::Work).unwrap();
            assert_eq!(campaign.attributes.stress, before + 6 + mess);
            campaign.apply(Action::Clean).unwrap();
            assert_eq!(campaign.attributes.stress, before + 2 + mess);
        }
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
        let mut campaign = Campaign::legacy(5);
        while campaign.clock.day < 4 {
            next_day(&mut campaign);
        }
        campaign.attributes.stress = 0;
        campaign.apply(Action::Sleep).unwrap();
        assert_eq!(campaign.attributes.stress, 18); // Rafa missed +8, electricity due +10.
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
