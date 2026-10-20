//! Conversations and support tools backed by existing campaign actions.
use super::*;

const INK: Color = Color::srgb(0.10, 0.19, 0.17);
const MUTED: Color = Color::srgb(0.36, 0.44, 0.41);
const GREEN: Color = Color::srgb(0.05, 0.43, 0.32);
const LIGHT: Color = Color::srgb(0.90, 0.95, 0.91);

pub(super) fn actions(phone: &Phone, c: &Campaign) -> Vec<(String, PhoneCommand)> {
    match phone.page {
        PhonePage::Contacts => [Contact::Marina, Contact::Rafa]
            .into_iter()
            .map(|contact| {
                (
                    contact.label().into(),
                    PhoneCommand::Page(PhonePage::ContactThread(contact)),
                )
            })
            .collect(),
        PhonePage::ContactThread(contact) => vec![(
            format!(
                "{} {} · 1 PA",
                if contact == Contact::Marina && c.commitment_due(contact) {
                    "Answer"
                } else {
                    "Call"
                },
                contact.label()
            ),
            PhoneCommand::Apply(Action::Call(contact)),
        )],
        PhonePage::Support => {
            let mut result = vec![
                (
                    "Talk · 1 PA".into(),
                    PhoneCommand::Apply(Action::CareConversation),
                ),
                (
                    "Plan · 1 PA".into(),
                    PhoneCommand::Apply(Action::FinancialPlanning),
                ),
                (
                    "Daily betting cap".into(),
                    PhoneCommand::Page(PhonePage::DailyCap),
                ),
            ];
            if !c.support.app_blocked_today {
                result.push((
                    "Block betting until morning".into(),
                    PhoneCommand::Apply(Action::BlockBetting),
                ));
            }
            result.extend([
                ("Contacts".into(), PhoneCommand::Page(PhonePage::Contacts)),
                ("Bank".into(), PhoneCommand::Page(PhonePage::Bank)),
                ("Activity".into(), PhoneCommand::Page(PhonePage::History)),
            ]);
            result
        }
        PhonePage::DailyCap if phone.cap_entry.is_none() => {
            let mut result = vec![(
                format!("Apply {} cap", reais(phone.cap_cents)),
                PhoneCommand::Apply(Action::SetDailyCap(Some(phone.cap_cents))),
            )];
            if c.support.daily_cap_cents.is_some() {
                result.push((
                    "Remove cap tomorrow".into(),
                    PhoneCommand::Apply(Action::SetDailyCap(None)),
                ));
            }
            result
        }
        _ => vec![],
    }
}

