//! Modern phone launcher with PSX bitmap sampling; app contents stay in `phone.rs`.
use super::*;

pub(super) const APPS: [(PhonePage, &str); 7] = [
    (PhonePage::Betting, "Betting"),
    (PhonePage::Bank, "Bank"),
    (PhonePage::History, "History"),
    (PhonePage::Household, "Sales"),
    (PhonePage::Contacts, "Contacts"),
    (PhonePage::Support, "Support"),
    (PhonePage::Food, "Food"),
];

#[derive(Resource)]
pub(super) struct Skin {
    pub wallpaper: Handle<Image>,
    icons: Handle<Image>,
    font: Handle<Font>,
}

impl FromWorld for Skin {
    fn from_world(world: &mut World) -> Self {
        let assets = world.resource::<AssetServer>();
        Self {
            wallpaper: assets
                .load_builder()
                .with_settings(|settings: &mut ImageLoaderSettings| {
                    settings.sampler = ImageSampler::nearest();
                })
                .load("ui/phone-wallpaper.png"),
            icons: assets
                .load_builder()
                .with_settings(|settings: &mut ImageLoaderSettings| {
                    settings.sampler = ImageSampler::nearest();
                })
                .load("ui/phone-icons.png"),
            font: assets.load("fonts/LiberationSans-Regular.ttf"),
        }
    }
}

impl Skin {
    fn icon(&self, index: usize) -> ImageNode {
        let origin = Vec2::new((index % 4) as f32, (index / 4) as f32) * 32.0;
        ImageNode::new(self.icons.clone())
            .with_rect(Rect::from_corners(origin, origin + Vec2::splat(32.0)))
    }
}

fn time(c: &Campaign) -> &'static str {
    // The campaign advances in periods, so the phone never invents passing game minutes.
    match c.clock.period() {
        Period::Morning => "08:00",
        Period::Afternoon => "14:00",
        Period::Night => "20:00",
    }
}

pub(super) fn status_bar(parent: &mut ChildSpawnerCommands, skin: &Skin, c: &Campaign) {
    parent
        .spawn((
            Name::new("Phone status bar"),
            Node {
                height: px(34),
                width: percent(100),
                padding: UiRect::axes(px(22), px(6)),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::SpaceBetween,
                flex_shrink: 0.0,
                ..default()
            },
            Pickable::IGNORE,
        ))
        .with_children(|bar| {
            text(bar, &skin.font, time(c), 16.0, INK);
            bar.spawn(Node {
                column_gap: px(3),
                align_items: AlignItems::Center,
                ..default()
            })
            .with_children(|signals| {
                for index in [10, 11] {
                    signals.spawn((
                        skin.icon(index),
                        Node {
                            width: px(20),
                            height: px(20),
                            ..default()
                        },
                        Pickable::IGNORE,
                    ));
                }
            });
        });
}

fn shortcut(parent: &mut ChildSpawnerCommands, skin: &Skin, index: usize) {
    let (page, label) = APPS[index];
    parent
        .spawn((
            Name::new(format!("Open {label}")),
            Button,
            PhoneButton(PhonePage::Home, PhoneCommand::Page(page)),
            BackgroundColor(Color::NONE),
            Node {
                width: px(82),
                min_height: px(100),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: px(5),
                padding: UiRect::all(px(3)),
                border_radius: BorderRadius::all(px(12)),
                flex_shrink: 0.0,
                ..default()
            },
        ))
        .observe(queue_press)
        .observe(highlight)
        .observe(unhighlight)
        .with_children(|app| {
            app.spawn((
                skin.icon(index),
                Node {
                    width: px(72),
                    height: px(72),
                    ..default()
                },
                Pickable::IGNORE,
            ));
            text(app, &skin.font, label, 17.0, INK);
        });
}

fn highlight(over: On<Pointer<Over>>, mut colors: Query<&mut BackgroundColor, With<PhoneButton>>) {
    if let Ok(mut color) = colors.get_mut(over.entity) {
        color.0 = Color::srgba(1.0, 1.0, 1.0, 0.14);
    }
}

fn unhighlight(out: On<Pointer<Out>>, mut colors: Query<&mut BackgroundColor, With<PhoneButton>>) {
    if let Ok(mut color) = colors.get_mut(out.entity) {
        color.0 = Color::NONE;
    }
}

