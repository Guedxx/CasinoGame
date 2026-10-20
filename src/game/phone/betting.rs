//! Casino presentation; every wager still goes through the campaign's atomic action.
use super::*;

const GREEN: Color = Color::srgb(0.70, 0.91, 0.30);
const DARK: Color = Color::srgb(0.035, 0.10, 0.075);
const CARD: Color = Color::srgb(0.075, 0.17, 0.13);
const GOLD: Color = Color::srgb(0.96, 0.76, 0.35);
const WHEEL_RED: [u8; 4] = [209, 51, 61, 255];
const WHEEL_BLACK: [u8; 4] = [5, 6, 6, 255];

#[derive(Component)]
pub(super) struct Wheel;
#[derive(Component)]
pub(super) struct Reel(usize);
#[derive(Component)]
pub(super) struct FootballBall;

fn selections(page: PhonePage) -> &'static [(BetChoice, &'static str)] {
    match page {
        PhonePage::Roulette => &[
            (BetChoice::RouletteRed, "Red · 1.8×"),
            (BetChoice::RouletteBlack, "Black · 1.8×"),
        ],
        PhonePage::Football => &[
            (BetChoice::FootballHome, "Home\n1.8×"),
            (BetChoice::FootballDraw, "Draw\n3.6×"),
            (BetChoice::FootballAway, "Away\n3.6×"),
        ],
        _ => &[],
    }
}

fn chosen(phone: &Phone) -> Option<BetChoice> {
    if phone.page == PhonePage::Slots {
        Some(BetChoice::Slots)
    } else {
        phone
            .selected_bet
            .filter(|choice| page_for_choice(*choice) == phone.page)
    }
}

fn wager(phone: &Phone) -> Option<(String, PhoneCommand)> {
    chosen(phone).map(|choice| {
        (
            format!(
                "{} {}",
                if choice == BetChoice::Slots {
                    "Spin"
                } else {
                    "Bet"
                },
                reais(phone.stake_cents)
            ),
            PhoneCommand::Apply(Action::Bet {
                choice,
                stake_cents: phone.stake_cents,
            }),
        )
    })
}

pub(super) fn actions(phone: &Phone, c: &Campaign) -> Vec<(String, PhoneCommand)> {
    let mut result = Vec::new();
    if !phone.bet_info_open {
        if phone.page == PhonePage::Betting {
            for (page, label) in [
                (PhonePage::Slots, "Slots"),
                (PhonePage::Roulette, "Roulette"),
                (PhonePage::Football, "Football"),
            ] {
                result.push((label.into(), PhoneCommand::Page(page)));
            }
        } else if phone.page.is_game() {
            if phone.animation_left > 0.0 {
                result.push(("Show result".into(), PhoneCommand::Skip));
            } else if phone.stake_entry.is_none() {
                result.extend(
                    selections(phone.page)
                        .iter()
                        .map(|&(choice, label)| (label.into(), PhoneCommand::SelectBet(choice))),
                );
                if !c.support.app_blocked_today {
                    result.extend(wager(phone));
                }
            }
            if phone.stake_entry.is_some() {
                result.push(("Set stake".into(), PhoneCommand::ConfirmStake));
            } else {
                result.push(("Stake".into(), PhoneCommand::EditStake));
                for (label, delta) in [("−10", -1000), ("−1", -100), ("+1", 100), ("+10", 1000)]
                {
                    result.push((label.into(), PhoneCommand::Stake(delta)));
                }
            }
        }
    }
    for (page, label) in [
        (PhonePage::Betting, "Casino"),
        (PhonePage::Football, "Sport"),
        (PhonePage::BettingHistory, "My bets"),
    ] {
        result.push((label.into(), PhoneCommand::Page(page)));
    }
    result.push((
        if phone.bet_info_open {
            "Close rules"
        } else {
            "Rules"
        }
        .into(),
        PhoneCommand::ToggleBetInfo,
    ));
    result.push((
        "Open Support".into(),
        PhoneCommand::Page(PhonePage::Support),
    ));
    result
}

fn signed(cents: i64) -> String {
    format!("{}{}", if cents >= 0 { "+" } else { "" }, reais(cents))
}

