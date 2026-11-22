//! Fictional, gradual support and voluntary saved wagering limits.
use super::campaign::*;

impl Campaign {
    pub(super) fn care_conversation(&mut self) -> Result<(), String> {
        self.adjust_stress(-10);
        Attributes::adjust(&mut self.attributes.urge, -6);
        self.spend_pa(1)?;
        self.support.care_actions = self
            .support
            .care_actions
            .checked_add(1)
            .ok_or("No more care conversations can be recorded.")?;
        Ok(())
    }

    pub(super) fn financial_planning(&mut self) -> Result<(), String> {
        self.adjust_stress(-6);
        self.spend_pa(1)?;
        self.support.planning_actions = self
            .support
            .planning_actions
            .checked_add(1)
            .ok_or("No more planning actions can be recorded.")?;
        Ok(())
    }

    pub(super) fn set_daily_cap(&mut self, cents: Option<i64>) -> Result<(), String> {
        if cents.is_some_and(|value| value < 0 || value % 100 != 0) {
            return Err("Choose a whole-real daily cap of R$0 or more.".into());
        }
        if self.support.daily_cap_cents.is_none()
            || cents.is_some_and(|value| value <= self.support.daily_cap_cents.unwrap())
        {
            self.support.daily_cap_cents = cents;
            self.support.pending_cap = None;
        } else {
            self.support.pending_cap = Some(PendingCap {
                cents,
                effective_day: self.clock.day + 1,
            });
        }
        Ok(())
    }

