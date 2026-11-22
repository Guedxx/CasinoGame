use bevy::{prelude::*, ui::FocusPolicy};

use super::{
    campaign::{Campaign, HUNGER_RESTRICTION, SALE_PRESSURE, STRESS_RESTRICTION},
    campaign_save::ActionFeedback,
    routine::RoutineUi,
};
use crate::localization::Localization;

const IVORY: Color = Color::srgb(0.94, 0.90, 0.79);
const BRASS: Color = Color::srgb(0.72, 0.59, 0.38);
const INK: Color = Color::srgba(0.06, 0.055, 0.045, 0.94);
const GAIN: Color = Color::srgb(0.57, 0.91, 0.54);
const LOSS: Color = Color::srgb(1.0, 0.55, 0.50);
const WARNING: Color = Color::srgb(1.0, 0.84, 0.38);

fn value_color(index: usize, value: u8) -> Color {
    let (severity, warning, limit) = match index {
        0 => (value, HUNGER_RESTRICTION - 20, HUNGER_RESTRICTION),
        1 => (value, 80, 100),
        2 => (value, STRESS_RESTRICTION - 20, STRESS_RESTRICTION),
        3 => (value, SALE_PRESSURE - 20, SALE_PRESSURE),
        _ => (100 - value, 40, 55),
    };
    if severity >= limit {
        LOSS
    } else if severity >= warning {
        WARNING
    } else {
        IVORY
    }
}

#[cfg(test)]
#[test]
fn status_numbers_change_color_at_their_warning_and_critical_thresholds() {
    for (index, ordinary, warned, critical) in [
        (0, 59, 60, 80),
        (1, 79, 80, 100),
        (2, 59, 60, 80),
        (3, 49, 50, 70),
        (4, 61, 60, 45),
    ] {
        assert_eq!(value_color(index, ordinary), IVORY);
        assert_eq!(value_color(index, warned), WARNING);
        assert_eq!(value_color(index, critical), LOSS);
    }
}

#[derive(Component)]
pub(super) struct StatusHud;

#[derive(Resource)]
pub(super) struct HudSkin {
    medallions: Handle<Image>,
    font: Handle<Font>,
    pub(super) dialogue_font: Handle<Font>,
}

impl FromWorld for HudSkin {
    fn from_world(world: &mut World) -> Self {
        let assets = world.resource::<AssetServer>();
        Self {
            medallions: assets.load("ui/hud-medallions.png"),
            font: assets.load("fonts/LiberationSans-Bold.ttf"),
            dialogue_font: assets.load("fonts/LiberationSans-Regular.ttf"),
        }
    }
}

impl HudSkin {
    fn font(&self, size: f32) -> TextFont {
        TextFont::from_font_size(size).with_font(self.font.clone())
    }

    fn badge(&self, index: usize) -> ImageNode {
        // The generated atlas has padding around each 464px medallion region.
        let origin = Vec2::new(
            [56.0, 536.0, 1016.0][index % 3],
            if index < 3 { 24.0 } else { 508.0 },
        );
        ImageNode::new(self.medallions.clone())
            .with_rect(Rect::from_corners(origin, origin + Vec2::splat(464.0)))
    }
}