fn card() -> Node {
    Node {
        padding: UiRect::all(px(12)),
        border_radius: BorderRadius::all(px(12)),
        flex_shrink: 0.0,
        ..ui::column()
    }
}
fn tile(contact: Contact) -> ui::Tile {
    match contact {
        Contact::Marina => ui::Tile::Marina,
        Contact::Rafa => ui::Tile::Rafa,
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
        PhonePage::Contacts => {
            text(parent, &assets.font, "Conversa", 30.0, GREEN);
            text(parent, &art.regular, "Messages", 18.0, MUTED);
            for (index, (contact, (_, command))) in [Contact::Marina, Contact::Rafa]
                .into_iter()
                .zip(&choices)
                .enumerate()
            {
                parent
                    .spawn((
                        Button,
                        PhoneButton(phone.page, *command),
                        card(),
                        BackgroundColor(LIGHT),
                    ))
                    .observe(queue_press)
                    .with_children(|p| {
                        p.spawn(ui::row()).with_children(|row| {
                            ui::picture(row, art, tile(contact), 48.0, 48.0);
                            row.spawn(Node {
                                width: auto(),
                                flex_grow: 1.0,
                                min_width: px(0),
                                ..ui::column()
                            })
                            .with_children(|details| {
                                text(
                                    details,
                                    &assets.font,
                                    format!("{}  {}", index + 1, contact.label()),
                                    22.0,
                                    INK,
                                );
                                text(
                                    details,
                                    &art.regular,
                                    if c.commitments.iter().any(|p| {
                                        p.contact == contact && p.status == CommitmentStatus::Missed
                                    }) {
                                        "We missed our plan."
                                    } else if c.commitment_due(contact) {
                                        "Calling this afternoon…"
                                    } else if contact == Contact::Marina {
                                        "How was your day?"
                                    } else {
                                        "I'm nearby."
                                    },
                                    18.0,
                                    MUTED,
                                );
                            });
                        });
                    });
            }
        }
        PhonePage::ContactThread(contact) => {
            parent.spawn(ui::row()).with_children(|row| {
                ui::picture(row, art, tile(contact), 48.0, 48.0);
                text(row, &assets.font, contact.label(), 28.0, GREEN);
            });
            parent
                .spawn((card(), BackgroundColor(LIGHT)))
                .with_children(|p| {
                    text(p, &art.regular, preview(c, contact), 21.0, INK);
                    text(p, &art.regular, context(c, contact), 18.0, MUTED);
                });
            for plan in c.commitments.iter().filter(|p| p.contact == contact) {
                parent
                    .spawn((card(), BackgroundColor(Color::srgb(0.95, 0.96, 0.95))))
                    .with_children(|p| {
                        text(
                            p,
                            &assets.font,
                            if contact == Contact::Marina {
                                "Phone call"
                            } else {
                                "Visit · front door"
                            },
                            20.0,
                            INK,
                        );
                        text(
                            p,
                            &art.regular,
                            format!(
                                "Day {} · {} · {}",
                                plan.day,
                                plan.period.label(),
                                match plan.status {
                                    CommitmentStatus::Scheduled => "Scheduled",
                                    CommitmentStatus::Completed => "Kept",
                                    CommitmentStatus::Missed => "Missed",
                                }
                            ),
                            18.0,
                            MUTED,
                        );
                    });
            }
            for (index, (label, command)) in choices.into_iter().enumerate() {
                ui::action(
                    parent,
                    &assets.font,
                    phone.page,
                    &format!("{}  {label}", index + 1),
                    command,
                    GREEN,
                    Color::WHITE,
                );
            }
            text(
                parent,
                &art.regular,
                "Relationships +8 · stress −6",
                17.0,
                MUTED,
            );
            text(
                parent,
                &art.regular,
                if contact == Contact::Rafa {
                    "Phone calls do not replace door visits."
                } else {
                    "Answer scheduled calls during the afternoon."
                },
                17.0,
                MUTED,
            );
        }
        PhonePage::Support => {
            text(parent, &assets.font, "Respiro", 30.0, GREEN);
            text(
                parent,
                &art.regular,
                "Fictional care desk · one step at a time",
                18.0,
                MUTED,
            );
            for (index, (label, command)) in
                choices.iter().enumerate().filter(|(_, (_, command))| {
                    !matches!(
                        command,
                        PhoneCommand::Page(
                            PhonePage::Contacts | PhonePage::Bank | PhonePage::History
                        )
                    )
                })
            {
                ui::action(
                    parent,
                    &assets.font,
                    phone.page,
                    &format!("{}  {label}", index + 1),
                    *command,
                    if index < 2 { GREEN } else { LIGHT },
                    if index < 2 { Color::WHITE } else { INK },
                );
                if index < 2 {
                    text(
                        parent,
                        &art.regular,
                        if index == 0 {
                            "Stress −10 · urge −6"
                        } else {
                            "Stress −6"
                        },
                        17.0,
                        MUTED,
                    );
                }
            }
            parent.spawn(ui::row()).with_children(|row| {
                for (index, (label, command)) in
                    choices.iter().enumerate().filter(|(_, (_, command))| {
                        matches!(
                            command,
                            PhoneCommand::Page(
                                PhonePage::Contacts | PhonePage::Bank | PhonePage::History
                            )
                        )
                    })
                {
                    row.spawn(Node {
                        flex_grow: 1.0,
                        flex_basis: px(0),
                        min_width: px(0),
                        ..ui::column()
                    })
                    .with_children(|p| {
                        ui::action(
                            p,
                            &art.regular,
                            phone.page,
                            &format!("{} {label}", index + 1),
                            *command,
                            LIGHT,
                            INK,
                        );
                    });
                }
            });
            text(
                parent,
                &art.regular,
                format!(
                    "{} care steps · {} planning steps\nBetting {} · information always free",
                    c.support.care_actions,
                    c.support.planning_actions,
                    if c.support.app_blocked_today {
                        "blocked until morning"
                    } else {
                        "available"
                    }
                ),
                17.0,
                MUTED,
            );
        }
        PhonePage::DailyCap => {
            text(parent, &assets.font, "Daily betting cap", 28.0, GREEN);
            text(
                parent,
                &art.regular,
                format!(
                    "Current {} · staked today {}",
                    c.support
                        .daily_cap_cents
                        .map(reais)
                        .unwrap_or_else(|| "No cap".into()),
                    reais(
                        c.daily_finances(c.clock.day)
                            .map_or(0, |f| f.gross_bets_cents)
                    )
                ),
                19.0,
                INK,
            );
            if let Some(pending) = &c.support.pending_cap {
                text(
                    parent,
                    &art.regular,
                    format!(
                        "Pending: {} · day {} morning",
                        pending.cents.map(reais).unwrap_or_else(|| "No cap".into()),
                        pending.effective_day
                    ),
                    18.0,
                    GREEN,
                );
            }
            if let Some(entry) = &phone.cap_entry {
                text(parent, &assets.font, format!("R${entry}_"), 32.0, INK);
                text(
                    parent,
                    &art.regular,
                    "Whole reais · Enter selects",
                    17.0,
                    MUTED,
                );
                ui::action(
                    parent,
                    &assets.font,
                    phone.page,
                    "Select amount",
                    PhoneCommand::ConfirmCap,
                    GREEN,
                    Color::WHITE,
                );
            } else {
                ui::action(
                    parent,
                    &assets.font,
                    phone.page,
                    &format!("{} · edit", reais(phone.cap_cents)),
                    PhoneCommand::EditCap,
                    LIGHT,
                    INK,
                );
                parent.spawn(ui::row()).with_children(|row| {
                    for (label, delta) in [("−10", -1000), ("−1", -100), ("+1", 100), ("+10", 1000)]
                    {
                        row.spawn(Node {
                            flex_grow: 1.0,
                            flex_basis: px(0),
                            ..ui::column()
                        })
                        .with_children(|p| {
                            ui::action(
                                p,
                                &assets.font,
                                phone.page,
                                label,
                                PhoneCommand::Cap(delta),
                                LIGHT,
                                INK,
                            );
                        });
                    }
                });
            }
            for (index, (label, command)) in choices.into_iter().enumerate() {
                ui::action(
                    parent,
                    &assets.font,
                    phone.page,
                    &format!("{}  {label}", index + 1),
                    command,
                    GREEN,
                    Color::WHITE,
                );
            }
            text(
                parent,
                &art.regular,
                "R$0 → betting blocked\nLower cap → now · clears pending change\nHigher / removed cap → morning\nSame cap → clears pending change\nPayouts never refill the cap",
                17.0,
                MUTED,
            );
        }
        _ => {}
    }
}

