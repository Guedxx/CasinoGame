use bevy::{prelude::*, ui::FocusPolicy};

use super::{
    campaign::Campaign,
    routine::{RoutineUi, reais},
};
use crate::localization::Localization;

const IVORY: Color = Color::srgb(0.94, 0.90, 0.79);
const BRASS: Color = Color::srgb(0.72, 0.59, 0.38);
const INK: Color = Color::srgba(0.06, 0.055, 0.045, 0.94);

#[derive(Resource)]
pub(super) struct HudSkin {
    medallions: Handle<Image>,
    font: Handle<Font>,
}

impl FromWorld for HudSkin {
    fn from_world(world: &mut World) -> Self {
        let assets = world.resource::<AssetServer>();
        Self {
            medallions: assets.load("ui/hud-medallions.png"),
            font: assets.load("fonts/LiberationSans-Bold.ttf"),
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

pub(super) fn spawn(commands: &mut Commands, campaign: &Campaign, skin: &HudSkin, localization: &Localization) {
    commands
        .spawn((
            RoutineUi,
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
            ZIndex(20),
            Pickable::IGNORE,
            FocusPolicy::Pass,
        ))
        .with_children(|hud| {
            hud.spawn((
                Text::new(reais(campaign.balance_cents)),
                skin.font(27.0),
                TextColor(IVORY),
                TextShadow::default(),
            ));
            hud.spawn(Node {
                align_items: AlignItems::Center,
                column_gap: px(12),
                ..default()
            })
            .with_children(|row| {
                row.spawn((
                    skin.badge(5),
                    Node {
                        width: px(70),
                        height: px(70),
                        ..default()
                    },
                ));
                row.spawn((
                    Text::new(format!(
                        "{} {} / 10\n{}\n{} {}",
                        localization.text("day"),
                        campaign.clock.day,
                        localization.period(campaign.clock.period().label()),
                        campaign.clock.remaining_pa(), localization.text("actions_left")
                    )),
                    skin.font(17.0),
                    TextColor(IVORY),
                    TextShadow::default(),
                ));
            });
            let a = &campaign.attributes;
            let traits = [
                (localization.text("hunger"), a.hunger),
                (localization.text("boredom"), a.boredom),
                (localization.text("stress"), a.stress),
                (localization.text("urge"), a.urge),
                (localization.text("relationships"), a.relationships),
            ];
            for indices in [&[0, 1][..], &[3, 4][..]] {
                hud.spawn(Node {
                    justify_content: JustifyContent::End,
                    column_gap: px(9),
                    ..default()
                })
                .with_children(|row| {
                    for &index in indices {
                        let (label, value) = traits[index];
                        row.spawn((
                            Name::new(format!("{label}: {value} / 100")),
                            Node {
                                width: px(80),
                                flex_direction: FlexDirection::Column,
                                align_items: AlignItems::Center,
                                row_gap: px(4),
                                ..default()
                            },
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
                            ))
                            .with_children(|badge| {
                                badge
                                    .spawn((
                                        Node {
                                            min_width: px(38),
                                            padding: UiRect::axes(px(6), px(1)),
                                            border: UiRect::all(px(1)),
                                            justify_content: JustifyContent::Center,
                                            ..default()
                                        },
                                        BackgroundColor(INK),
                                        BorderColor::all(BRASS),
                                    ))
                                    .with_children(|plate| {
                                        plate.spawn((
                                            Text::new(value.to_string()),
                                            skin.font(19.0),
                                            TextColor(IVORY),
                                        ));
                                    });
                            });
                            stat.spawn((
                                Text::new(label),
                                skin.font(12.0),
                                TextColor(IVORY),
                                TextShadow::default(),
                            ));
                        });
                    }
                });
            }
        });
}

pub(super) fn interaction(commands: &mut Commands, label: &str, skin: &HudSkin) {
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
                        Text::new(format!("Interact · {label}")),
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
        };
        for index in 0..6 {
            let rect = skin.badge(index).rect.unwrap();
            assert!(rect.min.cmpge(Vec2::ZERO).all());
            assert!(rect.max.cmple(Vec2::new(width as f32, height as f32)).all());
            assert_eq!(rect.width(), rect.height());
        }
    }
}