fn action(
    parent: &mut ChildSpawnerCommands,
    art: &ui::Art,
    phone: &Phone,
    label: &str,
    command: PhoneCommand,
    selected: bool,
) -> Entity {
    let label = primary_key(phone, command)
        .map(|key| format!("{key}  {label}"))
        .unwrap_or_else(|| label.into());
    ui::action(
        parent,
        &art.regular,
        phone.page,
        &label,
        command,
        if selected { GREEN } else { CARD },
        if selected { DARK } else { INK },
    )
}

fn primary_key(phone: &Phone, command: PhoneCommand) -> Option<usize> {
    if phone.bet_info_open || phone.stake_entry.is_some() {
        return None;
    }
    match command {
        PhoneCommand::Page(target) if phone.page == PhonePage::Betting => {
            [PhonePage::Slots, PhonePage::Roulette, PhonePage::Football]
                .iter()
                .position(|page| *page == target)
                .map(|index| index + 1)
        }
        PhoneCommand::Skip if phone.animation_left > 0.0 => Some(1),
        PhoneCommand::SelectBet(choice) if phone.animation_left <= 0.0 => selections(phone.page)
            .iter()
            .position(|(allowed, _)| *allowed == choice)
            .map(|index| index + 1),
        PhoneCommand::Apply(Action::Bet { .. }) if phone.animation_left <= 0.0 => {
            Some(selections(phone.page).len() + 1)
        }
        _ => None,
    }
}

fn tile(
    parent: &mut ChildSpawnerCommands,
    art: &ui::Art,
    phone: &Phone,
    target: PhonePage,
    image: ui::Tile,
    label: &str,
    hero: bool,
) {
    let entity = action(parent, art, phone, label, PhoneCommand::Page(target), false);
    parent
        .commands()
        .entity(entity)
        .insert(Node {
            width: percent(100),
            min_height: px(if hero { 120 } else { 76 }),
            padding: UiRect::all(px(10)),
            border_radius: BorderRadius::all(px(12)),
            justify_content: JustifyContent::SpaceBetween,
            align_items: AlignItems::Center,
            flex_shrink: 0.0,
            ..default()
        })
        .with_children(|card| {
            let size = if hero { 100.0 } else { 56.0 };
            ui::picture(card, art, image, size, size);
        });
}

pub(super) fn render(
    parent: &mut ChildSpawnerCommands,
    assets: &PhoneAssets,
    art: &ui::Art,
    phone: &Phone,
    c: &Campaign,
) {
    parent
        .spawn(Node {
            justify_content: JustifyContent::SpaceBetween,
            ..ui::row()
        })
        .with_children(|header| {
            text(header, &assets.font, "VIRA", 24.0, GREEN);
            text(header, &assets.font, reais(c.balance_cents), 24.0, INK);
        });
    parent.spawn(ui::row()).with_children(|tabs| {
        for (page, label) in [
            (PhonePage::Betting, "Casino"),
            (PhonePage::Football, "Sport"),
            (PhonePage::BettingHistory, "My bets"),
        ] {
            let selected = phone.page == page
                || (page == PhonePage::Betting
                    && matches!(phone.page, PhonePage::Roulette | PhonePage::Slots));
            tabs.spawn(Node {
                flex_grow: 1.0,
                flex_basis: px(0),
                min_width: px(0),
                ..default()
            })
            .with_children(|tab| {
                action(tab, art, phone, label, PhoneCommand::Page(page), selected);
            });
        }
    });
    if phone.bet_info_open {
        rules(parent, art, phone);
    } else {
        match phone.page {
            PhonePage::Betting => {
                tile(
                    parent,
                    art,
                    phone,
                    PhonePage::Slots,
                    ui::Tile::Slots,
                    "Slots\nThree sevens · 3×",
                    true,
                );
                tile(
                    parent,
                    art,
                    phone,
                    PhonePage::Roulette,
                    ui::Tile::Roulette,
                    "Roulette\nRed or black · 1.8×",
                    false,
                );
                tile(
                    parent,
                    art,
                    phone,
                    PhonePage::Football,
                    ui::Tile::Football,
                    "Football\nHome · Draw · Away",
                    false,
                );
                if let Ok(day) = c.daily_finances(c.clock.day) {
                    text(
                        parent,
                        &art.regular,
                        format!("Today {}", signed(day.payouts_cents - day.gross_bets_cents)),
                        18.0,
                        MUTED,
                    );
                }
            }
            PhonePage::BettingHistory => history(parent, art, c),
            _ => game(parent, assets, art, phone, c),
        }
    }
    parent.spawn(ui::row()).with_children(|footer| {
        for (label, command) in [
            (
                if phone.bet_info_open {
                    "Close rules"
                } else {
                    "Rules"
                },
                PhoneCommand::ToggleBetInfo,
            ),
            ("Open Support", PhoneCommand::Page(PhonePage::Support)),
        ] {
            footer
                .spawn(Node {
                    flex_grow: 1.0,
                    flex_basis: px(0),
                    min_width: px(0),
                    ..default()
                })
                .with_children(|button| {
                    action(button, art, phone, label, command, false);
                });
        }
    });
}