pub(super) fn preview(c: &Campaign, contact: Contact) -> &'static str {
    if c.commitments
        .iter()
        .any(|p| p.contact == contact && p.status == CommitmentStatus::Missed)
    {
        "We missed our plan. We can still make time to talk."
    } else if contact == Contact::Marina && c.commitment_due(contact) {
        "I'm calling this afternoon. Are you there?"
    } else if c.attributes.relationships >= 80 {
        "I'm glad we've kept making time for each other."
    } else if c.attributes.relationships < 45 {
        "It's been a while. Shall we talk?"
    } else if contact == Contact::Marina {
        "How was your day? You can call, even when money is tight."
    } else {
        "I'm nearby. Our afternoon visits are at your front door."
    }
}

pub(super) fn context(c: &Campaign, contact: Contact) -> String {
    let mut message = if contact == Contact::Marina {
        let unpaid: i64 = c
            .bills
            .iter()
            .filter(|b| b.paid_day.is_none())
            .map(|b| b.amount_cents)
            .sum();
        format!(
            "You have {}. {} in unpaid bills. Check Bank for deadlines.",
            reais(c.balance_cents),
            reais(unpaid)
        )
    } else if c.betting.history.is_empty() {
        "We can spend time together without betting.".into()
    } else {
        let net: i128 = c
            .betting
            .history
            .iter()
            .map(|r| i128::from(r.net_cents))
            .sum();
        format!(
            "{} bets, net {}R${}.{:02}. {}",
            c.betting.history.len(),
            if net < 0 { "−" } else { "" },
            net.unsigned_abs() / 100,
            net.unsigned_abs() % 100,
            if c.betting.history.iter().any(|r| r.session_round >= 4) {
                "Some sessions ran beyond three rounds. Let's talk."
            } else {
                "We can talk without another bet."
            }
        )
    };
    if c.support.care_actions > 0 || c.support.planning_actions > 0 {
        message.push_str(" You've taken care or planning steps. Keep going, one step at a time.");
    }
    message
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn thread_only_authorizes_its_contact_and_cap_draft_never_applies() {
        let c = Campaign::new(1);
        let mut phone = Phone {
            page: PhonePage::ContactThread(Contact::Rafa),
            ..default()
        };
        assert_eq!(
            actions(&phone, &c)[0].1,
            PhoneCommand::Apply(Action::Call(Contact::Rafa))
        );
        phone.page = PhonePage::DailyCap;
        phone.cap_entry = Some("12".into());
        assert!(actions(&phone, &c).is_empty());
        assert!(!context(&c, Contact::Rafa).is_empty());
    }
}