pub(super) fn home(parent: &mut ChildSpawnerCommands, skin: &Skin, c: &Campaign) {
    parent
        .spawn((
            Name::new("Phone home screen"),
            PhoneScroll,
            ScrollPosition::default(),
            Node {
                width: percent(100),
                flex_grow: 1.0,
                min_height: px(0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                overflow: Overflow::scroll_y(),
                ..default()
            },
        ))
        .with_children(|home| {
            home.spawn((
                Node {
                    width: px(196),
                    height: px(196),
                    margin: UiRect::top(px(24)),
                    border: UiRect::all(px(2)),
                    border_radius: BorderRadius::MAX,
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    row_gap: px(5),
                    flex_shrink: 0.0,
                    ..default()
                },
                BorderColor::all(Color::srgba(0.82, 0.95, 0.98, 0.65)),
                BackgroundColor(Color::srgba(0.02, 0.10, 0.18, 0.25)),
                Pickable::IGNORE,
            ))
            .with_children(|clock| {
                text(clock, &skin.font, c.clock.period().label(), 18.0, MUTED);
                text(clock, &skin.font, time(c), 51.0, INK);
                text(clock, &skin.font, format!("Day {}", c.clock.day), 20.0, INK);
            });
            home.spawn((
                Node {
                    width: percent(100),
                    padding: UiRect::all(px(16)),
                    margin: UiRect::axes(px(0), px(16)),
                    border_radius: BorderRadius::all(px(16)),
                    row_gap: px(6),
                    flex_direction: FlexDirection::Column,
                    flex_shrink: 0.0,
                    ..default()
                },
                BackgroundColor(Color::srgba(0.02, 0.09, 0.16, 0.86)),
                Pickable::IGNORE,
            ))
            .with_children(|reminder| {
                text(reminder, &skin.font, "Reminder", 14.0, ACCENT);
                text(reminder, &skin.font, home_reminder(c), 19.0, INK);
            });
            home.spawn(Node {
                flex_grow: 1.0,
                min_height: px(20),
                ..default()
            });
            home.spawn(Node {
                width: percent(100),
                justify_content: JustifyContent::SpaceBetween,
                flex_shrink: 0.0,
                ..default()
            })
            .with_children(|apps| {
                for index in 0..4 {
                    shortcut(apps, skin, index);
                }
            });
            home.spawn((
                Name::new("Phone favorites dock"),
                Node {
                    width: percent(100),
                    padding: UiRect::top(px(18)),
                    margin: UiRect::top(px(14)),
                    border: UiRect::top(px(1)),
                    justify_content: JustifyContent::SpaceEvenly,
                    flex_shrink: 0.0,
                    ..default()
                },
                BorderColor::all(Color::srgba(0.82, 0.95, 0.98, 0.22)),
            ))
            .with_children(|dock| {
                for index in 4..APPS.len() {
                    shortcut(dock, skin, index);
                }
            });
        });
}

pub(super) fn navigation(parent: &mut ChildSpawnerCommands, skin: &Skin, page: PhonePage) {
    parent
        .spawn((
            Name::new("Phone navigation"),
            BackgroundColor(Color::srgba(0.01, 0.03, 0.05, 0.92)),
            Node {
                width: percent(100),
                height: px(56),
                justify_content: JustifyContent::SpaceEvenly,
                align_items: AlignItems::Center,
                flex_shrink: 0.0,
                ..default()
            },
        ))
        .with_children(|nav| {
            for (index, label, command) in [
                (7, "Back", PhoneCommand::Page(page.back())),
                (8, "Home", PhoneCommand::Page(PhonePage::Home)),
                (9, "Lower", PhoneCommand::Close),
            ] {
                nav.spawn((
                    Name::new(label),
                    Button,
                    PhoneButton(page, command),
                    BackgroundColor(Color::NONE),
                    Node {
                        width: px(112),
                        height: px(48),
                        border_radius: BorderRadius::all(px(12)),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        column_gap: px(5),
                        ..default()
                    },
                ))
                .observe(queue_press)
                .observe(highlight)
                .observe(unhighlight)
                .with_children(|button| {
                    button.spawn((
                        skin.icon(index),
                        Node {
                            width: px(23),
                            height: px(23),
                            ..default()
                        },
                        Pickable::IGNORE,
                    ));
                    text(button, &skin.font, label, 16.0, INK);
                });
            }
        });
}
