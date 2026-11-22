use super::*;
use ui::Tile;

const DARK: Color = Color::srgb(0.16, 0.14, 0.19);
const GREY: Color = Color::srgb(0.39, 0.37, 0.42);
const RED: Color = Color::srgb(0.82, 0.09, 0.15);
const PURPLE: Color = Color::srgb(0.40, 0.16, 0.64);
const PAPER: Color = Color::srgb(0.96, 0.95, 0.97);

pub(super) fn actions(phone: &Phone, c: &Campaign, localization: &Localization) -> Vec<(String, PhoneCommand)> {
    match phone.page {
        PhonePage::Household if items().any(|item| c.can_sell(item).is_ok()) => vec![(
            localization.choose("Sell an item", "Vender um item").into(),
            PhoneCommand::Page(PhonePage::SaleOffers),
        )],
        PhonePage::SaleOffers => items()
            .filter(|&item| c.can_sell(item).is_ok())
            .map(|item| {
                (
                    format_text!(
                        localization,
                        "Review {} sale: {} / 1 PA",
                        "Revisar venda ({}) · {} / 1 PA",
                        localization.item(item),
                        localization.reais(item.sale_cents())
                    ),
                    PhoneCommand::Page(PhonePage::ConfirmSale(item)),
                )
            })
            .collect(),
        PhonePage::ConfirmSale(item) => {
            let mut choices = vec![(
                localization.choose("Cancel / keep at home", "Cancelar e manter em casa").into(),
                PhoneCommand::Page(PhonePage::SaleOffers),
            )];
            if c.can_sell(item).is_ok() {
                choices.push((
                    if item == HouseholdItem::Pipoca {
                        localization.choose("Sell Pipoca / END CAMPAIGN", "Vender o Pipoca e ENCERRAR A CAMPANHA")
                    } else {
                        localization.choose("Confirm permanent sale", "Confirmar venda definitiva")
                    }
                    .into(),
                    PhoneCommand::Apply(Action::Sell(item)),
                ));
            }
            choices
        }
        _ => vec![],
    }
}

pub(super) fn items() -> impl Iterator<Item = HouseholdItem> {
    super::super::household::HOUSEHOLD_ITEMS
        .into_iter()
        .chain([HouseholdItem::Pipoca])
}

fn tile(item: HouseholdItem) -> Tile {
    match item {
        HouseholdItem::Tv => Tile::Tv,
        HouseholdItem::Fridge => Tile::Fridge,
        HouseholdItem::Stove => Tile::Stove,
        HouseholdItem::Cookware => Tile::Cookware,
        HouseholdItem::Couch => Tile::Couch,
        HouseholdItem::Books => Tile::Books,
        HouseholdItem::Pipoca => Tile::Pipoca,
    }
}

fn action(
    parent: &mut ChildSpawnerCommands,
    assets: &PhoneAssets,
    phone: &Phone,
    choices: &[(String, PhoneCommand)],
    command: PhoneCommand,
    fill: Color,
    localization: &Localization,
) {
    if let Some((index, (_, command))) = choices
        .iter()
        .enumerate()
        .find(|(_, (_, candidate))| *candidate == command)
    {
        let label = match command {
            PhoneCommand::Apply(Action::Sell(HouseholdItem::Pipoca)) => {
                localization.choose("Sell Pipoca / END CAMPAIGN", "Vender o Pipoca e ENCERRAR A CAMPANHA")
            }
            PhoneCommand::Apply(Action::Sell(_)) => localization.choose("Sell permanently", "Vender definitivamente"),
            _ => localization.choose("Keep at home", "Manter em casa"),
        };
        ui::action(
            parent,
            &assets.font,
            phone.page,
            &format!("{}  {label}", index + 1),
            *command,
            fill,
            Color::WHITE,
        );
    }
}