fn rules(parent: &mut ChildSpawnerCommands, art: &ui::Art, phone: &Phone) {
    let odds = match phone.page {
        PhonePage::Roulette => "Red 50% · Black 50%\nWinning return 1.8×",
        PhonePage::Slots => "Win 30% · Loss 70%\nWinning return 3×",
        PhonePage::Football => "Home 50% · 1.8×\nDraw 25% · 3.6×\nAway 25% · 3.6×",
        _ => {
            "Roulette: 50% each · 1.8×\nSlots: win 30% · 3×\nFootball: home 50% · 1.8×\nDraw / away: 25% each · 3.6×"
        }
    };
    text(parent, &art.regular, odds, 18.0, INK);
    text(
        parent,
        &art.regular,
        "Returns include your stake.\nAverage return: 90%. No guaranteed win.\n\nWhole reais · minimum R$1 · no borrowing.\n1 PA buys 3 rounds across all games.\nLeaving this app forfeits unused rounds.\n\nFictional games. Campaign money only.",
        18.0,
        MUTED,
    );
}

fn history(parent: &mut ChildSpawnerCommands, art: &ui::Art, c: &Campaign) {
    if c.betting.history.is_empty() {
        text(parent, &art.regular, "No bets yet.", 18.0, MUTED);
    }
    for round in c.betting.history.iter().rev() {
        parent
            .spawn((
                BackgroundColor(CARD),
                Node {
                    padding: UiRect::all(px(10)),
                    border_radius: BorderRadius::all(px(8)),
                    ..ui::column()
                },
            ))
            .with_children(|row| {
                text(
                    row,
                    &art.regular,
                    format!(
                        "{} · Day {} / {}",
                        choice_label(round.choice),
                        round.clock.day,
                        round.clock.period().label()
                    ),
                    18.0,
                    INK,
                );
                text(
                    row,
                    &art.regular,
                    format!(
                        "{} → {}   {}",
                        reais(round.stake_cents),
                        reais(round.total_return_cents),
                        signed(round.net_cents)
                    ),
                    18.0,
                    if round.net_cents > 0 { GREEN } else { MUTED },
                );
            });
    }
}

fn choice_label(choice: BetChoice) -> &'static str {
    match choice {
        BetChoice::RouletteRed => "Roulette · Red",
        BetChoice::RouletteBlack => "Roulette · Black",
        BetChoice::Slots => "Slots",
        BetChoice::FootballHome => "Football · Home",
        BetChoice::FootballDraw => "Football · Draw",
        BetChoice::FootballAway => "Football · Away",
    }
}

fn result_label(outcome: BetOutcome) -> &'static str {
    match outcome {
        BetOutcome::RouletteRed => "Red",
        BetOutcome::RouletteBlack => "Black",
        BetOutcome::SlotsWin => "Three sevens",
        BetOutcome::SlotsLoss => "No match",
        BetOutcome::FootballHome => "Home wins",
        BetOutcome::FootballDraw => "Draw",
        BetOutcome::FootballAway => "Away wins",
    }
}

fn settled(phone: &Phone) -> Option<&BetRound> {
    phone
        .settled_round
        .as_ref()
        .filter(|round| page_for_choice(round.choice) == phone.page)
}

