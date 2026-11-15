//! The wall calendar reads the same clock and bills as the bank.
use bevy::{
    camera::RenderTarget,
    core_pipeline::tonemapping::{DebandDither, Tonemapping},
    image::ImageSampler,
    light::NotShadowCaster,
    prelude::*,
    render::render_resource::TextureFormat,
    text::FontSmoothing,
    ui::FocusPolicy,
    ui_render::UiAntiAlias,
};

use super::{
    campaign::{Bill, BillKind, DAYS},
    campaign_save::CampaignStore,
};

const INK: Color = Color::srgb(0.13, 0.11, 0.08);
const RULE: Color = Color::srgb(0.30, 0.25, 0.18);
const TODAY: Color = Color::srgb(0.13, 0.27, 0.31);
const UNPAID: Color = Color::srgb(0.55, 0.15, 0.10);
const PAID: Color = Color::srgb(0.17, 0.33, 0.17);

pub(super) struct CalendarPlugin;

#[derive(Resource)]
struct CalendarAssets {
    root: Entity,
    font: Handle<Font>,
}

#[derive(Component)]
struct CalendarContent;

#[derive(Component)]
pub(super) struct CalendarText;

impl Plugin for CalendarPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup)
            .add_systems(PostUpdate, present.before(bevy::ui::UiSystems::Prepare));
    }
}

fn setup(
    mut commands: Commands,
    assets: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    let mut image = Image::new_target_texture(512, 640, TextureFormat::Bgra8UnormSrgb, None);
    image.sampler = ImageSampler::nearest();
    let target = images.add(image);
    let camera = commands
        .spawn((
            Name::new("Wall calendar texture camera"),
            Camera2d,
            Msaa::Off,
            Tonemapping::None,
            DebandDither::Disabled,
            UiAntiAlias::Off,
            Camera {
                order: -2,
                ..default()
            },
            RenderTarget::Image(target.clone().into()),
        ))
        .id();
    let root = commands
        .spawn((
            Name::new("Wall calendar page"),
            UiTargetCamera(camera),
            ImageNode::new(assets.load("ui/wall-calendar.png")),
            Node {
                width: percent(100),
                height: percent(100),
                ..default()
            },
            Pickable::IGNORE,
            FocusPolicy::Pass,
        ))
        .id();
    let paper = materials.add(StandardMaterial {
        base_color: Color::srgb(0.69, 0.60, 0.43),
        perceptual_roughness: 1.0,
        ..default()
    });
    commands
        .spawn((
            Name::new("Wall calendar left of TV"),
            Transform::from_xyz(0.038, 1.83, 0.95)
                .with_rotation(Quat::from_rotation_y(std::f32::consts::FRAC_PI_2)),
            Visibility::Inherited,
        ))
        .with_children(|calendar| {
            calendar.spawn((
                Mesh3d(meshes.add(Cuboid::new(0.80, 1.00, 0.02))),
                MeshMaterial3d(paper),
                Pickable::IGNORE,
            ));
            calendar.spawn((
                Mesh3d(meshes.add(Rectangle::new(0.80, 1.00))),
                MeshMaterial3d(materials.add(StandardMaterial {
                    base_color_texture: Some(target.clone()),
                    // Low textured emission keeps the dates legible in this dim wall corner.
                    emissive_texture: Some(target),
                    emissive: LinearRgba::rgb(0.08, 0.08, 0.065),
                    perceptual_roughness: 1.0,
                    ..default()
                })),
                Transform::from_xyz(0.0, 0.0, 0.0105),
                NotShadowCaster,
                Pickable::IGNORE,
            ));
            calendar.spawn((
                Mesh3d(meshes.add(Cuboid::new(0.80, 0.012, 0.024))),
                MeshMaterial3d(materials.add(StandardMaterial {
                    base_color: Color::srgb(0.23, 0.17, 0.11),
                    perceptual_roughness: 1.0,
                    ..default()
                })),
                Transform::from_xyz(0.0, 0.494, 0.002),
                Pickable::IGNORE,
            ));
        });
    commands.insert_resource(CalendarAssets {
        root,
        font: assets.load("fonts/LiberationSans-Bold.ttf"),
    });
}