fn card() -> Node {
    Node {
        padding: UiRect::all(px(8)),
        row_gap: px(6),
        border_radius: BorderRadius::all(px(12)),
        ..ui::column()
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
        PhonePage::Household | PhonePage::SaleOffers => {
            let history = phone.page == PhonePage::Household;
            text(
                parent,
                &assets.font,
                if history {
                    localization.choose("Household sales", "Vendas de itens da casa")
                } else {
                    localization.choose("Sell an item", "Vender um item")
                },
                32.,
                PURPLE,
            );
            if history && !choices.is_empty() {
                ui::action(
                    parent,
                    &assets.font,
                    phone.page,
                    localization.choose("1  Sell an item", "1  Vender um item"),
                    PhoneCommand::Page(PhonePage::SaleOffers),
                    PURPLE,
                    Color::WHITE,
                );
            }
            text(
                parent,
                &assets.font,
                format_text!(
                    localization,
                    "{}  ·  {} PA left",
                    "{}  ·  {} PA restantes",
                    localization.reais(c.balance_cents),
                    c.clock.remaining_pa()
                ),
                21.,
                DARK,
            );
            let all: Vec<_> = items()
                .filter(|&item| {
                    if history {
                        !c.household.owns(item)
                    } else {
                        c.can_sell(item).is_ok()
                    }
                })
                .collect();
            if all.is_empty() {
                text(
                    parent,
                    &art.regular,
                    if history {
                        localization.choose("No sales yet", "Nenhuma venda registrada")
                    } else {
                        localization.choose("No items available", "Nenhum item disponível")
                    },
                    21.,
                    GREY,
                );
            }
            for pair in all.chunks(2) {
                parent
                    .spawn(Node {
                        align_items: AlignItems::Stretch,
                        ..ui::row()
                    })
                    .with_children(|row| {
                        for &item in pair {
                            row.spawn((
                                Node {
                                    width: percent(48.5),
                                    ..card()
                                },
                                BackgroundColor(PAPER),
                            ))
                            .with_children(|listing| {
                                listing
                                    .spawn(Node {
                                        width: percent(100),
                                        height: px(112),
                                        align_items: AlignItems::Center,
                                        justify_content: JustifyContent::Center,
                                        flex_shrink: 0.,
                                        ..default()
                                    })
                                    .with_children(|photo| {
                                        ui::picture(photo, art, tile(item), 96., 96.);
                                        if !c.household.owns(item) {
                                            photo
                                                .spawn((
                                                    Node {
                                                        position_type: PositionType::Absolute,
                                                        top: px(0),
                                                        width: percent(100),
                                                        padding: UiRect::vertical(px(4)),
                                                        justify_content: JustifyContent::Center,
                                                        border_radius: BorderRadius::all(px(6)),
                                                        ..default()
                                                    },
                                                    BackgroundColor(PURPLE),
                                                    Pickable::IGNORE,
                                                ))
                                                .with_children(|badge| {
                                                    text(
                                                        badge,
                                                        &assets.font,
                                                        localization.choose("SOLD", "VENDIDO"),
                                                        20.,
                                                        Color::WHITE,
                                                    );
                                                });
                                        }
                                    });
                                text(listing, &assets.font, localization.item(item), 23., DARK);
                                text(listing, &assets.font, localization.reais(item.sale_cents()), 23., PURPLE);
                                if c.can_sell(item).is_ok() {
                                    text(listing, &art.regular, localization.choose("At home · 1 PA", "Em casa · 1 PA"), 18., GREY);
                                    let command = PhoneCommand::Page(PhonePage::ConfirmSale(item));
                                    if let Some(index) = choices
                                        .iter()
                                        .position(|(_, candidate)| *candidate == command)
                                    {
                                        ui::action(
                                            listing,
                                            &assets.font,
                                            phone.page,
                                            &format_text!(localization, "{}  Review sale", "{}  Revisar venda", index + 1),
                                            command,
                                            PURPLE,
                                            Color::WHITE,
                                        );
                                    }
                                }
                            });
                        }
                    });
            }
        }
        PhonePage::ConfirmSale(item) => {
            text(parent, &assets.font, localization.choose("Review permanent sale", "Revisar venda definitiva"), 29., PURPLE);
            parent
                .spawn((card(), BackgroundColor(PAPER)))
                .with_children(|review| {
                    review.spawn(ui::row()).with_children(|row| {
                        ui::picture(row, art, tile(item), 96., 96.);
                        row.spawn(Node {
                            width: Val::Auto,
                            flex_grow: 1.,
                            min_width: px(0),
                            ..ui::column()
                        })
                        .with_children(|summary| {
                            text(summary, &assets.font, localization.item(item), 26., DARK);
                            text(
                                summary,
                                &assets.font,
                                format_text!(localization, "Receive {}", "Receber {}", localization.reais(item.sale_cents())),
                                23.,
                                PURPLE,
                            );
                            text(
                                summary,
                                &art.regular,
                                format_text!(localization, "Costs 1 PA · {} PA left", "Custo de 1 PA · {} PA restantes", c.clock.remaining_pa()),
                                19.,
                                GREY,
                            );
                        });
                    });
                });
            text(parent, &assets.font, localization.choose("This cannot be undone", "A venda é definitiva"), 25., RED);
            if item == HouseholdItem::Fridge {
                text(
                    parent,
                    &assets.font,
                    format_text!(
                        localization,
                        "ALL {} stored food portions will be lost.",
                        "Você perderá todas as {} porções de comida armazenadas.",
                        c.household.food_portions
                    ),
                    21.,
                    RED,
                );
                text(
                    parent,
                    &art.regular,
                    localization.choose("The fridge is permanently removed. Cooking and grocery storage become unavailable; prepared-food delivery remains available.", "A geladeira sai de casa definitivamente. Você fica sem poder cozinhar ou guardar mantimentos, mas ainda pode pedir comida pronta."),
                    20.,
                    DARK,
                );
            } else {
                text(parent, &art.regular, localization.sale_loss(item), 20., DARK);
            }
            if let Err(reason) = c.can_sell(item) {
                text(parent, &art.regular, localization.action_error(&reason), 20., RED);
            }
            action(
                parent,
                assets,
                phone,
                &choices,
                PhoneCommand::Page(PhonePage::SaleOffers),
                PURPLE,
                localization,
            );
            action(
                parent,
                assets,
                phone,
                &choices,
                PhoneCommand::Apply(Action::Sell(item)),
                RED,
                localization,
            );
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sale_registry_respects_offer_gates_and_keeps_cancel() {
        let mut c = Campaign::new(5);
        let mut phone = Phone {
            page: PhonePage::Household,
            ..default()
        };
        assert!(actions(&phone, &c, &Localization::ENGLISH).is_empty());
        c.attributes.stress = 70;
        assert_eq!(
            actions(&phone, &c, &Localization::ENGLISH),
            vec![(
                "Sell an item".into(),
                PhoneCommand::Page(PhonePage::SaleOffers)
            )]
        );
        phone.page = PhonePage::SaleOffers;
        assert_eq!(actions(&phone, &c, &Localization::ENGLISH).len(), 6);
        phone.page = PhonePage::ConfirmSale(HouseholdItem::Pipoca);
        assert_eq!(actions(&phone, &c, &Localization::ENGLISH).len(), 1);
        c.household
            .sold
            .extend(super::super::super::household::HOUSEHOLD_ITEMS);
        assert_eq!(actions(&phone, &c, &Localization::ENGLISH).len(), 2);
        assert_eq!(
            actions(&phone, &c, &Localization::ENGLISH)[1].1,
            PhoneCommand::Apply(Action::Sell(HouseholdItem::Pipoca))
        );
    }
}
