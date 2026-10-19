//! Bills and social commitments use the shared atomic action and clock paths.
use super::campaign::*;

impl Campaign {
    pub fn commitment_due(&self, contact: Contact) -> bool {
        self.commitments.iter().any(|plan| {
            plan.contact == contact
                && plan.day == self.clock.day
                && plan.period == self.clock.period()
                && plan.status == CommitmentStatus::Scheduled
        })
    }

    pub(super) fn pay_bill(&mut self, kind: BillKind) -> Result<(), String> {
        let index = self
            .bills
            .iter()
            .position(|bill| bill.kind == kind)
            .unwrap();
        if self.bills[index].paid_day.is_some() {
            return Err("This bill is already paid. See History for the payment.".into());
        }
        self.record_money(
            LedgerKind::BillPayment(kind),
            -self.bills[index].amount_cents,
        )?;
        self.bills[index].paid_day = Some(self.clock.day);
        Ok(())
    }

    pub(super) fn negotiate_rent(&mut self) -> Result<(), String> {
        if self.support.rent_negotiated {
            return Err(
                "The one-time rent extension is already in place: due end of day 10.".into(),
            );
        }
        let index = self
            .bills
            .iter()
            .position(|bill| bill.kind == BillKind::Rent)
            .unwrap();
        if self.bills[index].paid_day.is_some() {
            return Err("Rent is already paid; no extension is needed.".into());
        }
        if self.clock.day >= 9 {
            return Err(
                "Rent must be negotiated before housing loss at the start of day 9.".into(),
            );
        }
        self.spend_pa(1)?;
        self.support.rent_negotiated = true;
        self.bills[index].due_day = 10;
        Ok(())
    }

    pub(super) fn call_contact(&mut self, contact: Contact) -> Result<(), String> {
        if contact == Contact::Marina && self.commitment_due(contact) {
            self.complete_commitment(contact);
        }
        self.conversation()
    }

    pub(super) fn receive_visit(&mut self, contact: Contact) -> Result<(), String> {
        if contact != Contact::Rafa || !self.commitment_due(contact) {
            return Err("Rafa visits at the front door on days 4 and 9, in the afternoon. No scheduled visit is waiting now.".into());
        }
        self.complete_commitment(contact);
        self.conversation()
    }

    fn complete_commitment(&mut self, contact: Contact) {
        // Mark it before the last afternoon PA can close the commitment's period.
        self.commitments
            .iter_mut()
            .find(|plan| plan.contact == contact && plan.day == self.clock.day)
            .unwrap()
            .status = CommitmentStatus::Completed;
    }

    fn conversation(&mut self) -> Result<(), String> {
        Attributes::adjust(&mut self.attributes.relationships, 8);
        self.adjust_stress(-6);
        self.spend_pa(1)
    }

