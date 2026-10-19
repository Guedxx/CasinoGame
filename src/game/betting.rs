//! Exact fictional odds and atomic betting transitions; animation never enters this module.
use super::campaign::*;

impl BetChoice {
    pub fn outcome(self, bucket: u8) -> BetOutcome {
        assert!(bucket < 100);
        match self {
            Self::RouletteRed | Self::RouletteBlack => {
                if bucket < 50 {
                    BetOutcome::RouletteRed
                } else {
                    BetOutcome::RouletteBlack
                }
            }
            Self::Slots => {
                if bucket < 30 {
                    BetOutcome::SlotsWin
                } else {
                    BetOutcome::SlotsLoss
                }
            }
            _ => match bucket {
                0..=49 => BetOutcome::FootballHome,
                50..=74 => BetOutcome::FootballDraw,
                _ => BetOutcome::FootballAway,
            },
        }
    }

    pub fn total_return(self, outcome: BetOutcome, stake: i64) -> Result<i64, String> {
        let tenths = match (self, outcome) {
            (Self::RouletteRed, BetOutcome::RouletteRed)
            | (Self::RouletteBlack, BetOutcome::RouletteBlack)
            | (Self::FootballHome, BetOutcome::FootballHome) => 18,
            (Self::Slots, BetOutcome::SlotsWin) => 30,
            (Self::FootballDraw, BetOutcome::FootballDraw)
            | (Self::FootballAway, BetOutcome::FootballAway) => 36,
            _ => 0,
        };
        (stake / 10)
            .checked_mul(tenths)
            .ok_or_else(|| "The stake is too large.".into())
    }

    pub fn accepts(self, outcome: BetOutcome) -> bool {
        (0..100).any(|bucket| self.outcome(bucket) == outcome)
    }
}

// Track the gap from the clamped value without relief, including subsequent healthy reductions.
fn adjust_with_relief(value: &mut u8, gap: &mut u8, change: i16) {
    let baseline = (i16::from(*value) + i16::from(*gap))
        .saturating_add(change)
        .clamp(0, 100);
    Attributes::adjust(value, change);
    *gap = (baseline - i16::from(*value)) as u8;
}

impl Campaign {
    /// All permanent stress/boredom effects must use these helpers while relief can be active.
    pub(super) fn adjust_stress(&mut self, change: i16) {
        if let Some(relief) = &mut self.betting.relief {
            adjust_with_relief(&mut self.attributes.stress, &mut relief.stress, change);
        } else {
            Attributes::adjust(&mut self.attributes.stress, change);
        }
    }

    pub(super) fn adjust_boredom(&mut self, change: i16) {
        if let Some(relief) = &mut self.betting.relief {
            adjust_with_relief(&mut self.attributes.boredom, &mut relief.boredom, change);
        } else {
            Attributes::adjust(&mut self.attributes.boredom, change);
        }
    }

    fn grant_relief(&mut self, boredom: u8, stress: u8) {
        let relief = self.betting.relief.get_or_insert(TemporaryRelief {
            day: self.clock.day,
            period: self.clock.period(),
            boredom: 0,
            stress: 0,
        });
        let boredom = boredom.min(self.attributes.boredom);
        let stress = stress.min(self.attributes.stress);
        self.attributes.boredom -= boredom;
        self.attributes.stress -= stress;
        relief.boredom += boredom;
        relief.stress += stress;
    }

