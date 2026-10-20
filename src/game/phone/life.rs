use super::*;
use ui::Tile;

const DARK: Color = Color::srgb(0.16, 0.14, 0.19);
const GREY: Color = Color::srgb(0.39, 0.37, 0.42);
const RED: Color = Color::srgb(0.82, 0.09, 0.15);
const PURPLE: Color = Color::srgb(0.40, 0.16, 0.64);
const PAPER: Color = Color::srgb(0.96, 0.95, 0.97);

pub(super) fn actions(phone: &Phone, c: &Campaign) -> Vec<(String, PhoneCommand)> {
    match phone.page {
        PhonePage::Food => {
            let mut choices = vec![(
                "Order a meal: R$20".into(),
                PhoneCommand::Apply(Action::OrderFood),
            )];
            if c.require_kitchen().is_ok() {
                choices.extend([
                    (
                        "Groceries: R$40 / 3 portions".into(),
                        PhoneCommand::Apply(Action::Groceries),
                    ),
                    (
                        "Cook and eat: 1 portion".into(),
                        PhoneCommand::Apply(Action::Cook),
                    ),
                ]);
            }
            choices
        }
        PhonePage::Household if items().any(|item| c.can_sell(item).is_ok()) => vec![(
            "Sell an item".into(),
            PhoneCommand::Page(PhonePage::SaleOffers),
        )],
        PhonePage::SaleOffers => items()
            .filter(|&item| c.can_sell(item).is_ok())
            .map(|item| {
                (
                    format!(
                        "Review {} sale: {} / 1 PA",
                        item.label(),
                        reais(item.sale_cents())
                    ),
                    PhoneCommand::Page(PhonePage::ConfirmSale(item)),
                )
            })
            .collect(),
        PhonePage::ConfirmSale(item) => {
            let mut choices = vec![(
                "Cancel / keep at home".into(),
                PhoneCommand::Page(PhonePage::SaleOffers),
            )];
            if c.can_sell(item).is_ok() {
                choices.push((
                    if item == HouseholdItem::Pipoca {
                        "Sell Pipoca / END CAMPAIGN"
                    } else {
                        "Confirm permanent sale"
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

fn items() -> impl Iterator<Item = HouseholdItem> {
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
) {
    if let Some((index, (_, command))) = choices
        .iter()
        .enumerate()
        .find(|(_, (_, candidate))| *candidate == command)
    {
        let label = match command {
            PhoneCommand::Apply(Action::Groceries) => "Buy groceries",
            PhoneCommand::Apply(Action::Cook) => "Cook and eat",
            PhoneCommand::Apply(Action::OrderFood) => "Order meal",
            PhoneCommand::Apply(Action::Sell(HouseholdItem::Pipoca)) => {
                "Sell Pipoca / END CAMPAIGN"
            }
            PhoneCommand::Apply(Action::Sell(_)) => "Sell permanently",
            _ => "Keep at home",
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
) {
    let choices = actions(phone, c);
    match phone.page {
        PhonePage::Food => {
            text(parent, &assets.font, "MeFood", 34., RED);
            parent
                .spawn((card(), BackgroundColor(PAPER)))
                .with_children(|summary| {
                    text(
                        summary,
                        &assets.font,
                        format!(
                            "{}  ·  {} PA left",
                            reais(c.balance_cents),
                            c.clock.remaining_pa()
                        ),
                        21.,
                        DARK,
                    );
                    text(
                        summary,
                        &art.regular,
                        format!(
                            "Hunger {} / 100  ·  {} portions",
                            c.attributes.hunger, c.household.food_portions
                        ),
                        19.,
                        GREY,
                    );
                });
            for (tile, title, copy, command) in [
                (
                    Tile::Meal,
                    "Prepared meal",
                    "R$20 · 1 PA\nAlways available",
                    PhoneCommand::Apply(Action::OrderFood),
                ),
                (
                    Tile::Groceries,
                    "Stock your kitchen",
                    "R$40 · 3 portions · 1 PA",
                    PhoneCommand::Apply(Action::Groceries),
                ),
            ] {
                parent
                    .spawn((card(), BackgroundColor(PAPER)))
                    .with_children(|item| {
                        item.spawn(ui::row()).with_children(|row| {
                            ui::picture(row, art, tile, 64., 64.);
                            row.spawn(Node {
                                flex_grow: 1.,
                                min_width: px(0),
                                width: Val::Auto,
                                ..ui::column()
                            })
                            .with_children(|copy_node| {
                                text(copy_node, &assets.font, title, 22., DARK);
                                text(copy_node, &art.regular, copy, 18., GREY);
                            });
                        });
                        if choices.iter().any(|(_, candidate)| *candidate == command) {
                            action(item, assets, phone, &choices, command, RED);
                        } else {
                            text(
                                item,
                                &art.regular,
                                "Unavailable: fridge, stove and cookware needed",
                                18.,
                                GREY,
                            );
                        }
                    });
            }
            parent
                .spawn((card(), BackgroundColor(PAPER)))
                .with_children(|kitchen| {
                    kitchen
                        .spawn(Node {
                            justify_content: JustifyContent::SpaceBetween,
                            ..ui::row()
                        })
                        .with_children(|row| {
                            text(row, &assets.font, "Cook at home", 22., DARK);
                            text(row, &art.regular, "1 portion · 1 PA", 18., GREY);
                        });
                    if c.require_kitchen().is_ok() {
                        action(
                            kitchen,
                            assets,
                            phone,
                            &choices,
                            PhoneCommand::Apply(Action::Cook),
                            RED,
                        );
                    } else {
                        text(
                            kitchen,
                            &art.regular,
                            "Your kitchen is incomplete. Prepared-food delivery remains available.",
                            19.,
                            GREY,
                        );
                    }
                });
            text(
                parent,
                &art.regular,
                "Cooking and groceries need a fridge, stove and cookware.",
                18.,
                GREY,
            );
        }
        PhonePage::Household | PhonePage::SaleOffers => {
            let history = phone.page == PhonePage::Household;
            text(
                parent,
                &assets.font,
                if history {
                    "Home Market"
                } else {
                    "Sell an item"
                },
                32.,
                PURPLE,
            );
            if history && !choices.is_empty() {
                ui::action(
                    parent,
                    &assets.font,
                    phone.page,
                    "1  Sell an item",
                    PhoneCommand::Page(PhonePage::SaleOffers),
                    PURPLE,
                    Color::WHITE,
                );
            }
            text(
                parent,
                &assets.font,
                format!(
                    "{}  ·  {} PA left",
                    reais(c.balance_cents),
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
                        "No sales yet"
                    } else {
                        "No items available"
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
                                                        "SOLD",
                                                        20.,
                                                        Color::WHITE,
                                                    );
                                                });
                                        }
                                    });
                                text(listing, &assets.font, item.label(), 23., DARK);
                                text(listing, &assets.font, reais(item.sale_cents()), 23., PURPLE);
                                if c.can_sell(item).is_ok() {
                                    text(listing, &art.regular, "At home · 1 PA", 18., GREY);
                                    let command = PhoneCommand::Page(PhonePage::ConfirmSale(item));
                                    if let Some(index) = choices
                                        .iter()
                                        .position(|(_, candidate)| *candidate == command)
                                    {
                                        ui::action(
                                            listing,
                                            &assets.font,
                                            phone.page,
                                            &format!("{}  Review sale", index + 1),
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
            text(parent, &assets.font, "Review permanent sale", 29., PURPLE);
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
                            text(summary, &assets.font, item.label(), 26., DARK);
                            text(
                                summary,
                                &assets.font,
                                format!("Receive {}", reais(item.sale_cents())),
                                23.,
                                PURPLE,
                            );
                            text(
                                summary,
                                &art.regular,
                                format!("Costs 1 PA · {} PA left", c.clock.remaining_pa()),
                                19.,
                                GREY,
                            );
                        });
                    });
                });
            text(parent, &assets.font, "This cannot be undone", 25., RED);
            if item == HouseholdItem::Fridge {
                text(
                    parent,
                    &assets.font,
                    format!(
                        "ALL {} stored food portions will be lost.",
                        c.household.food_portions
                    ),
                    21.,
                    RED,
                );
                text(
                    parent,
                    &art.regular,
                    "The fridge is permanently removed. Cooking and grocery storage become unavailable; prepared-food delivery remains available.",
                    20.,
                    DARK,
                );
            } else {
                text(parent, &art.regular, item.sale_loss(), 20., DARK);
            }
            if let Err(reason) = c.can_sell(item) {
                text(parent, &art.regular, reason, 20., RED);
            }
            action(
                parent,
                assets,
                phone,
                &choices,
                PhoneCommand::Page(PhonePage::SaleOffers),
                PURPLE,
            );
            action(
                parent,
                assets,
                phone,
                &choices,
                PhoneCommand::Apply(Action::Sell(item)),
                RED,
            );
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn food_actions_keep_delivery_after_kitchen_sale() {
        let mut c = Campaign::new(5);
        c.household.sold.push(HouseholdItem::Fridge);
        let phone = Phone {
            page: PhonePage::Food,
            ..default()
        };
        assert_eq!(
            actions(&phone, &c),
            vec![(
                "Order a meal: R$20".into(),
                PhoneCommand::Apply(Action::OrderFood)
            )]
        );
    }

    #[test]
    fn sale_registry_respects_offer_gates_and_keeps_cancel() {
        let mut c = Campaign::new(5);
        let mut phone = Phone {
            page: PhonePage::Household,
            ..default()
        };
        assert!(actions(&phone, &c).is_empty());
        c.attributes.stress = 70;
        assert_eq!(
            actions(&phone, &c),
            vec![(
                "Sell an item".into(),
                PhoneCommand::Page(PhonePage::SaleOffers)
            )]
        );
        phone.page = PhonePage::SaleOffers;
        assert_eq!(actions(&phone, &c).len(), 6);
        phone.page = PhonePage::ConfirmSale(HouseholdItem::Pipoca);
        assert_eq!(actions(&phone, &c).len(), 1);
        c.household
            .sold
            .extend(super::super::super::household::HOUSEHOLD_ITEMS);
        assert_eq!(actions(&phone, &c).len(), 2);
        assert_eq!(
            actions(&phone, &c)[1].1,
            PhoneCommand::Apply(Action::Sell(HouseholdItem::Pipoca))
        );
    }
}