    pub(super) fn validate_obligations(&self) -> Result<(), String> {
        for bill in &self.bills {
            let mut payments = self
                .ledger
                .iter()
                .filter(|row| row.kind == LedgerKind::BillPayment(bill.kind));
            let payment = payments.next();
            if payments.next().is_some()
                || match (bill.paid_day, payment) {
                    (None, None) => false,
                    (Some(day), Some(row)) => {
                        row.clock.day != day || row.amount_cents != -bill.amount_cents
                    }
                    _ => true,
                }
            {
                return Err(
                    "Bill payments must match one exact payment in the financial ledger.".into(),
                );
            }
        }
        for plan in &self.commitments {
            let passed = (plan.day, plan.period) < (self.clock.day, self.clock.period());
            let invalid = match plan.status {
                CommitmentStatus::Scheduled => passed,
                CommitmentStatus::Missed => !passed,
                CommitmentStatus::Completed => {
                    plan.day > self.clock.day
                        || plan.day == self.clock.day && self.clock.pa_spent < 3
                }
            };
            if invalid {
                return Err(
                    "A social commitment status does not match its calendar deadline.".into(),
                );
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::campaign_save::{decode_campaign, encode_campaign};

    fn next_day(c: &mut Campaign) {
        c.apply(Action::Sleep).unwrap();
        c.apply(Action::AcknowledgeReport).unwrap();
    }
    fn day(c: &mut Campaign, target: u8) {
        while c.clock.day < target {
            next_day(c);
        }
    }
    fn reject(c: &mut Campaign, action: Action) {
        let before = c.clone();
        assert!(c.apply(action).is_err(), "{action:?}");
        assert_eq!(*c, before);
    }

    #[test]
    fn payments_are_exact_free_atomic_and_cannot_repeat_even_after_restore() {
        let mut c = Campaign::new(5);
        reject(&mut c, Action::PayBill(BillKind::Rent));
        for _ in 0..6 {
            c.apply(Action::Wait).unwrap();
        }
        let attributes = c.attributes.clone();
        let clock = c.clock;
        c.apply(Action::PayBill(BillKind::Electricity)).unwrap();
        assert_eq!(c.balance_cents, 2_000);
        assert_eq!(c.clock, clock);
        assert_eq!(c.attributes, attributes);
        assert_eq!(c.ledger.len(), 1);
        assert_eq!(c.ledger[0].amount_cents, -10_000);
        assert_eq!(c.bills[0].paid_day, Some(1));
        let mut c = decode_campaign(&encode_campaign(&c).unwrap()).unwrap();
        reject(&mut c, Action::PayBill(BillKind::Electricity));
        reject(&mut c, Action::PayBill(BillKind::Rent));
        reject(&mut c, Action::Call(Contact::Marina));
    }

    #[test]
    fn electricity_warns_then_cuts_off_and_payment_restores_it() {
        let mut c = Campaign::new(5);
        day(&mut c, 5);
        assert!(c.has_power());
        c.apply(Action::Sleep).unwrap();
        assert!(
            c.reports[4]
                .consequences
                .iter()
                .any(|text| text.contains("cutoff starts on day 6"))
        );
        c.apply(Action::AcknowledgeReport).unwrap();
        assert!(!c.has_power());
        reject(&mut c, Action::WatchTv);
        c.apply(Action::PayBill(BillKind::Electricity)).unwrap();
        assert!(c.has_power());
        c.apply(Action::WatchTv).unwrap();
    }

    #[test]
    fn rent_warning_loss_and_one_time_extension_keep_report_stages() {
        let mut c = Campaign::new(5);
        day(&mut c, 8);
        let mut extended = c.clone();
        extended.apply(Action::NegotiateRent).unwrap();
        assert_eq!(extended.clock.pa_spent, 1);
        assert_eq!(extended.bills[1].due_day, 10);
        assert_eq!(extended.balance_cents, c.balance_cents);
        reject(&mut extended, Action::NegotiateRent);
        c.apply(Action::Sleep).unwrap();
        assert!(
            c.reports[7]
                .consequences
                .iter()
                .any(|text| text.contains("housing loss"))
        );
        reject(&mut c, Action::NegotiateRent);
        c.apply(Action::AcknowledgeReport).unwrap();
        assert_eq!(c.stage, CampaignStage::Ended(Ending::HousingLoss));
        reject(&mut c, Action::PayBill(BillKind::Rent));
        reject(&mut c, Action::NegotiateRent);
        day(&mut extended, 10);
        assert_eq!(extended.stage, CampaignStage::Playing);
        next_day(&mut extended);
        assert_eq!(extended.stage, CampaignStage::Ended(Ending::DayTen));
        assert_eq!(
            extended.reports[9].overdue_bills,
            [BillKind::Electricity, BillKind::Rent]
        );

        let mut paid = Campaign::new(5);
        paid.record_money(LedgerKind::Work, 30_000).unwrap();
        paid.apply(Action::PayBill(BillKind::Rent)).unwrap();
        reject(&mut paid, Action::NegotiateRent);
        let mut no_pa = Campaign::new(5);
        for _ in 0..6 {
            no_pa.apply(Action::Wait).unwrap();
        }
        reject(&mut no_pa, Action::NegotiateRent);
    }

    #[test]
    fn matching_calls_and_visits_complete_before_final_afternoon_pa() {
        for (target, contact, action) in [
            (2, Contact::Marina, Action::Call(Contact::Marina)),
            (4, Contact::Rafa, Action::ReceiveVisit(Contact::Rafa)),
            (7, Contact::Marina, Action::Call(Contact::Marina)),
            (9, Contact::Rafa, Action::ReceiveVisit(Contact::Rafa)),
        ] {
            let mut c = Campaign::new(5);
            c.apply(Action::NegotiateRent).unwrap();
            day(&mut c, target);
            for _ in 0..3 {
                c.apply(Action::Wait).unwrap();
            }
            let relationships = c.attributes.relationships;
            let stress = c.attributes.stress;
            assert!(c.commitment_due(contact));
            c.apply(action).unwrap();
            assert_eq!(c.clock.pa_spent, 4);
            assert_eq!(c.attributes.relationships, (relationships + 8).min(100));
            assert_eq!(c.attributes.stress, stress.saturating_sub(6));
            assert_eq!(
                c.commitments
                    .iter()
                    .find(|plan| plan.day == target)
                    .unwrap()
                    .status,
                CommitmentStatus::Completed
            );
            if contact == Contact::Rafa {
                reject(&mut c, action);
            }
            c.apply(Action::Sleep).unwrap();
            assert_eq!(c.reports.last().unwrap().completed_commitments, [contact]);
            assert!(c.reports.last().unwrap().missed_commitments.is_empty());
        }
    }

    #[test]
    fn wrong_contact_phone_visit_early_sleep_and_late_calls_never_rewrite_a_miss() {
        let mut c = Campaign::new(5);
        reject(&mut c, Action::ReceiveVisit(Contact::Rafa));
        day(&mut c, 4);
        c.apply(Action::Wait).unwrap();
        c.apply(Action::Wait).unwrap();
        reject(&mut c, Action::ReceiveVisit(Contact::Marina));
        c.apply(Action::Call(Contact::Rafa)).unwrap();
        assert!(c.commitment_due(Contact::Rafa));
        c.apply(Action::Call(Contact::Marina)).unwrap();
        assert_eq!(c.commitments[1].status, CommitmentStatus::Missed);
        reject(&mut c, Action::ReceiveVisit(Contact::Rafa));
        let relationships = c.attributes.relationships;
        c.apply(Action::Call(Contact::Rafa)).unwrap();
        assert_eq!(c.attributes.relationships, (relationships + 8).min(100));
        assert_eq!(c.commitments[1].status, CommitmentStatus::Missed);
        c.apply(Action::Sleep).unwrap();
        assert_eq!(c.reports[3].missed_commitments, [Contact::Rafa]);

        let mut c = Campaign::new(5);
        day(&mut c, 2);
        c.apply(Action::Call(Contact::Marina)).unwrap(); // A morning call is not the afternoon plan.
        assert_eq!(c.commitments[0].status, CommitmentStatus::Scheduled);
        c.apply(Action::Sleep).unwrap();
        assert_eq!(c.commitments[0].status, CommitmentStatus::Missed);
        assert_eq!(c.reports[1].missed_commitments, [Contact::Marina]);
    }

    #[test]
    fn conversations_consume_relief_without_reinventing_stress_at_expiry() {
        let mut c = Campaign::new(5);
        c.clock.pa_spent = 1;
        c.attributes.stress = 2;
        c.betting.relief = Some(TemporaryRelief {
            day: 1,
            period: Period::Morning,
            boredom: 0,
            stress: 4,
        });
        c.apply(Action::Call(Contact::Marina)).unwrap();
        assert_eq!(c.attributes.stress, 0);
        assert!(c.betting.relief.is_none());
        assert_eq!(c.attributes.relationships, 73);
        assert_eq!(c.attributes.urge, 10);
    }

    #[test]
    fn obligation_validation_rejects_unbacked_payments_and_impossible_statuses() {
        let base = Campaign::new(5);
        let mut bad = base.clone();
        bad.bills[0].paid_day = Some(1);
        assert!(bad.validate().is_err());
        let mut paid = base.clone();
        paid.apply(Action::PayBill(BillKind::Electricity)).unwrap();
        for variant in 0..4 {
            let mut bad = paid.clone();
            match variant {
                0 => bad.bills[0].paid_day = None,
                1 => {
                    bad.record_money(LedgerKind::BillPayment(BillKind::Electricity), -100)
                        .unwrap();
                }
                2 => {
                    bad.ledger[0].amount_cents = -9_999;
                    bad.ledger[0].balance_after_cents += 1;
                    bad.balance_cents += 1;
                }
                _ => {
                    bad.clock.day = 2;
                    bad.bills[0].paid_day = Some(2);
                }
            }
            assert!(bad.validate().is_err());
        }
        for status in [CommitmentStatus::Completed, CommitmentStatus::Missed] {
            let mut bad = base.clone();
            bad.commitments[0].status = status;
            assert!(bad.validate().is_err());
        }
        let mut c = base;
        day(&mut c, 2);
        for _ in 0..4 {
            c.apply(Action::Wait).unwrap();
        }
        c.commitments[0].status = CommitmentStatus::Scheduled;
        assert!(c.validate().is_err());
    }

    #[test]
    fn ten_day_no_bet_budget_keeps_bills_meals_and_all_commitments() {
        let mut c = Campaign::new(5);
        let rng = c.rng.clone();
        for current_day in 1..=10 {
            if [1, 2, 3, 5, 6, 7, 8].contains(&current_day) {
                c.apply(Action::Work).unwrap();
            }
            if [4, 9].contains(&current_day) {
                c.apply(Action::Wait).unwrap();
                c.apply(Action::Wait).unwrap();
            }
            if [2, 7].contains(&current_day) {
                c.apply(Action::Call(Contact::Marina)).unwrap();
            }
            if [4, 9].contains(&current_day) {
                c.apply(Action::ReceiveVisit(Contact::Rafa)).unwrap();
            }
            if [3, 5, 8].contains(&current_day) {
                c.apply(Action::Groceries).unwrap();
            }
            c.apply(Action::Cook).unwrap();
            if current_day == 4 {
                c.apply(Action::PayBill(BillKind::Electricity)).unwrap();
            }
            if current_day == 7 {
                c.apply(Action::PayBill(BillKind::Rent)).unwrap();
            }
            while c.clock.remaining_pa() > 0 {
                c.apply(Action::WatchTv).unwrap();
            }
            assert!(c.attributes.hunger <= 60);
            next_day(&mut c);
            c = decode_campaign(&encode_campaign(&c).unwrap()).unwrap();
        }
        assert_eq!(c.stage, CampaignStage::Ended(Ending::DayTen));
        assert_eq!(c.balance_cents, 4_000);
        assert_eq!(c.household.food_portions, 2);
        assert_eq!(c.worked_days.len(), 7);
        assert_eq!(
            c.ledger
                .iter()
                .filter(|row| row.kind == LedgerKind::Food)
                .count(),
            3
        );
        assert_eq!(
            c.ledger
                .iter()
                .filter(|row| matches!(row.kind, LedgerKind::BillPayment(_)))
                .count(),
            2
        );
        assert!(
            c.commitments
                .iter()
                .all(|plan| plan.status == CommitmentStatus::Completed)
        );
        assert!(
            c.reports
                .iter()
                .all(|report| report.missed_commitments.is_empty()
                    && report.overdue_bills.is_empty())
        );
        assert!(c.betting.history.is_empty());
        assert_eq!(c.attributes.urge, 10);
        assert_eq!(c.rng, rng);
        assert_eq!(c.attributes.relationships, 97);
    }
}
