//! Read-only finance views; campaign actions remain authoritative.
use super::*;

const INK: Color = Color::srgb(0.12, 0.10, 0.17);
const MUTED: Color = Color::srgb(0.38, 0.36, 0.43);
const BUTTON: Color = Color::srgb(0.95, 0.93, 0.97);
const WHITE: Color = Color::WHITE;
const PURPLE: Color = Color::srgb(0.43, 0.15, 0.70);

pub(super) fn actions(phone: &Phone, c: &Campaign) -> Vec<(String, PhoneCommand)> {
    match phone.page {
        PhonePage::Bank => {
            let mut result: Vec<_> = c
                .bills
                .iter()
                .filter(|b| b.paid_day.is_none())
                .map(|b| {
                    (
                        format!("Pay {} · {}", b.kind.label(), reais(b.amount_cents)),
                        PhoneCommand::Apply(Action::PayBill(b.kind)),
                    )
                })
                .collect();
            if !c.support.rent_negotiated
                && c.clock.day < 9
                && c.bills
                    .iter()
                    .any(|b| b.kind == BillKind::Rent && b.paid_day.is_none())
            {
                result.push((
                    "Extend rent · 1 PA".into(),
                    PhoneCommand::Apply(Action::NegotiateRent),
                ));
            }
            result.push(("Activity".into(), PhoneCommand::Page(PhonePage::History)));
            result
        }
        PhonePage::History => vec![(
            "Daily reports".into(),
            PhoneCommand::Page(PhonePage::Reports),
        )],
        _ => vec![],
    }
}

fn card() -> Node {
    Node {
        padding: UiRect::all(px(14)),
        border_radius: BorderRadius::all(px(12)),
        flex_shrink: 0.0,
        ..ui::column()
    }
}

pub(super) fn render(
    parent: &mut ChildSpawnerCommands,
    assets: &PhoneAssets,
    art: &ui::Art,
    phone: &Phone,
    c: &Campaign,
) {
    let choices = actions(phone, c);
    match phone.page {
        PhonePage::Bank => {
            parent
                .spawn((card(), BackgroundColor(PURPLE)))
                .with_children(|p| {
                    text(p, &art.regular, "Violeta", 20.0, WHITE);
                    text(p, &art.regular, "Available balance", 18.0, WHITE);
                    text(p, &assets.font, reais(c.balance_cents), 35.0, WHITE);
                });
            text(parent, &assets.font, "Bills", 24.0, INK);
            for bill in &c.bills {
                parent
                    .spawn((card(), BackgroundColor(BUTTON)))
                    .with_children(|p| {
                        p.spawn(Node {
                            justify_content: JustifyContent::SpaceBetween,
                            ..ui::row()
                        })
                        .with_children(|row| {
                            text(row, &assets.font, bill.kind.label(), 22.0, INK);
                            text(row, &assets.font, reais(bill.amount_cents), 22.0, INK);
                        });
                        text(
                            p,
                            &art.regular,
                            bill.paid_day
                                .map(|day| format!("Paid · day {day}"))
                                .unwrap_or_else(|| {
                                    format!(
                                        "{} · due day {}",
                                        if c.clock.day > bill.due_day {
                                            "Overdue"
                                        } else {
                                            "Unpaid"
                                        },
                                        bill.due_day
                                    )
                                }),
                            18.0,
                            MUTED,
                        );
                        if bill.paid_day.is_none() {
                            let command = PhoneCommand::Apply(Action::PayBill(bill.kind));
                            if let Some(index) = choices.iter().position(|(_, cmd)| *cmd == command)
                            {
                                ui::action(
                                    p,
                                    &assets.font,
                                    phone.page,
                                    &format!("{}  Pay", index + 1),
                                    command,
                                    PURPLE,
                                    WHITE,
                                );
                            }
                        }
                        if bill.kind == BillKind::Rent {
                            if let Some((label, command)) = choices
                                .iter()
                                .find(|(_, cmd)| *cmd == PhoneCommand::Apply(Action::NegotiateRent))
                            {
                                text(p, &art.regular, "Extend to day 10", 17.0, MUTED);
                                ui::action(
                                    p,
                                    &assets.font,
                                    phone.page,
                                    &format!(
                                        "{}  {label}",
                                        choices.iter().position(|(_, cmd)| cmd == command).unwrap()
                                            + 1
                                    ),
                                    *command,
                                    PURPLE,
                                    WHITE,
                                );
                            }
                        }
                    });
            }
            text(
                parent,
                &art.regular,
                if c.has_power() {
                    "Power on · payments use no PA"
                } else {
                    "Power off · pay electricity to restore it"
                },
                18.0,
                MUTED,
            );
            ui::action(
                parent,
                &assets.font,
                phone.page,
                &format!("{}  Activity", choices.len()),
                PhoneCommand::Page(PhonePage::History),
                PURPLE,
                WHITE,
            );
        }
        PhonePage::History => {
            text(parent, &assets.font, "Activity", 30.0, PURPLE);
            ui::action(
                parent,
                &assets.font,
                phone.page,
                "1  Daily reports",
                PhoneCommand::Page(PhonePage::Reports),
                PURPLE,
                WHITE,
            );
            if c.ledger.is_empty() {
                text(parent, &art.regular, "No transactions yet", 21.0, MUTED);
            }
            let mut rounds = c.betting.history.iter();
            for entry in &c.ledger {
                parent
                    .spawn((card(), BackgroundColor(BUTTON)))
                    .with_children(|p| {
                        let label = match entry.kind {
                            LedgerKind::Work => "MeFood work".into(),
                            LedgerKind::Food => "Food purchase".into(),
                            LedgerKind::BetStake => "Bet stake".into(),
                            LedgerKind::BetPayout => "Payout · includes stake".into(),
                            LedgerKind::BillPayment(kind) => format!("{} payment", kind.label()),
                            LedgerKind::Sale(item) => format!("{} sale", item.label()),
                        };
                        text(
                            p,
                            &assets.font,
                            format!("{label}  {}", reais(entry.amount_cents)),
                            21.0,
                            INK,
                        );
                        text(
                            p,
                            &art.regular,
                            format!(
                                "Day {} · {} · PA {}\nBalance {}",
                                entry.clock.day,
                                entry.clock.period().label(),
                                entry.clock.pa_spent,
                                reais(entry.balance_after_cents)
                            ),
                            17.0,
                            MUTED,
                        );
                        if entry.kind == LedgerKind::BetStake
                            && let Some(round) = rounds.next()
                        {
                            text(p, &art.regular, round_text(round), 17.0, MUTED);
                        }
                    });
            }
            text(parent, &assets.font, "Daily totals", 24.0, INK);
            for day in 1..=c.clock.day {
                parent
                    .spawn((card(), BackgroundColor(BUTTON)))
                    .with_children(|p| {
                        text(p, &assets.font, format!("Day {day}"), 21.0, INK);
                        text(
                            p,
                            &art.regular,
                            c.daily_finances(day)
                                .map(|f| totals(&f))
                                .unwrap_or_else(|e| e),
                            18.0,
                            MUTED,
                        );
                    });
            }
        }
        PhonePage::Reports => {
            text(parent, &assets.font, "Daily reports", 30.0, PURPLE);
            if c.reports.is_empty() {
                text(
                    parent,
                    &art.regular,
                    "No reports yet\nRecorded after sleep",
                    22.0,
                    MUTED,
                );
            }
            for report in c.reports.iter().rev() {
                parent.spawn((card(), BackgroundColor(BUTTON))).with_children(|p| {
                    text(p, &assets.font, format!("Day {}", report.day), 25.0, INK);
                    text(p, &art.regular, totals(&report.finances), 18.0, INK);
                    text(p, &art.regular, format!("PA used {} / 6 · elapsed in sleep {}\nRelationships {} → {} ({:+})", report.pa_used, PA_PER_DAY - report.pa_used, report.relationships_opening, report.relationships_closing, i16::from(report.relationships_closing) - i16::from(report.relationships_opening)), 18.0, MUTED);
                    let names = |contacts: &[Contact]| if contacts.is_empty() { "None".into() } else { contacts.iter().map(|c| c.plan_label()).collect::<Vec<_>>().join(", ") };
                    text(p, &art.regular, format!("Kept: {}\nMissed: {}\nOverdue: {}", names(&report.completed_commitments), names(&report.missed_commitments), if report.overdue_bills.is_empty() { "None".into() } else { report.overdue_bills.iter().map(|b| b.label()).collect::<Vec<_>>().join(", ") }), 18.0, MUTED);
                    for consequence in &report.consequences { text(p, &art.regular, consequence, 18.0, INK); }
                });
            }
        }
        _ => {}
    }
}