fn game(
    parent: &mut ChildSpawnerCommands,
    assets: &PhoneAssets,
    art: &ui::Art,
    phone: &Phone,
    c: &Campaign,
) {
    match phone.page {
        PhonePage::Roulette => wheel(parent, assets, art, phone),
        PhonePage::Slots => reels(parent, art, phone),
        PhonePage::Football => pitch(parent, art, phone),
        _ => return,
    }
    if phone.animation_left > 0.0 {
        action(parent, art, phone, "Show result", PhoneCommand::Skip, false);
    } else if let Some(round) = settled(phone) {
        text(
            parent,
            &art.regular,
            format!(
                "{} · Return {} · {}",
                result_label(round.outcome),
                reais(round.total_return_cents),
                signed(round.net_cents)
            ),
            18.0,
            if round.net_cents > 0 { GOLD } else { INK },
        );
    }
    if phone.animation_left <= 0.0 && phone.stake_entry.is_none() {
        parent.spawn(ui::row()).with_children(|choices| {
            for &(choice, label) in selections(phone.page) {
                choices
                    .spawn(Node {
                        flex_grow: 1.0,
                        flex_basis: px(0),
                        min_width: px(0),
                        ..default()
                    })
                    .with_children(|button| {
                        action(
                            button,
                            art,
                            phone,
                            label,
                            PhoneCommand::SelectBet(choice),
                            phone.selected_bet == Some(choice),
                        );
                    });
            }
        });
    }
    if let Some(entry) = &phone.stake_entry {
        text(parent, &assets.font, format!("R${entry}_"), 24.0, INK);
        action(
            parent,
            art,
            phone,
            "Set stake",
            PhoneCommand::ConfirmStake,
            true,
        );
        text(
            parent,
            &art.regular,
            "Whole reais · Enter to set",
            18.0,
            MUTED,
        );
    } else {
        let amount = reais(phone.stake_cents);
        parent
            .spawn(Node {
                column_gap: px(6),
                ..ui::row()
            })
            .with_children(|steps| {
                for (label, command) in [
                    ("−10", PhoneCommand::Stake(-1000)),
                    ("−1", PhoneCommand::Stake(-100)),
                    (amount.as_str(), PhoneCommand::EditStake),
                    ("+1", PhoneCommand::Stake(100)),
                    ("+10", PhoneCommand::Stake(1000)),
                ] {
                    let editing = command == PhoneCommand::EditStake;
                    steps
                        .spawn(Node {
                            width: if editing { Val::Auto } else { px(48) },
                            flex_grow: if editing { 1.0 } else { 0.0 },
                            flex_basis: if editing { px(0) } else { Val::Auto },
                            flex_shrink: 0.0,
                            min_width: px(0),
                            ..default()
                        })
                        .with_children(|button| {
                            let entity = action(button, art, phone, label, command, false);
                            button.commands().entity(entity).insert(Node {
                                width: percent(100),
                                min_height: px(44),
                                padding: UiRect::axes(px(4), px(8)),
                                border_radius: BorderRadius::all(px(10)),
                                justify_content: JustifyContent::Center,
                                align_items: AlignItems::Center,
                                ..default()
                            });
                        });
                }
            });
        if phone.animation_left <= 0.0 && !c.support.app_blocked_today {
            if let Some((label, command)) = wager(phone) {
                action(parent, art, phone, &label, command, true);
            } else {
                text(parent, &art.regular, "Choose a side", 18.0, MUTED);
            }
        }
    }
    let left = c
        .betting
        .session
        .as_ref()
        .map_or(0, |session| session.paid_rounds_remaining);
    parent.spawn(ui::row()).with_children(|rounds| {
        for index in 0..3 {
            rounds.spawn((
                BackgroundColor(if index < left { GREEN } else { CARD }),
                BorderColor::all(MUTED),
                Node {
                    width: px(12),
                    height: px(12),
                    border: UiRect::all(px(1)),
                    border_radius: BorderRadius::MAX,
                    ..default()
                },
                Pickable::IGNORE,
            ));
        }
        text(
            rounds,
            &art.regular,
            format!("Next: {} PA", u8::from(left == 0)),
            18.0,
            MUTED,
        );
    });
    if c.support.app_blocked_today {
        text(parent, &art.regular, "Blocked until morning", 18.0, GOLD);
    } else if let Some(cap) = c.support.daily_cap_cents {
        if let Ok(day) = c.daily_finances(c.clock.day) {
            text(
                parent,
                &art.regular,
                format!(
                    "Daily cap left {}",
                    reais((cap - day.gross_bets_cents).max(0))
                ),
                18.0,
                MUTED,
            );
        }
    }
}

