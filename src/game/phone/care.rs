//! Conversations and support tools backed by existing campaign actions.
use super::*;

const INK: Color = Color::srgb(0.10, 0.19, 0.17);
const MUTED: Color = Color::srgb(0.36, 0.44, 0.41);
const GREEN: Color = Color::srgb(0.05, 0.43, 0.32);
const LIGHT: Color = Color::srgb(0.90, 0.95, 0.91);

pub(super) fn actions(phone: &Phone, c: &Campaign, localization: &Localization) -> Vec<(String, PhoneCommand)> {
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
                if c.uses_phone_for(contact) && c.commitment_due(contact) {
                    localization.choose("Answer", "Atender")
                } else {
                    localization.choose("Call", "Ligar para")
                },
                contact.label()
            ),
            PhoneCommand::Apply(Action::Call(contact)),
        )],
        PhonePage::Support => {
            let mut result = vec![
                (
                    localization.choose("Talk · 1 PA", "Conversar · 1 PA").into(),
                    PhoneCommand::Apply(Action::CareConversation),
                ),
                (
                    localization.choose("Plan · 1 PA", "Planejar as finanças · 1 PA").into(),
                    PhoneCommand::Apply(Action::FinancialPlanning),
                ),
                (
                    localization.text("therapy_session").into(),
                    PhoneCommand::Apply(Action::TherapySession),
                ),
                (
                    localization.choose("Daily betting cap", "Limite diário de apostas").into(),
                    PhoneCommand::Page(PhonePage::DailyCap),
                ),
            ];
            if !c.support.app_blocked_today {
                result.push((
                    localization.choose("Block betting until morning", "Bloquear apostas até amanhã").into(),
                    PhoneCommand::Apply(Action::BlockBetting),
                ));
            }
            if !c.household.sold.is_empty() || life::items().any(|item| c.can_sell(item).is_ok()) {
                result.push((localization.choose("Sales", "Vendas").into(), PhoneCommand::Page(PhonePage::Household)));
            }
            result
        }
        PhonePage::DailyCap if phone.cap_entry.is_none() => {
            let mut result = vec![(
                format_text!(localization, "Apply {} cap", "Aplicar limite de {}", localization.reais(phone.cap_cents)),
                PhoneCommand::Apply(Action::SetDailyCap(Some(phone.cap_cents))),
            )];
            if c.support.daily_cap_cents.is_some() {
                result.push((
                    localization.choose("Remove cap tomorrow", "Remover o limite amanhã").into(),
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
    localization: &Localization,
) {
    let choices = actions(phone, c, localization);
    match phone.page {
        PhonePage::Contacts => {
            text(parent, &assets.font, "Conversa", 30.0, GREEN);
            text(parent, &art.regular, localization.choose("Messages", "Mensagens"), 18.0, MUTED);
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
                                if let Some(status) = contact_status(c, contact, localization) {
                                    text(details, &art.regular, status, 18.0, MUTED);
                                }
                            });
                        });
                    });
            }
        }
        PhonePage::ContactThread(contact) => {
            let message = localization.text(match contact {
                Contact::Marina => "marina_message",
                Contact::Rafa => "rafa_message",
            });
            parent.spawn(ui::row()).with_children(|row| {
                ui::picture(row, art, tile(contact), 48.0, 48.0);
                text(row, &assets.font, contact.label(), 28.0, GREEN);
            });
            parent
                .spawn((card(), BackgroundColor(LIGHT)))
                .with_children(|p| {
                    text(p, &art.regular, message, 21.0, INK);
                });
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
        }
        PhonePage::Support => {
            text(parent, &assets.font, "Respiro", 30.0, GREEN);
            text(
                parent,
                &art.regular,
                localization.choose("Fictional care desk · one step at a time", "Apoio fictício · um passo de cada vez"),
                18.0,
                MUTED,
            );
            for (index, (label, command)) in choices.iter().enumerate() {
                let care_action = matches!(
                    command,
                    PhoneCommand::Apply(
                        Action::CareConversation | Action::FinancialPlanning | Action::TherapySession
                    )
                );
                ui::action(
                    parent,
                    &assets.font,
                    phone.page,
                    &format!("{}  {label}", index + 1),
                    *command,
                    if care_action { GREEN } else { LIGHT },
                    if care_action { Color::WHITE } else { INK },
                );
            }
            text(
                parent,
                &art.regular,
                format_text!(
                    localization,
                    "{} care steps · {} planning steps\nBetting {} · information always free",
                    "{} ações de apoio · {} ações de planejamento\nApostas {} · informações gratuitas",
                    c.support.care_actions,
                    c.support.planning_actions,
                    if c.support.app_blocked_today {
                        localization.choose("blocked until morning", "bloqueadas até amanhã")
                    } else {
                        localization.choose("available", "disponíveis")
                    }
                ),
                17.0,
                MUTED,
            );
        }
        PhonePage::DailyCap => {
            text(parent, &assets.font, localization.choose("Daily betting cap", "Limite diário de apostas"), 28.0, GREEN);
            text(
                parent,
                &art.regular,
                format_text!(
                    localization,
                    "Current {} · staked today {}",
                    "Limite atual {} · apostado hoje {}",
                    c.support
                        .daily_cap_cents
                        .map(|amount| localization.reais(amount))
                        .unwrap_or_else(|| localization.choose("No cap", "Sem limite").into()),
                    localization.reais(
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
                    format_text!(
                        localization,
                        "Pending: {} · day {} morning",
                        "Pendente: {} · manhã do dia {}",
                        pending.cents.map(|amount| localization.reais(amount)).unwrap_or_else(|| localization.choose("No cap", "Sem limite").into()),
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
                    localization.choose("Whole reais · Enter selects", "Valores inteiros em reais · Enter para selecionar"),
                    17.0,
                    MUTED,
                );
                ui::action(
                    parent,
                    &assets.font,
                    phone.page,
                    localization.choose("Select amount", "Selecionar valor"),
                    PhoneCommand::ConfirmCap,
                    GREEN,
                    Color::WHITE,
                );
            } else {
                ui::action(
                    parent,
                    &assets.font,
                    phone.page,
                    &format_text!(localization, "{} · edit", "{} · editar", localization.reais(phone.cap_cents)),
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
                localization.choose("R$0 → betting blocked\nLower cap → now · clears pending change\nHigher / removed cap → morning\nSame cap → clears pending change\nPayouts never refill the cap", "R$0 → apostas bloqueadas\nReduzir o limite → efeito imediato, cancela a alteração pendente\nAumentar ou remover o limite → amanhã de manhã\nManter o limite → cancela a alteração pendente\nOs prêmios não repõem o limite"),
                17.0,
                MUTED,
            );
        }
        _ => {}
    }
}

fn contact_status(c: &Campaign, contact: Contact, localization: &Localization) -> Option<String> {
    if c.commitment_due(contact) {
        if c.uses_phone_for(contact) {
            Some(localization.choose("Calling now…", "Ligando agora…").into())
        } else {
            Some(localization.choose("Waiting at the front door.", "Esperando na porta de entrada.").into())
        }
    } else if let Some(plan) = c.commitments.iter().find(|plan| {
        plan.contact == contact && plan.status == CommitmentStatus::Scheduled
    }) {
        Some(format!("{} {} · {}", localization.text("day"), plan.day, localization.period(plan.period.label())))
    } else if c.commitments.iter().any(|plan| {
        plan.contact == contact && plan.status == CommitmentStatus::Missed
    }) {
        Some(localization.choose("We missed our plan.", "Perdemos o horário combinado.").into())
    } else {
        None
    }
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
            actions(&phone, &c, &Localization::ENGLISH)[0].1,
            PhoneCommand::Apply(Action::Call(Contact::Rafa))
        );
        phone.page = PhonePage::DailyCap;
        phone.cap_entry = Some("12".into());
        assert!(actions(&phone, &c, &Localization::ENGLISH).is_empty());
        assert_eq!(contact_status(&c, Contact::Rafa, &Localization::ENGLISH).as_deref(), Some("Day 4 · Night"));
    }
}