fn totals(f: &DailyFinances) -> String {
    format!(
        "Opening {} · closing {}\nWork +{} · sales +{}\nFood -{} · bills -{}\nStaked {} · returned {}\nBetting net {}",
        reais(f.opening_cents),
        reais(f.closing_cents),
        reais(f.work_cents),
        reais(f.sales_cents),
        reais(f.food_cents),
        reais(f.bills_cents),
        reais(f.gross_bets_cents),
        reais(f.payouts_cents),
        reais(f.payouts_cents - f.gross_bets_cents)
    )
}

fn round_text(r: &BetRound) -> String {
    let choice = match r.choice {
        BetChoice::RouletteRed => "Roulette · red",
        BetChoice::RouletteBlack => "Roulette · black",
        BetChoice::Slots => "Slots",
        BetChoice::FootballHome => "Football · home",
        BetChoice::FootballDraw => "Football · draw",
        BetChoice::FootballAway => "Football · away",
    };
    let outcome = match r.outcome {
        BetOutcome::RouletteRed => "red",
        BetOutcome::RouletteBlack => "black",
        BetOutcome::SlotsWin => "win",
        BetOutcome::SlotsLoss => "loss",
        BetOutcome::FootballHome => "home",
        BetOutcome::FootballDraw => "draw",
        BetOutcome::FootballAway => "away",
    };
    format!(
        "{choice} → {outcome}\nStake {} · return {} · net {}\nSession {} · round {} · {} PA",
        reais(r.stake_cents),
        reais(r.total_return_cents),
        reais(r.net_cents),
        r.session_id,
        r.session_round,
        r.pa_consumed
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bank_registry_keeps_free_payments_and_hides_used_extension() {
        let mut c = Campaign::new(1);
        let phone = Phone {
            page: PhonePage::Bank,
            ..default()
        };
        assert_eq!(actions(&phone, &c).len(), 4);
        c.support.rent_negotiated = true;
        assert!(
            !actions(&phone, &c)
                .iter()
                .any(|(_, cmd)| *cmd == PhoneCommand::Apply(Action::NegotiateRent))
        );
        c.clock.pa_spent = PA_PER_DAY;
        assert!(actions(&phone, &c).iter().any(|(_, cmd)| *cmd == PhoneCommand::Apply(Action::PayBill(BillKind::Electricity))));
    }
}