pub(super) fn wheel_image() -> Image {
    let mut data = vec![0; 64 * 64 * 4];
    for y in 0..64_i32 {
        for x in 0..64_i32 {
            let (dx, dy) = (2 * x - 63, 2 * y - 63);
            if dx * dx + dy * dy <= 64 * 64 {
                let start = ((y * 64 + x) * 4) as usize;
                data[start..start + 4].copy_from_slice(if x < 32 {
                    &WHEEL_RED
                } else {
                    &WHEEL_BLACK
                });
            }
        }
    }
    let mut image = Image::new(
        Extent3d {
            width: 64,
            height: 64,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    );
    image.sampler = ImageSampler::nearest();
    image
}

fn wheel(parent: &mut ChildSpawnerCommands, assets: &PhoneAssets, art: &ui::Art, phone: &Phone) {
    parent
        .spawn(Node {
            width: percent(100),
            height: px(144),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            flex_shrink: 0.0,
            ..default()
        })
        .with_children(|area| {
            area.spawn((
                Wheel,
                ImageNode::new(art.wheel.clone()),
                UiTransform::from_rotation(Rot2::radians(wheel_angle(phone))),
                Node {
                    width: px(128),
                    height: px(128),
                    ..default()
                },
                Pickable::IGNORE,
            ));
            area.spawn((
                BackgroundColor(GOLD),
                Node {
                    position_type: PositionType::Absolute,
                    width: px(26),
                    height: px(26),
                    border_radius: BorderRadius::MAX,
                    ..default()
                },
                Pickable::IGNORE,
            ));
            area.spawn(Node {
                position_type: PositionType::Absolute,
                top: px(0),
                ..default()
            })
            .with_children(|pointer| text(pointer, &assets.font, "▼", 24.0, GOLD));
        });
}

fn reels(parent: &mut ChildSpawnerCommands, art: &ui::Art, phone: &Phone) {
    parent
        .spawn((
            BackgroundColor(GOLD),
            Node {
                height: px(132),
                padding: UiRect::all(px(10)),
                border_radius: BorderRadius::all(px(12)),
                justify_content: JustifyContent::Center,
                ..ui::row()
            },
        ))
        .with_children(|machine| {
            for index in 0..3 {
                machine
                    .spawn((
                        BackgroundColor(Color::srgb(0.96, 0.93, 0.80)),
                        Node {
                            flex_grow: 1.0,
                            flex_basis: px(0),
                            height: percent(100),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            border_radius: BorderRadius::all(px(6)),
                            ..default()
                        },
                    ))
                    .with_children(|window| {
                        window.spawn((
                            Reel(index),
                            art.image(reel_tile(
                                settled(phone).map(|r| r.outcome),
                                phone.animation_left,
                                index,
                            )),
                            Node {
                                width: px(76),
                                height: px(76),
                                ..default()
                            },
                            Pickable::IGNORE,
                        ));
                    });
            }
        });
}

fn pitch(parent: &mut ChildSpawnerCommands, art: &ui::Art, phone: &Phone) {
    parent
        .spawn(Node {
            justify_content: JustifyContent::SpaceBetween,
            ..ui::row()
        })
        .with_children(|teams| {
            text(teams, &art.regular, "HOME", 18.0, INK);
            text(teams, &art.regular, "AWAY", 18.0, INK);
        });
    parent
        .spawn((
            BackgroundColor(Color::srgb(0.08, 0.30, 0.17)),
            BorderColor::all(Color::srgb(0.62, 0.78, 0.61)),
            Node {
                width: percent(100),
                height: px(122),
                border: UiRect::all(px(2)),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                flex_shrink: 0.0,
                ..default()
            },
        ))
        .with_children(|field| {
            field.spawn((
                BackgroundColor(Color::srgb(0.62, 0.78, 0.61)),
                Node {
                    position_type: PositionType::Absolute,
                    width: px(2),
                    height: percent(100),
                    ..default()
                },
                Pickable::IGNORE,
            ));
            field.spawn((
                BorderColor::all(Color::srgb(0.62, 0.78, 0.61)),
                Node {
                    width: px(58),
                    height: px(58),
                    border: UiRect::all(px(2)),
                    border_radius: BorderRadius::MAX,
                    ..default()
                },
                Pickable::IGNORE,
            ));
            field.spawn((
                FootballBall,
                BackgroundColor(INK),
                UiTransform::from_translation(Val2::px(ball_x(phone), 0.0)),
                Node {
                    position_type: PositionType::Absolute,
                    width: px(15),
                    height: px(15),
                    border_radius: BorderRadius::MAX,
                    ..default()
                },
                Pickable::IGNORE,
            ));
        });
}

fn wheel_angle(phone: &Phone) -> f32 {
    let Some(round) = settled(phone) else {
        return std::f32::consts::FRAC_PI_2;
    };
    let target = if round.outcome == BetOutcome::RouletteRed {
        1.0
    } else {
        -1.0
    } * std::f32::consts::FRAC_PI_2;
    let left = (phone.animation_left * 10.0).ceil() / 10.0;
    let progress = 1.0 - (left / 1.6).clamp(0.0, 1.0);
    (1.0 - (1.0 - progress).powi(3)) * (std::f32::consts::TAU * 3.0 + target)
}

fn reel_tile(outcome: Option<BetOutcome>, left: f32, index: usize) -> ui::Tile {
    let symbols = [ui::Tile::Seven, ui::Tile::Bar, ui::Tile::Bell];
    if left > [0.8, 0.4, 0.0][index] {
        symbols[((left * 10.0).ceil() as usize + index) % 3]
    } else if outcome == Some(BetOutcome::SlotsWin) {
        ui::Tile::Seven
    } else {
        [ui::Tile::Bar, ui::Tile::Seven, ui::Tile::Bell][index]
    }
}

fn ball_x(phone: &Phone) -> f32 {
    if phone.animation_left > 0.0 {
        ((phone.animation_left * 10.0).ceil() * 0.8).sin() * 80.0
    } else {
        match settled(phone).map(|round| round.outcome) {
            Some(BetOutcome::FootballHome) => -100.0,
            Some(BetOutcome::FootballAway) => 100.0,
            _ => 0.0,
        }
    }
}

pub(super) fn animate(
    phone: Res<Phone>,
    art: Res<ui::Art>,
    mut wheels: Query<&mut UiTransform, (With<Wheel>, Without<FootballBall>)>,
    mut reels: Query<(&Reel, &mut ImageNode)>,
    mut balls: Query<&mut UiTransform, (With<FootballBall>, Without<Wheel>)>,
) {
    if phone.animation_left <= 0.0 {
        return;
    }
    for mut transform in &mut wheels {
        let rotation = Rot2::radians(wheel_angle(&phone));
        if transform.rotation != rotation {
            transform.rotation = rotation;
        }
    }
    for (reel, mut image) in &mut reels {
        let rect = art
            .image(reel_tile(
                settled(&phone).map(|r| r.outcome),
                phone.animation_left,
                reel.0,
            ))
            .rect;
        if image.rect != rect {
            image.rect = rect;
        }
    }
    for mut transform in &mut balls {
        let translation = Val2::px(ball_x(&phone), 0.0);
        if transform.translation != translation {
            transform.translation = translation;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wheel_texture_has_transparent_corners_and_equal_solid_halves() {
        let image = wheel_image();
        assert_eq!((image.width(), image.height()), (64, 64));
        let data = image.data.unwrap();
        let pixel = |x: usize, y: usize| &data[(y * 64 + x) * 4..(y * 64 + x + 1) * 4];
        for (x, y) in [(0, 0), (63, 0), (0, 63), (63, 63)] {
            assert_eq!(pixel(x, y), [0, 0, 0, 0]);
        }
        assert_eq!(pixel(0, 31), WHEEL_RED);
        assert_eq!(pixel(31, 0), WHEEL_RED);
        assert_eq!(pixel(32, 63), WHEEL_BLACK);
        assert_eq!(pixel(63, 32), WHEEL_BLACK);
        let red = data
            .chunks_exact(4)
            .filter(|pixel| *pixel == WHEEL_RED)
            .count();
        let black = data
            .chunks_exact(4)
            .filter(|pixel| *pixel == WHEEL_BLACK)
            .count();
        assert_eq!(red, black);
        assert!((3000..3300).contains(&(red + black)));
        assert!(
            data.chunks_exact(4)
                .all(|pixel| pixel == WHEEL_RED || pixel == WHEEL_BLACK || pixel == [0, 0, 0, 0])
        );
    }

    #[test]
    fn selections_only_expose_the_current_mode_and_explicit_wager() {
        let c = Campaign::new(42);
        let mut phone = Phone {
            page: PhonePage::Roulette,
            ..default()
        };
        assert!(
            !actions(&phone, &c)
                .iter()
                .any(|(_, command)| matches!(command, PhoneCommand::Apply(_)))
        );
        phone.selected_bet = Some(BetChoice::FootballHome);
        assert!(wager(&phone).is_none());
        phone.selected_bet = Some(BetChoice::RouletteRed);
        assert!(actions(&phone, &c).iter().any(|(_, command)| *command
            == PhoneCommand::Apply(Action::Bet {
                choice: BetChoice::RouletteRed,
                stake_cents: 100,
            })));
        phone.animation_left = 1.6;
        assert!(
            !actions(&phone, &c)
                .iter()
                .any(|(_, command)| matches!(command, PhoneCommand::Apply(_)))
        );
        assert!(
            actions(&phone, &c)
                .iter()
                .any(|(_, command)| *command == PhoneCommand::Skip)
        );
        phone.bet_info_open = true;
        assert!(!actions(&phone, &c).iter().any(|(_, command)| matches!(
            command,
            PhoneCommand::Apply(_) | PhoneCommand::SelectBet(_) | PhoneCommand::Skip
        )));
    }

    #[test]
    fn slots_end_on_the_saved_symbols_and_football_does_not_invent_scores() {
        for index in 0..3 {
            assert!(matches!(
                reel_tile(Some(BetOutcome::SlotsWin), 0.0, index),
                ui::Tile::Seven
            ));
        }
        assert!(matches!(
            reel_tile(Some(BetOutcome::SlotsLoss), 0.0, 0),
            ui::Tile::Bar
        ));
        assert!(matches!(
            reel_tile(Some(BetOutcome::SlotsLoss), 0.0, 1),
            ui::Tile::Seven
        ));
        assert!(matches!(
            reel_tile(Some(BetOutcome::SlotsLoss), 0.0, 2),
            ui::Tile::Bell
        ));
        for outcome in [
            BetOutcome::FootballHome,
            BetOutcome::FootballDraw,
            BetOutcome::FootballAway,
        ] {
            assert!(!result_label(outcome).chars().any(|ch| ch.is_ascii_digit()));
        }
    }

    #[test]
    fn roulette_stops_with_the_saved_color_under_the_top_pointer() {
        let mut c = Campaign::new(42);
        c.apply(Action::Bet {
            choice: BetChoice::RouletteRed,
            stake_cents: 100,
        })
        .unwrap();
        let mut phone = Phone {
            page: PhonePage::Roulette,
            settled_round: c.betting.history.last().cloned(),
            ..default()
        };
        for (outcome, half) in [
            (BetOutcome::RouletteRed, Vec2::NEG_X),
            (BetOutcome::RouletteBlack, Vec2::X),
        ] {
            phone.settled_round.as_mut().unwrap().outcome = outcome;
            let center = Rot2::radians(wheel_angle(&phone)) * half;
            assert!(center.y < -0.999 && center.x.abs() < 0.001);
        }
    }

    #[test]
    fn primary_keyboard_hints_follow_the_dispatch_inventory() {
        let c = Campaign::new(42);
        for page in [
            PhonePage::Betting,
            PhonePage::Roulette,
            PhonePage::Slots,
            PhonePage::Football,
        ] {
            let phone = Phone {
                page,
                selected_bet: selections(page).first().map(|(choice, _)| *choice),
                ..default()
            };
            for (index, (_, command)) in actions(&phone, &c).iter().enumerate() {
                if let Some(key) = primary_key(&phone, *command) {
                    let first = actions(&phone, &c)
                        .iter()
                        .position(|(_, candidate)| candidate == command)
                        .unwrap();
                    assert_eq!(key, first + 1, "{page:?}, command {index}");
                }
            }
        }
    }
}
