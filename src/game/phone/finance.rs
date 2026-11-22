//! Daily income and spending for the phone history.
use super::*;
use crate::game::routine::day_money_text;

pub(super) fn render(
    parent: &mut ChildSpawnerCommands,
    assets: &PhoneAssets,
    art: &ui::Art,
    c: &Campaign,
    localization: &Localization,
) {
    text(parent, &assets.font, localization.choose("All days log", "Histórico de todos os dias"), 30.0, Color::srgb(0.43, 0.15, 0.70));
    for day in (1..=c.clock.day).rev() {
        parent
            .spawn((
                Node {
                    padding: UiRect::all(px(14)),
                    border_radius: BorderRadius::all(px(12)),
                    flex_shrink: 0.0,
                    ..ui::column()
                },
                BackgroundColor(Color::srgb(0.95, 0.93, 0.97)),
            ))
            .with_children(|card| {
                text(
                    card,
                    &art.regular,
                    day_money_text(c, day, localization),
                    20.0,
                    Color::srgb(0.12, 0.10, 0.17),
                );
            });
    }
}