fn bill_status(bill: &Bill, today: u8) -> (&'static str, Color) {
    if bill.paid_day.is_some() {
        ("Paid", PAID)
    } else if today > bill.due_day {
        ("Overdue", UNPAID)
    } else if today == bill.due_day {
        ("Due today", UNPAID)
    } else {
        ("Unpaid", UNPAID)
    }
}

fn text(
    parent: &mut ChildSpawnerCommands,
    font: &Handle<Font>,
    value: impl Into<String>,
    size: f32,
    color: Color,
) {
    parent.spawn((
        CalendarText,
        Text::new(value),
        TextFont::from_font_size(size)
            .with_font(font.clone())
            .with_font_smoothing(FontSmoothing::None),
        TextColor(color),
        Pickable::IGNORE,
    ));
}

fn present(
    mut commands: Commands,
    store: Res<CampaignStore>,
    assets: Res<CalendarAssets>,
    existing: Query<Entity, With<CalendarContent>>,
) {
    if !store.is_changed() && !assets.is_added() {
        return;
    }
    for entity in &existing {
        commands.entity(entity).despawn();
    }
    let Some(campaign) = store.campaign() else {
        return;
    };
    commands.entity(assets.root).with_children(|page| {
        page.spawn((
            CalendarContent,
            Node {
                width: percent(100),
                height: percent(100),
                padding: UiRect {
                    top: px(72),
                    ..UiRect::all(px(32))
                },
                flex_direction: FlexDirection::Column,
                row_gap: px(16),
                ..default()
            },
            Pickable::IGNORE,
            FocusPolicy::Pass,
        ))
        .with_children(|content| {
            text(
                content,
                &assets.font,
                format!("TODAY: DAY {}", campaign.clock.day),
                42.0,
                TODAY,
            );
            content
                .spawn(Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: px(8),
                    ..default()
                })
                .with_children(|grid| {
                    for row in 0..2 {
                        grid.spawn(Node {
                            column_gap: px(6),
                            ..default()
                        })
                        .with_children(|cells| {
                            for day in (row * 5 + 1)..=(row * 5 + 5).min(DAYS) {
                                let today = day == campaign.clock.day;
                                cells
                                    .spawn((
                                        Name::new(format!("Calendar day {day}")),
                                        Node {
                                            width: px(84.8),
                                            height: px(136),
                                            border: UiRect::all(px(if today { 4 } else { 2 })),
                                            padding: UiRect::top(px(6)),
                                            flex_direction: FlexDirection::Column,
                                            align_items: AlignItems::Center,
                                            flex_shrink: 0.0,
                                            ..default()
                                        },
                                        BorderColor::all(if today { TODAY } else { RULE }),
                                    ))
                                    .with_children(|cell| {
                                        text(cell, &assets.font, day.to_string(), 46.0, INK);
                                        text(
                                            cell,
                                            &assets.font,
                                            if today { "TODAY" } else { "" },
                                            18.0,
                                            TODAY,
                                        );
                                        for bill in
                                            campaign.bills.iter().filter(|b| b.due_day == day)
                                        {
                                            let label = match bill.kind {
                                                BillKind::Electricity => "POWER",
                                                BillKind::Rent => "RENT",
                                            };
                                            let (status, color) =
                                                bill_status(bill, campaign.clock.day);
                                            text(cell, &assets.font, label, 18.0, color);
                                            if status == "Paid" {
                                                text(cell, &assets.font, "PAID", 18.0, color);
                                            }
                                        }
                                    });
                            }
                        });
                    }
                });
            content
                .spawn((
                    Node {
                        border: UiRect::top(px(2)),
                        padding: UiRect::top(px(16)),
                        flex_direction: FlexDirection::Column,
                        row_gap: px(12),
                        ..default()
                    },
                    BorderColor::all(RULE),
                ))
                .with_children(|bills| {
                    for bill in &campaign.bills {
                        bills
                            .spawn(Node {
                                flex_direction: FlexDirection::Column,
                                ..default()
                            })
                            .with_children(|row| {
                                text(
                                    row,
                                    &assets.font,
                                    format!("{}: Day {}", bill.kind.label(), bill.due_day),
                                    34.0,
                                    INK,
                                );
                                let (status, color) = bill_status(bill, campaign.clock.day);
                                text(row, &assets.font, status, 24.0, color);
                            });
                    }
                });
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::{
        campaign::{Action, Campaign},
        campaign_save::SnapshotState,
    };

    #[test]
    fn bill_status_uses_end_of_day_deadline_and_payment_takes_priority() {
        let mut bill = Campaign::new(1).bills.remove(0);
        assert_eq!(bill_status(&bill, 3).0, "Unpaid");
        assert_eq!(bill_status(&bill, 4).0, "Due today");
        assert_eq!(bill_status(&bill, 5).0, "Overdue");
        bill.paid_day = Some(2);
        for day in [2, 4, 5, 10] {
            assert_eq!(bill_status(&bill, day), ("Paid", PAID));
        }
    }

    fn copy(app: &mut App) -> String {
        let mut texts = app.world_mut().query::<&Text>();
        texts
            .iter(app.world())
            .map(|text| text.0.as_str())
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn day_copy(app: &mut App, day: u8) -> String {
        let world = app.world_mut();
        let mut cells = world.query::<(Entity, &Name)>();
        let cell = cells
            .iter(world)
            .find(|(_, name)| name.as_str() == format!("Calendar day {day}"))
            .unwrap()
            .0;
        world
            .get::<Children>(cell)
            .unwrap()
            .iter()
            .filter_map(|entity| world.get::<Text>(entity))
            .map(|text| text.0.as_str())
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn show(app: &mut App, campaign: Option<Campaign>) {
        app.insert_resource(CampaignStore::for_test(
            campaign.clone(),
            SnapshotState::Protected,
        ));
        app.update();
        assert_eq!(
            app.world().resource::<CampaignStore>().campaign(),
            campaign.as_ref()
        );
        let mut contents = app.world_mut().query::<&CalendarContent>();
        assert_eq!(
            contents.iter(app.world()).count(),
            usize::from(campaign.is_some())
        );
    }

    #[test]
    fn calendar_follows_reports_payments_extensions_and_campaign_replacement_read_only() {
        let mut app = App::new();
        let root = app.world_mut().spawn_empty().id();
        app.insert_resource(CalendarAssets {
            root,
            font: Handle::default(),
        })
        .add_systems(Update, present);
        show(&mut app, None);
        let mut campaign = Campaign::new(9);
        show(&mut app, Some(campaign.clone()));
        assert!(copy(&mut app).contains("TODAY: DAY 1"));
        assert!(copy(&mut app).contains("Electricity: Day 4"));
        assert!(copy(&mut app).contains("Rent: Day 7"));
        assert!(day_copy(&mut app, 1).contains("TODAY"));
        assert!(day_copy(&mut app, 4).contains("POWER"));
        assert!(day_copy(&mut app, 7).contains("RENT"));

        campaign.apply(Action::Sleep).unwrap();
        show(&mut app, Some(campaign.clone()));
        assert!(day_copy(&mut app, 1).contains("TODAY"));
        campaign.apply(Action::AcknowledgeReport).unwrap();
        campaign
            .apply(Action::PayBill(BillKind::Electricity))
            .unwrap();
        campaign.apply(Action::NegotiateRent).unwrap();
        show(&mut app, Some(campaign.clone()));
        assert!(!day_copy(&mut app, 1).contains("TODAY"));
        assert!(day_copy(&mut app, 2).contains("TODAY"));
        assert!(day_copy(&mut app, 4).contains("PAID"));
        assert!(!day_copy(&mut app, 7).contains("RENT"));
        assert!(day_copy(&mut app, 10).contains("RENT"));
        assert!(copy(&mut app).contains("Rent: Day 10"));

        campaign.clock.day = 5;
        campaign.bills[0].paid_day = None;
        campaign
            .household
            .sold
            .push(super::super::campaign::HouseholdItem::Tv);
        show(&mut app, Some(campaign));
        assert!(copy(&mut app).contains("Overdue"));
        assert!(day_copy(&mut app, 5).contains("TODAY"));

        show(&mut app, Some(Campaign::new(10)));
        assert!(copy(&mut app).contains("Rent: Day 7"));
        assert!(!copy(&mut app).contains("PAID"));
        assert!(day_copy(&mut app, 1).contains("TODAY"));
        show(&mut app, None);
        assert!(copy(&mut app).is_empty());
    }
}