    pub(super) fn settle_bet(&mut self, choice: BetChoice, stake_cents: i64) -> Result<(), String> {
        if stake_cents < 100 || stake_cents % 100 != 0 {
            return Err("Enter a whole-real stake of at least R$1.".into());
        }
        if stake_cents > self.balance_cents {
            return Err("The stake exceeds your available balance. Lower the stake.".into());
        }
        // Validate even the largest possible return before drawing an outcome.
        choice.total_return(choice.outcome(0), stake_cents)?;
        choice.total_return(choice.outcome(99), stake_cents)?;
        choice.total_return(choice.outcome(50), stake_cents)?;
        if self.support.app_blocked_today {
            return Err(
                "Betting is blocked until tomorrow morning. Support stays available.".into(),
            );
        }
        if let Some(cap) = self.support.daily_cap_cents {
            let gross = self.daily_finances(self.clock.day)?.gross_bets_cents;
            if gross
                .checked_add(stake_cents)
                .is_none_or(|amount| amount > cap)
            {
                return Err("This stake exceeds your remaining daily wagering cap.".into());
            }
        }
        let is_new = self.betting.session.is_none();
        if is_new {
            let id = self.betting.next_session_id;
            self.betting.next_session_id = id
                .checked_add(1)
                .ok_or("No more betting sessions are available.")?;
            self.betting.session = Some(BetSession {
                id,
                rounds: 0,
                paid_rounds_remaining: 0,
                last_stake_cents: 0,
                last_was_loss: false,
            });
        }
        let session = self.betting.session.as_ref().unwrap();
        let round_number = session
            .rounds
            .checked_add(1)
            .ok_or("End this session before another round.")?;
        let chasing = session.last_was_loss && stake_cents > session.last_stake_cents;
        let pa_consumed = u8::from(session.paid_rounds_remaining == 0);
        if pa_consumed == 1 {
            self.spend_pa(1)?;
            if !is_new {
                Attributes::adjust(&mut self.attributes.hunger, 6);
            }
        }
        if self
            .betting
            .relief
            .as_ref()
            .is_some_and(|r| (r.day, r.period) != (self.clock.day, self.clock.period()))
        {
            self.expire_relief();
        }
        if is_new
            && self.betting.last_session_relief_period
                != Some((self.clock.day, self.clock.period()))
        {
            self.grant_relief(8, 0);
            self.betting.last_session_relief_period = Some((self.clock.day, self.clock.period()));
        }
        let outcome = choice.outcome(self.rng.sample_below(100)? as u8);
        let total_return_cents = choice.total_return(outcome, stake_cents)?;
        self.record_money(LedgerKind::BetStake, -stake_cents)?;
        if total_return_cents > 0 {
            self.record_money(LedgerKind::BetPayout, total_return_cents)?;
        }
        if chasing {
            Attributes::adjust(&mut self.attributes.urge, 8);
        }
        if total_return_cents > 0 {
            Attributes::adjust(&mut self.attributes.urge, 7);
            self.grant_relief(0, 4);
        } else {
            self.adjust_stress(8);
        }
        let session = self.betting.session.as_mut().unwrap();
        session.rounds = round_number;
        session.paid_rounds_remaining = if pa_consumed == 1 {
            2
        } else {
            session.paid_rounds_remaining - 1
        };
        session.last_stake_cents = stake_cents;
        session.last_was_loss = total_return_cents == 0;
        self.betting.history.push(BetRound {
            choice,
            outcome,
            stake_cents,
            total_return_cents,
            net_cents: total_return_cents - stake_cents,
            session_id: session.id,
            session_round: round_number,
            clock: self.clock,
            pa_consumed,
        });
        Ok(())
    }