pub(super) fn spawn(
    commands: &mut Commands,
    campaign: &Campaign,
    skin: &HudSkin,
    localization: &Localization,
    delta: ActionFeedback,
    show_summary: bool,
    attributes_visible: usize,
) {
    let passive = (Pickable::IGNORE, FocusPolicy::Pass);
    commands
        .spawn((
            RoutineUi,
            StatusHud,
            Name::new("Character status"),
            Node {
                position_type: PositionType::Absolute,
                top: px(20),
                right: px(20),
                width: px(258),
                max_width: percent(85),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::End,
                row_gap: px(12),
                ..default()
            },
            ZIndex(if show_summary { 20 } else { 31 }),
            passive,
        ))
        .with_children(|hud| {
            if show_summary || delta.money != 0 {
                hud.spawn((
                    Node {
                        align_items: AlignItems::Center,
                        column_gap: px(7),
                        ..default()
                    },
                    passive,
                ))
                .with_children(|row| {
                    row.spawn((
                        Text::new(localization.reais(campaign.balance_cents)),
                        skin.font(27.0),
                        TextColor(IVORY),
                        TextShadow::default(),
                        passive,
                    ));
                    if delta.money != 0 {
                        let sign = if delta.money > 0 { "+" } else { "" };
                        row.spawn((
                            Text::new(format!("{sign}{}", localization.reais(delta.money))),
                            skin.font(18.0),
                            TextColor(if delta.money > 0 { GAIN } else { LOSS }),
                            passive,
                        ));
                    }
                });
            }
            if show_summary {
                hud.spawn((
                    Node {
                        align_items: AlignItems::Center,
                        column_gap: px(12),
                        ..default()
                    },
                    passive,
                ))
                .with_children(|row| {
                    row.spawn((
                        skin.badge(5),
                        Node {
                            width: px(70),
                            height: px(70),
                            ..default()
                        },
                        passive,
                    ));
                    row.spawn((
                        Text::new(format!(
                            "{} {} / {}\n{}\n{} {}",
                            localization.text("day"),
                            campaign.clock.day,
                            campaign.last_day(),
                            localization.period(campaign.clock.period().label()),
                            campaign.clock.remaining_pa(),
                            if campaign.clock.remaining_pa() == 1 {
                                localization.choose("action left", "ação restante")
                            } else {
                                localization.text("actions_left")
                            }
                        )),
                        skin.font(17.0),
                        TextColor(IVORY),
                        TextShadow::default(),
                        passive,
                    ));
                });
            }
            let values = campaign.attributes.values();
            let labels = ["hunger", "boredom", "stress", "urge", "relationships"];
            for row in [0..3, 3..5] {
                let indices = row.filter(|&index| {
                    index < attributes_visible && (show_summary || delta.attributes[index] != 0)
                });
                if indices.clone().next().is_none() {
                    continue;
                }
                hud.spawn((
                    Node {
                        justify_content: JustifyContent::End,
                        column_gap: px(9),
                        ..default()
                    },
                    passive,
                ))
                .with_children(|row| {
                    for index in indices {
                        let label = localization.text(labels[index]);
                        let value = values[index];
                        let change = delta.attributes[index];
                        row.spawn((
                            Name::new(format!("{label}: {value} / 100")),
                            Node {
                                width: px(80),
                                flex_direction: FlexDirection::Column,
                                align_items: AlignItems::Center,
                                row_gap: px(4),
                                ..default()
                            },
                            passive,
                        ))
                        .with_children(|stat| {
                            stat.spawn((
                                skin.badge(index),
                                Node {
                                    width: px(72),
                                    height: px(72),
                                    justify_content: JustifyContent::Center,
                                    align_items: AlignItems::End,
                                    ..default()
                                },
                                passive,
                            ))
                            .with_children(|badge| {
                                badge
                                    .spawn((
                                        Node {
                                            min_width: px(38),
                                            padding: UiRect::axes(px(6), px(1)),
                                            border: UiRect::all(px(1)),
                                            justify_content: JustifyContent::Center,
                                            align_items: AlignItems::Center,
                                            column_gap: px(4),
                                            ..default()
                                        },
                                        BackgroundColor(INK),
                                        BorderColor::all(BRASS),
                                        passive,
                                    ))
                                    .with_children(|plate| {
                                        plate.spawn((
                                            Text::new(value.to_string()),
                                            skin.font(19.0),
                                            TextColor(value_color(index, value)),
                                            passive,
                                        ));
                                        if change != 0 {
                                            plate.spawn((
                                                Text::new(format!("{change:+}")),
                                                skin.font(13.0),
                                                TextColor(if change > 0 { GAIN } else { LOSS }),
                                                passive,
                                            ));
                                        }
                                    });
                            });
                            stat.spawn((
                                Text::new(label),
                                skin.font(12.0),
                                TextColor(IVORY),
                                TextShadow::default(),
                                passive,
                            ));
                        });
                    }
                });
            }
        });
}

pub(super) fn interaction(commands: &mut Commands, label: &str, skin: &HudSkin, localization: &Localization) {
    commands
        .spawn((
            RoutineUi,
            Name::new("Interaction prompt"),
            Node {
                position_type: PositionType::Absolute,
                top: percent(50),
                margin: UiRect::top(px(28)),
                width: percent(100),
                justify_content: JustifyContent::Center,
                ..default()
            },
            ZIndex(20),
            Pickable::IGNORE,
            FocusPolicy::Pass,
        ))
        .with_children(|center| {
            center
                .spawn((
                    Node {
                        max_width: percent(90),
                        padding: UiRect::axes(px(14), px(9)),
                        column_gap: px(12),
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    BackgroundColor(INK),
                ))
                .with_children(|prompt| {
                    prompt.spawn((
                        Node {
                            padding: UiRect::axes(px(7), px(2)),
                            border: UiRect::all(px(1)),
                            ..default()
                        },
                        BorderColor::all(BRASS),
                        children![(Text::new("E"), skin.font(18.0), TextColor(IVORY))],
                    ));
                    prompt.spawn((
                        Text::new(format!("{} · {label}", localization.choose("Interact", "Interagir"))),
                        skin.font(18.0),
                        TextColor(IVORY),
                    ));
                });
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_medallions_have_alpha_and_all_six_regions_fit() {
        let png = include_bytes!("../../assets/ui/hud-medallions.png");
        assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
        let width = u32::from_be_bytes(png[16..20].try_into().unwrap());
        let height = u32::from_be_bytes(png[20..24].try_into().unwrap());
        assert_eq!(png[25], 6, "HUD artwork needs RGBA transparency");
        let skin = HudSkin {
            medallions: default(),
            font: default(),
            dialogue_font: default(),
        };
        for index in 0..6 {
            let rect = skin.badge(index).rect.unwrap();
            assert!(rect.min.cmpge(Vec2::ZERO).all());
            assert!(rect.max.cmple(Vec2::new(width as f32, height as f32)).all());
            assert_eq!(rect.width(), rect.height());
        }
    }
}