    pub(super) fn validate_support(&self) -> Result<(), String> {
        let invalid_amount = |value: i64| value < 0 || value % 100 != 0;
        if self.support.daily_cap_cents.is_some_and(invalid_amount)
            || self.support.pending_cap.as_ref().is_some_and(|pending| {
                pending.cents.is_some_and(invalid_amount)
                    || pending.effective_day != self.clock.day + 1
                    || self
                        .support
                        .daily_cap_cents
                        .is_none_or(|current| pending.cents.is_some_and(|value| value <= current))
            })
        {
            return Err("Daily caps must use whole reais; only increases or removal may wait until tomorrow.".into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bet(c: &mut Campaign, stake_cents: i64) {
        c.apply(Action::Bet {
            choice: BetChoice::RouletteRed,
            stake_cents,
        })
        .unwrap();
    }

    fn morning(c: &mut Campaign) {
        c.apply(Action::Sleep).unwrap();
        c.apply(Action::AcknowledgeReport).unwrap();
    }

    #[test]
    fn care_and_planning_take_one_pa_with_gradual_clamped_effects_and_counts() {
        let mut c = Campaign::new(5);
        let rng = c.rng.clone();
        c.apply(Action::CareConversation).unwrap();
        assert_eq!(
            (c.clock.pa_spent, c.attributes.stress, c.attributes.urge),
            (1, 20, 24)
        );
        assert_eq!((c.attributes.hunger, c.attributes.boredom), (33, 33));
        assert_eq!((c.support.care_actions, c.support.planning_actions), (1, 0));
        c.apply(Action::FinancialPlanning).unwrap();
        assert_eq!(
            (c.clock.pa_spent, c.attributes.stress, c.attributes.urge),
            (2, 14, 24)
        );
        c.apply(Action::CareConversation).unwrap();
        assert_eq!((c.attributes.stress, c.attributes.urge), (4, 18));
        c.apply(Action::FinancialPlanning).unwrap();
        assert_eq!(c.attributes.stress, 0);
        assert_eq!(
            (
                c.clock.pa_spent,
                c.support.care_actions,
                c.support.planning_actions
            ),
            (4, 2, 2)
        );
        assert_eq!(c.stage, CampaignStage::Playing);
        assert_eq!(c.balance_cents, INITIAL_BALANCE_CENTS);
        assert!(c.ledger.is_empty());
        assert_eq!(c.rng, rng);
    }

    #[test]
    fn care_and_planning_consume_actual_relief_without_reintroducing_clamped_stress() {
        for stress in [0, 3, 9, 25, 99, 100] {
            for (action, reduction) in [
                (Action::CareConversation, 10),
                (Action::FinancialPlanning, 6),
            ] {
                let mut c = Campaign::new(5);
                c.attributes.stress = stress;
                c.attributes.boredom = 2;
                for _ in 0..3 {
                    let mut forecast = c.rng.clone();
                    let choice = if forecast.sample_below(100).unwrap() < 50 {
                        BetChoice::RouletteRed
                    } else {
                        BetChoice::RouletteBlack
                    };
                    c.apply(Action::Bet {
                        choice,
                        stake_cents: 100,
                    })
                    .unwrap();
                }
                let before = c.clone();
                c.apply(action).unwrap();
                assert_eq!(c.attributes.stress, stress.saturating_sub(reduction));
                assert_eq!(c.attributes.boredom, 8);
                assert!(c.betting.relief.is_none());
                assert!(c.betting.session.is_none());
                assert_eq!(c.clock.pa_spent, 2);
                assert_eq!(c.betting.history, before.betting.history);
                assert_eq!(c.rng, before.rng);
                c.apply(Action::WatchTv).unwrap();
                c.apply(Action::Wait).unwrap();
                assert_eq!(
                    c.attributes.stress,
                    stress.saturating_sub(reduction).saturating_sub(3)
                );
            }
        }
        let mut c = Campaign::new(5);
        c.apply(Action::Wait).unwrap();
        c.attributes.stress = 68;
        c.apply(Action::CareConversation).unwrap();
        assert_eq!(c.household.mess, 0);
    }

    #[test]
    fn failed_support_actions_are_atomic_at_zero_pa_and_outside_playing() {
        for action in [Action::CareConversation, Action::FinancialPlanning] {
            let mut full = Campaign::new(5);
            full.support.care_actions = u16::MAX;
            full.support.planning_actions = u16::MAX;
            let before = full.clone();
            assert!(full.apply(action).is_err());
            assert_eq!(full, before);
        }
        let mut c = Campaign::new(5);
        for _ in 0..6 {
            c.apply(Action::Wait).unwrap();
        }
        for action in [
            Action::CareConversation,
            Action::FinancialPlanning,
            Action::SetDailyCap(Some(-100)),
            Action::SetDailyCap(Some(50)),
        ] {
            let before = c.clone();
            assert!(c.apply(action).is_err());
            assert_eq!(c, before);
        }
        c.apply(Action::SetDailyCap(Some(0))).unwrap();
        c.apply(Action::BlockBetting).unwrap();
        let before = c.clone();
        assert!(c.apply(Action::BlockBetting).is_err());
        assert_eq!(c, before);
        for stage in [
            CampaignStage::DailyReport,
            CampaignStage::Ended(Ending::FinalDay),
        ] {
            let mut stopped = Campaign::new(5);
            stopped.stage = stage;
            for action in [
                Action::CareConversation,
                Action::FinancialPlanning,
                Action::SetDailyCap(None),
                Action::BlockBetting,
            ] {
                let before = stopped.clone();
                assert!(stopped.apply(action).is_err());
                assert_eq!(stopped, before);
            }
        }
    }

    #[test]
    fn caps_lower_now_replace_pending_and_only_raise_or_remove_in_the_morning() {
        let mut c = Campaign::new(5);
        c.apply(Action::SetDailyCap(None)).unwrap();
        assert!(c.support.pending_cap.is_none());
        c.apply(Action::SetDailyCap(Some(1000))).unwrap();
        assert_eq!(c.support.daily_cap_cents, Some(1000));
        for value in [Some(2000), Some(1500), None] {
            c.apply(Action::SetDailyCap(value)).unwrap();
            assert_eq!(c.support.daily_cap_cents, Some(1000));
            assert_eq!(
                c.support.pending_cap,
                Some(PendingCap {
                    cents: value,
                    effective_day: 2
                })
            );
        }
        c.apply(Action::SetDailyCap(Some(1000))).unwrap();
        assert!(c.support.pending_cap.is_none());
        c.apply(Action::SetDailyCap(Some(2000))).unwrap();
        c.apply(Action::SetDailyCap(Some(500))).unwrap();
        assert_eq!(c.support.daily_cap_cents, Some(500));
        assert!(c.support.pending_cap.is_none());
        c.apply(Action::SetDailyCap(Some(0))).unwrap();
        c.apply(Action::SetDailyCap(Some(100))).unwrap();
        assert_eq!(c.support.daily_cap_cents, Some(0));
        morning(&mut c);
        assert_eq!(c.support.daily_cap_cents, Some(100));
        assert!(c.support.pending_cap.is_none());
        c.apply(Action::SetDailyCap(None)).unwrap();
        assert_eq!(c.support.daily_cap_cents, Some(100));
        morning(&mut c);
        assert!(c.support.daily_cap_cents.is_none());
        assert_eq!(c.clock.pa_spent, 0);
    }

    #[test]
    fn earlier_gross_stakes_and_block_survive_reentry_without_changing_committed_results() {
        let mut c = Campaign::new(5);
        bet(&mut c, 1000);
        let settled = c.clone();
        c.apply(Action::SetDailyCap(Some(1000))).unwrap();
        for _ in 0..2 {
            let before = c.clone();
            assert!(
                c.apply(Action::Bet {
                    choice: BetChoice::Slots,
                    stake_cents: 100
                })
                .is_err()
            );
            assert_eq!(c, before);
            c.apply(Action::EndBetSession).unwrap();
        }
        c.apply(Action::SetDailyCap(Some(0))).unwrap();
        let before = c.clone();
        assert!(
            c.apply(Action::Bet {
                choice: BetChoice::RouletteRed,
                stake_cents: 100
            })
            .is_err()
        );
        assert_eq!(c, before);
        c.apply(Action::BlockBetting).unwrap();
        assert_eq!(c.betting.history, settled.betting.history);
        assert_eq!(c.balance_cents, settled.balance_cents);
        assert_eq!(c.rng, settled.rng);
        c.apply(Action::SetDailyCap(None)).unwrap();
        morning(&mut c);
        assert!(!c.support.app_blocked_today);
        assert!(c.support.daily_cap_cents.is_none());
        bet(&mut c, 100);
    }

    #[test]
    fn saved_caps_reject_fractional_negative_and_non_increasing_pending_values() {
        for value in [-100, 1, 150] {
            let mut c = Campaign::new(5);
            c.support.daily_cap_cents = Some(value);
            assert!(c.validate().is_err());
        }
        for (current, pending, day) in [
            (None, Some(100), 2),
            (Some(100), Some(100), 2),
            (Some(100), Some(0), 2),
            (Some(100), Some(150), 2),
            (Some(100), None, 3),
        ] {
            let mut c = Campaign::new(5);
            c.support.daily_cap_cents = current;
            c.support.pending_cap = Some(PendingCap {
                cents: pending,
                effective_day: day,
            });
            assert!(c.validate().is_err());
        }
    }
}