    pub(super) fn validate_betting(&self) -> Result<(), String> {
        let invalid =
            || "Betting history, session or financial entries do not reconcile.".to_string();
        let mut ledger = self
            .ledger
            .iter()
            .filter(|row| matches!(row.kind, LedgerKind::BetStake | LedgerKind::BetPayout));
        let mut previous: Option<&BetRound> = None;
        for round in &self.betting.history {
            let new_session = previous.is_none_or(|last| last.session_id != round.session_id);
            let expected_id = previous
                .map_or(Some(1), |last| {
                    last.session_id.checked_add(u32::from(new_session))
                })
                .ok_or_else(invalid)?;
            let expected_round = if new_session {
                1
            } else {
                previous
                    .unwrap()
                    .session_round
                    .checked_add(1)
                    .ok_or_else(invalid)?
            };
            let expected_pa = u8::from(expected_round % 3 == 1);
            if round.clock.day == 0
                || round.clock.day > self.clock.day
                || round.clock.pa_spent > PA_PER_DAY
                || round.clock > self.clock
                || round.clock.pa_spent == 0
                || round.session_id != expected_id
                || round.session_round != expected_round
                || round.pa_consumed != expected_pa
                || round.stake_cents < 100
                || round.stake_cents % 100 != 0
                || !round.choice.accepts(round.outcome)
                || round
                    .choice
                    .total_return(round.outcome, round.stake_cents)?
                    != round.total_return_cents
                || round.total_return_cents.checked_sub(round.stake_cents) != Some(round.net_cents)
            {
                return Err(invalid());
            }
            if let Some(last) = previous {
                if round.clock < last.clock
                    || (!new_session
                        && (round.clock.day != last.clock.day
                            || round.clock.pa_spent != last.clock.pa_spent + expected_pa))
                    || (new_session
                        && round.clock.day == last.clock.day
                        && round.clock.pa_spent <= last.clock.pa_spent)
                {
                    return Err(invalid());
                }
            }
            for (kind, amount) in [
                (LedgerKind::BetStake, -round.stake_cents),
                (LedgerKind::BetPayout, round.total_return_cents),
            ] {
                if amount != 0
                    && ledger.next().is_none_or(|row| {
                        row.clock != round.clock || row.kind != kind || row.amount_cents != amount
                    })
                {
                    return Err(invalid());
                }
            }
            previous = Some(round);
        }
        if ledger.next().is_some()
            || self.betting.next_session_id
                != previous
                    .map_or(Some(1), |last| last.session_id.checked_add(1))
                    .ok_or_else(invalid)?
        {
            return Err(invalid());
        }
        if let Some(session) = &self.betting.session {
            let last = previous.ok_or_else(invalid)?;
            if self.stage != CampaignStage::Playing
                || last.clock != self.clock
                || session.id != last.session_id
                || session.rounds != last.session_round
                || session.paid_rounds_remaining != (3 - session.rounds % 3) as u8 % 3
                || session.last_stake_cents != last.stake_cents
                || session.last_was_loss != (last.total_return_cents == 0)
            {
                return Err(invalid());
            }
        }
        if self.betting.relief.as_ref().is_some_and(|r| {
            r.day != self.clock.day
                || r.period != self.clock.period()
                || u16::from(r.boredom) + u16::from(self.attributes.boredom) > 100
                || u16::from(r.stress) + u16::from(self.attributes.stress) > 100
        }) || self
            .betting
            .last_session_relief_period
            .is_some_and(|(day, period)| {
                day == 0 || (day, period) > (self.clock.day, self.clock.period())
            })
        {
            return Err("Temporary betting relief has an invalid period or baseline.".into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::super::campaign_save::{decode_campaign, encode_campaign};
    use super::*;

    fn bet(c: &mut Campaign, choice: BetChoice, stake_cents: i64) {
        c.apply(Action::Bet {
            choice,
            stake_cents,
        })
        .unwrap();
    }
    fn force_next(c: &mut Campaign, choice: BetChoice, win: bool) {
        let seed = (0..1000)
            .find(|seed| {
                let mut rng = CampaignRng::new(*seed);
                let outcome = choice.outcome(rng.sample_below(100).unwrap() as u8);
                (choice.total_return(outcome, 100).unwrap() > 0) == win
            })
            .unwrap();
        c.rng = CampaignRng::new(seed);
    }

    #[test]
    fn cumulative_daily_overflow_rejects_atomically_and_keeps_the_day_payable_and_reportable() {
        use super::super::campaign_save::{CampaignStore, SnapshotState};
        let mut c = Campaign::new(5);
        for index in 0..31 {
            if c.clock.remaining_pa() == 0
                && c.betting.session.as_ref().unwrap().paid_rounds_remaining == 0
            {
                c.apply(Action::Sleep).unwrap();
                c.apply(Action::AcknowledgeReport).unwrap();
            }
            let mut forecast = c.rng.clone();
            let bucket = forecast.sample_below(100).unwrap();
            let (choice, factor) = if c.balance_cents > 4_000_000_000_000_000_000 {
                (
                    if bucket < 50 {
                        BetChoice::RouletteBlack
                    } else {
                        BetChoice::RouletteRed
                    },
                    0_i64,
                )
            } else if bucket < 30 {
                (BetChoice::Slots, 30)
            } else if bucket < 50 {
                (BetChoice::FootballHome, 18)
            } else if bucket < 75 {
                (BetChoice::FootballDraw, 36)
            } else {
                (BetChoice::FootballAway, 36)
            };
            let max_return = if factor == 0 { 18 } else { factor };
            let balance_room = if factor == 0 {
                2_000_000_000_000_000_000
            } else {
                (i128::from(i64::MAX - c.balance_cents) * 10 / i128::from(factor - 10))
                    .min(i128::from(i64::MAX)) as i64
            };
            let stake_cents = c
                .balance_cents
                .min(i64::MAX / max_return * 10)
                .min(balance_room)
                / 100
                * 100;
            let action = Action::Bet {
                choice,
                stake_cents,
            };
            if index < 30 {
                c.apply(action).unwrap();
                c.daily_finances(c.clock.day).unwrap();
                continue;
            }
            assert_eq!(
                c.clock,
                Clock {
                    day: 2,
                    pa_spent: 4
                }
            );
            let before = c.clone();
            let snapshot = encode_campaign(&c).unwrap();
            let error = c.apply(action).unwrap_err();
            assert!(error.contains("Reduce the stake or finish the day"));
            assert_eq!(c, before);
            assert_eq!(encode_campaign(&c).unwrap(), snapshot);
            assert_eq!(decode_campaign(&snapshot).unwrap(), c);

            let mut store = CampaignStore::for_test(Some(c.clone()), SnapshotState::Protected);
            assert!(store.apply(action).is_err());
            assert_eq!(store.campaign(), Some(&before));
            assert_eq!(store.snapshot_state(), SnapshotState::Protected);
            assert!(store.notice.is_empty());

            // The provisional state also models unsupported snapshots from before this guard.
            let mut unsupported = before.clone();
            unsupported.settle_bet(choice, stake_cents).unwrap();
            assert!(unsupported.daily_finances(2).is_err());
            assert!(unsupported.validate().is_err());
            assert!(encode_campaign(&unsupported).is_err());
            let payload = serde_json::to_string(&unsupported).unwrap();
            let checksum = payload.bytes().fold(0xcbf29ce484222325_u64, |hash, byte| {
                (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3)
            });
            let old_snapshot =
                format!(r#"{{"version":1,"checksum":{checksum},"campaign":{payload}}}"#);
            assert!(
                decode_campaign(&old_snapshot)
                    .unwrap_err()
                    .contains("Reduce the stake")
            );

            let mut reduced = before;
            reduced
                .apply(Action::Bet {
                    choice,
                    stake_cents: 100,
                })
                .unwrap();
            reduced.apply(Action::Sleep).unwrap();
            let balance = c.balance_cents;
            c.apply(Action::PayBill(BillKind::Electricity)).unwrap();
            c.apply(Action::PayBill(BillKind::Rent)).unwrap();
            assert_eq!(c.balance_cents, balance - 52_000);
            c.apply(Action::Sleep).unwrap();
            assert_eq!(c.stage, CampaignStage::DailyReport);
            assert_eq!(
                c.reports.last().unwrap().finances,
                c.daily_finances(2).unwrap()
            );
            assert_eq!(decode_campaign(&encode_campaign(&c).unwrap()).unwrap(), c);
        }
    }

    #[test]
    fn every_hundred_bucket_partition_has_exact_payout_and_ninety_percent_return() {
        for (choice, expected_wins, winning_return) in [
            (BetChoice::RouletteRed, 50, 180),
            (BetChoice::RouletteBlack, 50, 180),
            (BetChoice::Slots, 30, 300),
            (BetChoice::FootballHome, 50, 180),
            (BetChoice::FootballDraw, 25, 360),
            (BetChoice::FootballAway, 25, 360),
        ] {
            let returns: Vec<_> = (0..100)
                .map(|bucket| choice.total_return(choice.outcome(bucket), 100).unwrap())
                .collect();
            assert_eq!(returns.iter().filter(|r| **r > 0).count(), expected_wins);
            assert!(returns.iter().all(|r| *r == 0 || *r == winning_return));
            assert_eq!(returns.iter().sum::<i64>(), 9000);
            assert_eq!(
                choice.total_return(choice.outcome(0), 12_300).unwrap() % 10,
                0
            );
        }
        for bucket in 0..100 {
            let outcome = BetChoice::FootballHome.outcome(bucket);
            assert_eq!(outcome, BetChoice::FootballDraw.outcome(bucket));
            assert_eq!(outcome, BetChoice::FootballAway.outcome(bucket));
        }
    }

    #[test]
    fn invalid_stakes_caps_blocks_and_empty_pa_are_atomic_including_rng() {
        for stake in [i64::MIN, -100, 0, 1, 99, 101, 12_100, i64::MAX] {
            let mut c = Campaign::new(42);
            let before = c.clone();
            assert!(
                c.apply(Action::Bet {
                    choice: BetChoice::Slots,
                    stake_cents: stake
                })
                .is_err()
            );
            assert_eq!(c, before);
        }
        let mut c = Campaign::new(42);
        c.support.daily_cap_cents = Some(200);
        bet(&mut c, BetChoice::Slots, 100);
        c.apply(Action::EndBetSession).unwrap();
        bet(&mut c, BetChoice::RouletteRed, 100);
        for guard in 0..3 {
            let mut blocked = c.clone();
            if guard == 1 {
                blocked.support.app_blocked_today = true;
            }
            if guard == 2 {
                blocked.support.daily_cap_cents = Some(50);
            }
            let before = blocked.clone();
            assert!(
                blocked
                    .apply(Action::Bet {
                        choice: BetChoice::FootballDraw,
                        stake_cents: 100
                    })
                    .is_err()
            );
            assert_eq!(blocked, before);
        }
        let mut c = Campaign::new(42);
        for _ in 0..6 {
            c.apply(Action::Wait).unwrap();
        }
        let before = c.clone();
        assert!(
            c.apply(Action::Bet {
                choice: BetChoice::Slots,
                stake_cents: 100
            })
            .is_err()
        );
        assert_eq!(c, before);
    }

    #[test]
    fn shared_modes_buy_rounds_one_four_seven_and_allow_last_paid_round_at_zero_pa() {
        let mut c = Campaign::new(42);
        for _ in 0..3 {
            c.apply(Action::Wait).unwrap();
        }
        let hunger = c.attributes.hunger;
        for (i, choice) in [
            BetChoice::RouletteRed,
            BetChoice::Slots,
            BetChoice::FootballDraw,
        ]
        .into_iter()
        .cycle()
        .take(9)
        .enumerate()
        {
            bet(&mut c, choice, 100);
            let round = c.betting.history.last().unwrap();
            assert_eq!(round.session_id, 1);
            assert_eq!(round.session_round as usize, i + 1);
            assert_eq!(round.pa_consumed, u8::from(i % 3 == 0));
            assert_eq!(c.clock.pa_spent, 4 + (i / 3) as u8);
        }
        assert_eq!(c.clock.remaining_pa(), 0);
        assert_eq!(c.attributes.hunger, (hunger + 3 * 8 + 2 * 6).min(100));
        let before = c.clone();
        assert!(
            c.apply(Action::Bet {
                choice: BetChoice::Slots,
                stake_cents: 100
            })
            .is_err()
        );
        assert_eq!(c, before);
        assert_eq!(c.daily_finances(1).unwrap().gross_bets_cents, 900);
    }

    #[test]
    fn exits_forfeit_paid_rounds_nonbet_activities_close_and_reopen_has_new_pa_cost() {
        let mut c = Campaign::new(9);
        bet(&mut c, BetChoice::Slots, 100);
        let settled = c.betting.history[0].clone();
        let rng = c.rng.clone();
        c.apply(Action::EndBetSession).unwrap();
        c.apply(Action::EndBetSession).unwrap();
        assert_eq!(c.betting.history, [settled]);
        assert_eq!(c.rng, rng);
        bet(&mut c, BetChoice::Slots, 100);
        assert_eq!(c.betting.history[1].session_id, 2);
        assert_eq!(c.betting.history[1].pa_consumed, 1);
        c.apply(Action::Clean).unwrap();
        assert!(c.betting.session.is_none());
        bet(&mut c, BetChoice::Slots, 100);
        assert_eq!(c.betting.history[2].session_id, 3);
        assert_eq!(c.clock.pa_spent, 4);
    }

    #[test]
    fn saved_settlements_restore_same_outcome_rng_history_and_paid_rounds() {
        let mut first = Campaign::new(71);
        bet(&mut first, BetChoice::RouletteBlack, 1200);
        let mut second = decode_campaign(&encode_campaign(&first).unwrap()).unwrap();
        assert_eq!(first, second);
        for choice in [
            BetChoice::FootballAway,
            BetChoice::Slots,
            BetChoice::RouletteRed,
        ] {
            bet(&mut first, choice, 100);
            bet(&mut second, choice, 100);
            assert_eq!(first, second);
        }
        let f = first.daily_finances(1).unwrap();
        assert_eq!(
            f.closing_cents,
            f.opening_cents - f.gross_bets_cents + f.payouts_cents
        );
        assert_eq!(
            f.gross_bets_cents,
            first
                .betting
                .history
                .iter()
                .map(|r| r.stake_cents)
                .sum::<i64>()
        );
    }

    #[test]
    fn restored_history_rejects_mismatched_choices_returns_sessions_rounds_pa_and_ledger() {
        let mut c = Campaign::new(42);
        bet(&mut c, BetChoice::RouletteRed, 100);
        bet(&mut c, BetChoice::Slots, 100);
        for variant in 0..9 {
            let mut bad = c.clone();
            match variant {
                0 => bad.betting.history[0].outcome = BetOutcome::SlotsWin,
                1 => bad.betting.history[0].total_return_cents += 1,
                2 => bad.betting.history[1].session_id = 2,
                3 => bad.betting.history[1].session_round = 3,
                4 => bad.betting.history[1].pa_consumed = 1,
                5 => bad.betting.session.as_mut().unwrap().paid_rounds_remaining = 2,
                6 => bad.betting.history.clear(),
                7 => bad.betting.next_session_id = u32::MAX,
                _ => bad.betting.history[1].clock.pa_spent = 2,
            }
            assert!(bad.validate().is_err(), "variant {variant}");
        }
        c.apply(Action::EndBetSession).unwrap();
        c.validate().unwrap();
    }

    #[test]
    fn chasing_uses_immediately_previous_loss_stake_in_same_session_independent_of_result() {
        for next_win in [false, true] {
            let mut c = Campaign::new(0);
            force_next(&mut c, BetChoice::Slots, false);
            bet(&mut c, BetChoice::Slots, 100);
            let urge = c.attributes.urge;
            force_next(&mut c, BetChoice::RouletteRed, next_win);
            bet(&mut c, BetChoice::RouletteRed, 200);
            assert_eq!(c.attributes.urge, urge + 8 + if next_win { 7 } else { 0 });
        }
        let mut c = Campaign::new(0);
        force_next(&mut c, BetChoice::Slots, false);
        bet(&mut c, BetChoice::Slots, 100);
        c.apply(Action::EndBetSession).unwrap();
        let urge = c.attributes.urge;
        force_next(&mut c, BetChoice::Slots, false);
        bet(&mut c, BetChoice::Slots, 200);
        assert_eq!(c.attributes.urge, urge);
    }

    #[test]
    fn relief_gap_follows_all_clamped_permanent_effects_without_creating_attributes() {
        for initial in 0..=100 {
            for relief in [4, 8, 100] {
                for change in [-150, -10, -3, 0, 6, 100, i16::MAX, i16::MIN] {
                    let mut actual = initial;
                    let mut baseline = initial;
                    let mut gap = relief.min(actual);
                    actual -= gap;
                    adjust_with_relief(&mut actual, &mut gap, change);
                    Attributes::adjust(&mut baseline, change);
                    assert_eq!(actual + gap, baseline);
                }
            }
        }
        let mut c = Campaign::new(9);
        c.attributes.stress = 3;
        c.attributes.boredom = 5;
        c.grant_relief(8, 4);
        c.adjust_stress(-10); // Future Care uses this same helper.
        c.adjust_boredom(-20);
        c.apply(Action::Wait).unwrap();
        c.apply(Action::Wait).unwrap();
        assert_eq!(c.attributes.stress, 0);
        assert_eq!(c.attributes.boredom, 6);
        assert!(c.betting.relief.is_none());
    }

    #[test]
    fn relief_expires_before_new_period_round_and_reopening_is_capped_per_period() {
        let mut c = Campaign::new(9);
        force_next(&mut c, BetChoice::RouletteRed, true);
        bet(&mut c, BetChoice::RouletteRed, 100);
        assert_eq!((c.attributes.boredom, c.attributes.stress), (25, 21));
        c.apply(Action::EndBetSession).unwrap();
        force_next(&mut c, BetChoice::RouletteRed, true);
        bet(&mut c, BetChoice::RouletteRed, 100);
        assert_eq!(c.clock.period(), Period::Afternoon);
        assert_eq!((c.attributes.boredom, c.attributes.stress), (28, 21));
        assert_eq!(c.betting.relief.as_ref().unwrap().stress, 4);
        // Reopening in the same afternoon gets no additional boredom relief.
        c.apply(Action::EndBetSession).unwrap();
        force_next(&mut c, BetChoice::RouletteRed, true);
        bet(&mut c, BetChoice::RouletteRed, 100);
        assert_eq!(c.attributes.boredom, 31);
        assert_eq!(c.betting.relief.as_ref().unwrap().boredom, 8);
        c.apply(Action::WatchTv).unwrap();
        assert!(c.betting.session.is_none());
        assert!(c.betting.relief.is_none());
        assert_eq!(c.attributes.stress, 22);
        assert_eq!(c.attributes.boredom, 22);
    }
}
