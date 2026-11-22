//! Permanent household sales and presentation derived from the authoritative campaign.
use bevy::{audio::Volume, prelude::*, world_serialization::WorldInstanceReady};
use bevy_rapier3d::prelude::{Collider, ColliderDisabled};

use super::{
    campaign::*, campaign_save::CampaignStore, player::Apartment, routine::GameplayOverlay,
};
use crate::{localization::Localization, state::GameState};

pub const HOUSEHOLD_ITEMS: [HouseholdItem; 6] = [
    HouseholdItem::Tv,
    HouseholdItem::Fridge,
    HouseholdItem::Stove,
    HouseholdItem::Cookware,
    HouseholdItem::Couch,
    HouseholdItem::Books,
];

impl HouseholdItem {
    pub fn label(self) -> &'static str {
        match self {
            Self::Tv => "TV",
            Self::Fridge => "Fridge",
            Self::Stove => "Stove",
            Self::Cookware => "Cookware",
            Self::Couch => "Couch",
            Self::Books => "Books",
            Self::Pipoca => "Pipoca",
        }
    }
    pub fn sale_cents(self) -> i64 {
        match self {
            Self::Tv => 9_000,
            Self::Fridge => 12_000,
            Self::Stove => 8_000,
            Self::Cookware => 3_000,
            Self::Couch => 6_000,
            Self::Books => 2_000,
            Self::Pipoca => 20_000,
        }
    }
    pub fn sale_loss(self) -> &'static str {
        match self {
            Self::Tv => "The TV is permanently removed. Watching TV will be unavailable.",
            Self::Fridge => {
                "The fridge is permanently removed. ALL stored food portions are discarded. Cooking and grocery storage will be unavailable; prepared-food delivery remains available."
            }
            Self::Stove | Self::Cookware => {
                "This kitchen equipment is permanently removed. Cooking and grocery storage will be unavailable; prepared-food delivery remains available."
            }
            Self::Couch => "The couch is permanently removed from your home.",
            Self::Books => "All six books are permanently removed. The shelf stays.",
            Self::Pipoca => {
                "Pipoca permanently leaves your home. Time together becomes unavailable. THIS ENDS THE CAMPAIGN immediately with a nonfatal failure ending."
            }
        }
    }
}

impl Campaign {
    pub fn can_sell(&self, item: HouseholdItem) -> Result<(), String> {
        if !self.household.owns(item) {
            return Err(format!(
                "{} has already been sold. Sales are permanent.",
                item.label()
            ));
        }
        self.require_low_stress()?;
        if item == HouseholdItem::Pipoca {
            if HOUSEHOLD_ITEMS.into_iter().any(|i| self.household.owns(i)) {
                return Err(
                    "Pipoca can only be offered after all six household groups have been sold."
                        .into(),
                );
            }
        } else if self.attributes.stress < SALE_PRESSURE && self.attributes.urge < SALE_PRESSURE {
            return Err("Household sales are offered at stress or urge 70 or above. Other actions remain available.".into());
        }
        Ok(())
    }

    pub(super) fn sell(&mut self, item: HouseholdItem) -> Result<(), String> {
        self.can_sell(item)?;
        self.spend_pa(1)?;
        self.record_money(LedgerKind::Sale(item), item.sale_cents())?;
        self.household.sold.push(item);
        if item == HouseholdItem::Fridge {
            self.household.food_portions = 0;
        }
        if item == HouseholdItem::Pipoca {
            self.stage = CampaignStage::Ended(Ending::DogSale);
        }
        Ok(())
    }

    pub(super) fn validate_household(&self) -> Result<(), String> {
        for item in HOUSEHOLD_ITEMS.into_iter().chain([HouseholdItem::Pipoca]) {
            let sales: Vec<_> = self
                .ledger
                .iter()
                .filter(|e| e.kind == LedgerKind::Sale(item))
                .collect();
            if sales.len() != usize::from(!self.household.owns(item))
                || sales.iter().any(|e| e.amount_cents != item.sale_cents())
            {
                return Err(
                    "Household sales must match ownership and their exact one-time proceeds."
                        .into(),
                );
            }
        }
        if !self.household.owns(HouseholdItem::Pipoca)
            != (self.stage == CampaignStage::Ended(Ending::DogSale))
            || (!self.household.owns(HouseholdItem::Pipoca)
                && HOUSEHOLD_ITEMS.into_iter().any(|i| self.household.owns(i)))
        {
            return Err(
                "Pipoca's sale must end the campaign after all six household sales.".into(),
            );
        }
        Ok(())
    }
}

pub struct HouseholdPlugin;

#[derive(Component)]
pub(crate) struct InteriorLight;
#[derive(Component)]
pub(crate) struct Pipoca;
#[derive(Component)]
struct Scanned;
#[derive(Component, Clone, Copy)]
enum HouseholdVisual {
    Owned(HouseholdItem),
    Food(u16),
    Waste(u8),
}
#[derive(Component)]
struct DogAnimation(Handle<AnimationGraph>, AnimationNodeIndex);
#[derive(Component)]
struct PressureAudio;
#[derive(Component)]
struct Thought;
#[derive(Resource, Default)]
struct Presentation {
    title: bool,
}

impl Plugin for HouseholdPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Presentation>()
            .add_systems(Startup, setup)
            .add_systems(
                Update,
                (scan_furniture, sync_visuals, environment.after(super::routine::RoutineView)).chain(),
            );
    }
}

pub(super) fn item_for_name(name: &str) -> Option<HouseholdItem> {
    let name = name.to_ascii_lowercase();
    if !name.starts_with("apartment_") {
        return None;
    }
    if name.contains("refrigerator") {
        Some(HouseholdItem::Fridge)
    } else if name.contains("stove.") {
        Some(HouseholdItem::Stove)
    } else if [
        "cookware",
        "casserole",
        "skillet",
        "stewpot",
        "dishes_",
        "cutlery_holder",
        "knife _holder",
    ]
    .iter()
    .any(|part| name.contains(part))
    {
        Some(HouseholdItem::Cookware)
    } else if name.contains("tv.") {
        Some(HouseholdItem::Tv)
    } else if name.contains("couch.") {
        Some(HouseholdItem::Couch)
    } else if name.contains("book_") {
        Some(HouseholdItem::Books)
    } else {
        None
    }
}

fn scan_furniture(
    mut commands: Commands,
    apartment: Query<Entity, With<Apartment>>,
    meshes: Query<Entity, (With<Mesh3d>, Without<Scanned>)>,
    hierarchy: Query<(Option<&Name>, Option<&ChildOf>)>,
) {
    let Ok(root) = apartment.single() else { return };
    for entity in &meshes {
        let mut current = Some(entity);
        let mut visual = None;
        let mut in_apartment = false;
        while let Some(ancestor) = current {
            if ancestor == root {
                in_apartment = true;
                break;
            }
            let Ok((name, parent)) = hierarchy.get(ancestor) else {
                break;
            };
            if let Some(name) = name {
                visual =
                    visual.or_else(|| item_for_name(name.as_str()).map(HouseholdVisual::Owned));
                if name.as_str().starts_with("Apartment_Cereal.") {
                    visual = Some(HouseholdVisual::Food(1));
                }
            }
            current = parent.map(ChildOf::parent);
        }
        if in_apartment {
            commands.entity(entity).insert(Scanned);
            if let Some(visual) = visual {
                commands.entity(entity).insert(visual);
            }
        }
    }
}

fn setup(
    mut commands: Commands,
    assets: Res<AssetServer>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
) {
    let (graph, index) = AnimationGraph::from_clip(
        assets.load(GltfAssetLabel::Animation(0).from_asset("models/pipoca.glb")),
    );
    commands
        .spawn((
            Name::new("Pipoca sitting by the window"),
            Pipoca,
            HouseholdVisual::Owned(HouseholdItem::Pipoca),
            WorldAssetRoot(assets.load(GltfAssetLabel::Scene(0).from_asset("models/pipoca.glb"))),
            Transform::from_xyz(2.72, 0.02, 1.70)
                .with_rotation(Quat::from_rotation_y(std::f32::consts::FRAC_PI_2))
                .with_scale(Vec3::splat(0.28)),
            DogAnimation(graphs.add(graph), index),
            // Collider dimensions are local, scaled with the supplied rig's runtime root.
            Collider::compound(vec![(
                Vec3::new(0.0, 1.4, 0.3),
                Quat::IDENTITY,
                Collider::cuboid(0.65, 1.5, 2.1),
            )]),
        ))
        .observe(animate_dog);
    // shortcut: at most three meal stacks, expand if exact physical inventory becomes necessary.
    for (i, x) in [2.48, 2.8, 3.2].into_iter().enumerate() {
        commands.spawn((
            HouseholdVisual::Food(i as u16 + 1),
            WorldAssetRoot(
                assets.load(GltfAssetLabel::Scene(0).from_asset("models/food_portion.glb")),
            ),
            Transform::from_xyz(x, 1.17, 4.04),
        ));
    }
    for (i, (file, position, scale)) in [
        ("waste_paper.glb", Vec3::new(2.15, 0.01, 1.45), 0.75),
        ("waste_meal.glb", Vec3::new(1.1, 0.01, 4.4), 0.65),
        ("waste_bag.glb", Vec3::new(3.1, 0.02, 3.35), 0.5),
    ]
    .into_iter()
    .enumerate()
    {
        commands.spawn((
            HouseholdVisual::Waste(i as u8 + 1),
            WorldAssetRoot(
                assets.load(GltfAssetLabel::Scene(0).from_asset(format!("models/{file}"))),
            ),
            Transform::from_translation(position).with_scale(Vec3::splat(scale)),
        ));
    }
    commands.spawn((
        PressureAudio,
        AudioPlayer::new(assets.load("audio/pressure.wav")),
        PlaybackSettings::LOOP.with_volume(Volume::Linear(0.0)),
    ));
    commands.spawn((
        Thought,
        Text::new(""),
        TextFont::from_font_size(18.0).with_font(assets.load("fonts/LiberationSans-Regular.ttf")),
        TextColor(Color::srgb(0.92, 0.91, 0.86)),
        Node {
            position_type: PositionType::Absolute,
            bottom: px(24),
            left: percent(24),
            max_width: percent(52),
            ..default()
        },
        Visibility::Hidden,
    ));
}

fn animate_dog(
    ready: On<WorldInstanceReady>,
    mut commands: Commands,
    children: Query<&Children>,
    graphs: Query<&DogAnimation>,
    mut players: Query<&mut AnimationPlayer>,
) {
    let Ok(graph) = graphs.get(ready.entity) else {
        return;
    };
    for entity in children.iter_descendants(ready.entity) {
        if let Ok(mut player) = players.get_mut(entity) {
            player.play(graph.1).repeat();
            commands
                .entity(entity)
                .insert(AnimationGraphHandle(graph.0.clone()));
        }
    }
}

fn sync_visuals(
    mut commands: Commands,
    store: Res<CampaignStore>,
    state: Res<State<GameState>>,
    mut presentation: ResMut<Presentation>,
    mut visuals: Query<(
        Entity,
        &HouseholdVisual,
        &mut Visibility,
        Has<ColliderDisabled>,
    )>,
) {
    if *state.get() == GameState::Title {
        presentation.title = true;
    }
    if matches!(*state.get(), GameState::Intro | GameState::Playing) {
        presentation.title = false;
    }
    for (entity, visual, mut visibility, disabled) in &mut visuals {
        let show = visual_visible(*visual, store.campaign(), presentation.title);
        let wanted = if show {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *visibility != wanted {
            *visibility = wanted;
        }
        if disabled == show {
            if show {
                commands.entity(entity).remove::<ColliderDisabled>();
            } else {
                commands.entity(entity).insert(ColliderDisabled);
            }
        }
    }
}

fn visual_visible(visual: HouseholdVisual, campaign: Option<&Campaign>, title: bool) -> bool {
    match visual {
        HouseholdVisual::Owned(item) if item != HouseholdItem::Pipoca && title => true,
        _ if title => false,
        HouseholdVisual::Owned(item) => campaign.is_some_and(|c| c.household.owns(item)),
        HouseholdVisual::Food(n) => campaign.is_some_and(|c| c.household.food_portions >= n),
        HouseholdVisual::Waste(n) => campaign.is_some_and(|c| c.household.mess >= n),
    }
}

fn pressure(stress: u8) -> f32 {
    (f32::from(stress) - 40.0).max(0.0) / 60.0
}

fn bulb_brightness(seconds: f32) -> f32 {
    1.0 + 0.012 * (seconds * 11.7).sin() + 0.007 * (seconds * 23.9).sin()
        - 0.035 * (seconds * 1.9).sin().max(0.0).powi(12)
}

fn thought(c: &Campaign, localization: &Localization) -> &'static str {
    if c.betting.history.is_empty() || c.attributes.urge < 60 {
        ""
    } else if c.attributes.urge < 80 {
        localization.choose("The betting app keeps coming to mind.", "O aplicativo de apostas não sai da minha cabeça.")
    } else {
        localization.choose("I keep thinking about another bet. I can step away.", "Não paro de pensar em fazer mais uma aposta. Posso me afastar um pouco.")
    }
}

fn environment(
    localization: Res<Localization>,
    store: Res<CampaignStore>,
    state: Res<State<GameState>>,
    time: Res<Time<Real>>,
    presentation: Res<Presentation>,
    overlay: Res<GameplayOverlay>,
    mut lights: Query<&mut SpotLight, With<InteriorLight>>,
    mut audio: Query<&mut AudioSink, With<PressureAudio>>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    mut thoughts: Query<(&mut Text, &mut Visibility), With<Thought>>,
) {
    let campaign = store.campaign();
    let playing = *state.get() == GameState::Playing
        && campaign.is_some_and(|c| c.stage == CampaignStage::Playing);
    let stress = if presentation.title {
        0.0
    } else {
        campaign.map_or(0.0, |c| pressure(c.attributes.stress))
    };
    let powered = presentation.title || campaign.is_none_or(Campaign::has_power);
    let pulse = (time.elapsed_secs() * 0.9).sin() * 0.015 * stress;
    let brightness = bulb_brightness(time.elapsed_secs());
    for mut light in &mut lights {
        light.intensity = if powered {
            40_000.0 * (1.0 - 0.12 * stress + pulse) * brightness
        } else {
            0.0
        };
    }
    for mut sink in &mut audio {
        sink.set_volume(Volume::Linear(
            if playing && windows.single().is_ok_and(|w| w.focused) {
                stress * 0.3
            } else {
                0.0
            },
        ));
    }
    for (mut text, mut visibility) in &mut thoughts {
        let value = if playing && !overlay.blocks_world() {
            campaign.map_or("", |campaign| thought(campaign, &localization))
        } else {
            ""
        };
        if text.0 != value {
            text.0 = value.into();
        }
        *visibility = if value.is_empty() {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        };
    }
}

#[cfg(test)]
mod tests {
    use super::super::campaign_save::{SnapshotState, decode_campaign, encode_campaign};
    use super::*;
    use bevy::{ecs::system::RunSystemOnce, time::Real};
    use bevy_rapier3d::prelude::*;

    fn offered() -> Campaign {
        let mut c = Campaign::new(5);
        c.attributes.stress = 70;
        c
    }

    #[test]
    fn sales_are_atomic_exact_permanent_and_fridge_discards_stored_food() {
        let mut no_pa = offered();
        no_pa.clock.pa_spent = 6;
        for item in HOUSEHOLD_ITEMS {
            let before = no_pa.clone();
            assert!(no_pa.apply(Action::Sell(item)).is_err());
            assert_eq!(no_pa, before);
        }
        for (stress, urge) in [(69, 69), (0, 0)] {
            let mut c = Campaign::new(5);
            c.attributes.stress = stress;
            c.attributes.urge = urge;
            for item in HOUSEHOLD_ITEMS.into_iter().chain([HouseholdItem::Pipoca]) {
                let before = c.clone();
                assert!(c.apply(Action::Sell(item)).is_err());
                assert_eq!(c, before);
            }
        }
        for by_urge in [false, true] {
            let mut c = offered();
            if by_urge {
                c.attributes.stress = 0;
                c.attributes.urge = 70;
            }
            let rng = c.rng.clone();
            for item in HOUSEHOLD_ITEMS {
                let before = c.clone();
                c.apply(Action::Sell(item)).unwrap();
                assert_eq!(c.balance_cents - before.balance_cents, item.sale_cents());
                assert_eq!(c.clock.pa_spent, before.clock.pa_spent + 1);
                assert!(!c.household.owns(item));
                assert_eq!(c.ledger.last().unwrap().kind, LedgerKind::Sale(item));
                assert_eq!(
                    c.household.food_portions,
                    if c.household.owns(HouseholdItem::Fridge) {
                        3
                    } else {
                        0
                    }
                );
                let sold = c.clone();
                assert!(c.apply(Action::Sell(item)).is_err());
                assert_eq!(c, sold);
            }
            assert_eq!(c.balance_cents, INITIAL_BALANCE_CENTS + 40_000);
            assert_eq!(c.daily_finances(1).unwrap().sales_cents, 40_000);
            assert_eq!(c.rng, rng);
            let before = c.clone();
            assert!(c.apply(Action::Sell(HouseholdItem::Pipoca)).is_err());
            assert_eq!(c, before);
            let restored = decode_campaign(&encode_campaign(&c).unwrap()).unwrap();
            assert_eq!(restored, c);
            for action in [Action::Cook, Action::Groceries, Action::WatchTv] {
                assert!(c.apply(action).is_err());
                assert_eq!(c, before);
            }
        }
    }

    #[test]
    fn dog_sale_needs_no_sale_pressure_after_six_sales_and_ends_without_elapsed_unused_pa() {
        let mut c = offered();
        for item in HOUSEHOLD_ITEMS {
            c.apply(Action::Sell(item)).unwrap();
        }
        c.apply(Action::Sleep).unwrap();
        c.apply(Action::AcknowledgeReport).unwrap();
        c.attributes.stress = 0;
        c.attributes.urge = 0;
        let before = c.clone();
        c.apply(Action::Sell(HouseholdItem::Pipoca)).unwrap();
        assert_eq!(c.stage, CampaignStage::Ended(Ending::DogSale));
        assert_eq!(
            c.clock,
            Clock {
                day: 2,
                pa_spent: 1
            }
        );
        assert_eq!(c.balance_cents, before.balance_cents + 20_000);
        assert_eq!(c.attributes.hunger, (before.attributes.hunger + 8).min(100));
        assert_eq!(c.reports, before.reports);
        assert_eq!(c.rng, before.rng);
        assert_eq!(decode_campaign(&encode_campaign(&c).unwrap()).unwrap(), c);
        let ended = c.clone();
        for action in [
            Action::Wait,
            Action::TimeWithPipoca,
            Action::Sell(HouseholdItem::Pipoca),
        ] {
            assert!(c.apply(action).is_err());
            assert_eq!(c, ended);
        }
    }

    #[test]
    fn pipoca_time_uses_shared_relief_helpers_and_household_load_validation_rejects_forged_sales() {
        let mut c = Campaign::new(5);
        c.apply(Action::TimeWithPipoca).unwrap();
        assert_eq!(
            (c.clock.pa_spent, c.attributes.stress, c.attributes.boredom),
            (1, 22, 28)
        );
        c.attributes.stress = 2;
        c.attributes.boredom = 0;
        c.apply(Action::TimeWithPipoca).unwrap();
        assert_eq!((c.attributes.stress, c.attributes.boredom), (0, 0));
        c.clock.pa_spent = 6;
        let before = c.clone();
        assert!(c.apply(Action::TimeWithPipoca).is_err());
        assert_eq!(c, before);
        let mut relieved = Campaign::new(5);
        relieved.attributes.stress = 9;
        relieved.attributes.boredom = 2;
        for _ in 0..3 {
            let mut forecast = relieved.rng.clone();
            let choice = if forecast.sample_below(100).unwrap() < 50 {
                BetChoice::RouletteRed
            } else {
                BetChoice::RouletteBlack
            };
            relieved
                .apply(Action::Bet {
                    choice,
                    stake_cents: 100,
                })
                .unwrap();
        }
        let before = relieved.clone();
        relieved.apply(Action::TimeWithPipoca).unwrap();
        assert_eq!(
            (relieved.attributes.stress, relieved.attributes.boredom),
            (1, 3)
        );
        assert!(relieved.betting.relief.is_none());
        assert_eq!(relieved.betting.history, before.betting.history);
        assert_eq!(relieved.rng, before.rng);
        for item in HOUSEHOLD_ITEMS {
            let mut forged = offered();
            forged.household.sold.push(item);
            forged.household.food_portions = 0;
            assert!(forged.validate().is_err());
            forged
                .record_money(LedgerKind::Sale(item), item.sale_cents() + 1)
                .unwrap();
            assert!(forged.validate().is_err());
        }
        let mut forged = offered();
        forged.stage = CampaignStage::Ended(Ending::DogSale);
        assert!(forged.validate().is_err());
    }

    #[test]
    fn actual_apartment_mesh_colliders_disable_on_sale_and_restore_for_title_new_game_and_load() {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            TransformPlugin,
            RapierPhysicsPlugin::<NoUserData>::default().in_fixed_schedule(),
        ))
        .insert_resource(State::new(GameState::Playing))
        .insert_resource(CampaignStore::for_test(
            Some(offered()),
            SnapshotState::Protected,
        ))
        .init_resource::<Presentation>()
        .add_systems(Update, (scan_furniture, sync_visuals).chain());
        let root = app
            .world_mut()
            .spawn((Apartment, Transform::default(), Visibility::Inherited))
            .id();
        let bytes = std::fs::read("assets/models/apartment_windows.glb").unwrap();
        let gltf = bevy::gltf::gltf::Gltf::from_slice(&bytes).unwrap();
        let mut entities = vec![];
        for node in gltf
            .nodes()
            .filter(|n| n.name().and_then(item_for_name).is_some())
        {
            let name = node.name().unwrap();
            let group = app
                .world_mut()
                .spawn((
                    Name::new(name.to_owned()),
                    ChildOf(root),
                    Transform::from_matrix(Mat4::from_cols_array_2d(&node.transform().matrix())),
                    Visibility::Inherited,
                ))
                .id();
            for primitive in node.mesh().unwrap().primitives() {
                let reader = primitive.reader(|_| gltf.blob.as_deref());
                let vertices = reader.read_positions().unwrap().map(Vec3::from).collect();
                let indices: Vec<_> = reader.read_indices().unwrap().into_u32().collect();
                let collider = Collider::trimesh(
                    vertices,
                    indices
                        .chunks_exact(3)
                        .map(|i| [i[0], i[1], i[2]])
                        .collect(),
                )
                .unwrap();
                let entity = app
                    .world_mut()
                    .spawn((
                        ChildOf(group),
                        Mesh3d::default(),
                        Transform::default(),
                        Visibility::Inherited,
                        collider,
                    ))
                    .id();
                entities.push((entity, item_for_name(name).unwrap()));
            }
        }
        assert!(entities.len() >= 22);
        let sleeper = app
            .world_mut()
            .spawn((
                Name::new("Apartment_Couch.001"),
                Mesh3d::default(),
                Visibility::Inherited,
            ))
            .id();
        app.finish();
        app.cleanup();
        let step = |app: &mut App| {
            app.update();
            for _ in 0..3 {
                app.world_mut().run_schedule(FixedUpdate);
            }
        };
        let enabled = |app: &mut App, entity: Entity| {
            let handle = app.world().get::<RapierColliderHandle>(entity).unwrap().0;
            let mut query = app.world_mut().query::<&RapierContextColliders>();
            query
                .single(app.world())
                .unwrap()
                .colliders
                .get(handle)
                .unwrap()
                .is_enabled()
        };
        let couch_hit = |app: &mut App| {
            app.world_mut()
                .run_system_once(|context: ReadRapierContext| {
                    context
                        .single()
                        .unwrap()
                        .cast_ray(
                            Vec3::new(4.0, 0.5, -1.0),
                            Vec3::NEG_X,
                            2.0,
                            true,
                            QueryFilter::default(),
                        )
                        .is_some()
                })
                .unwrap()
        };
        step(&mut app);
        assert!(couch_hit(&mut app));
        for item in HOUSEHOLD_ITEMS {
            app.world_mut()
                .resource_mut::<CampaignStore>()
                .apply(Action::Sell(item))
                .unwrap();
        }
        step(&mut app);
        for (entity, _) in &entities {
            assert_eq!(
                *app.world().get::<Visibility>(*entity).unwrap(),
                Visibility::Hidden
            );
            assert!(!enabled(&mut app, *entity));
        }
        assert!(!couch_hit(&mut app));
        assert!(app.world().get::<HouseholdVisual>(sleeper).is_none());
        assert_eq!(
            *app.world().get::<Visibility>(sleeper).unwrap(),
            Visibility::Inherited
        );
        let saved =
            encode_campaign(app.world().resource::<CampaignStore>().campaign().unwrap()).unwrap();
        app.insert_resource(State::new(GameState::Title));
        step(&mut app);
        for (entity, _) in &entities {
            assert!(enabled(&mut app, *entity));
        }
        app.insert_resource(CampaignStore::for_test(
            Some(Campaign::new(7)),
            SnapshotState::Protected,
        ));
        app.insert_resource(State::new(GameState::Intro));
        step(&mut app);
        assert!(!app.world().resource::<Presentation>().title);
        for (entity, _) in &entities {
            assert!(enabled(&mut app, *entity));
        }
        app.insert_resource(State::new(GameState::Playing));
        app.insert_resource(CampaignStore::for_test(
            Some(decode_campaign(&saved).unwrap()),
            SnapshotState::Protected,
        ));
        step(&mut app);
        for (entity, _) in &entities {
            assert!(!enabled(&mut app, *entity));
        }
    }

    #[test]
    fn incandescent_flicker_stays_subtle_and_smooth() {
        let mut previous = bulb_brightness(0.0);
        let mut dimmest = previous;
        let mut brightest = previous;
        for frame in 1..3600 {
            let brightness = bulb_brightness(frame as f32 / 60.0);
            assert!((0.946..=1.0191).contains(&brightness));
            assert!((brightness - previous).abs() < 0.009);
            dimmest = dimmest.min(brightness);
            brightest = brightest.max(brightness);
            previous = brightness;
        }
        assert!(brightest - dimmest > 0.04);
    }

    #[test]
    fn environment_tracks_actual_state_and_never_changes_campaign_or_rng() {
        let mut c = Campaign::new(5);
        for value in [0, 40, 60, 80, 100] {
            assert!(pressure(value) >= 0.0 && pressure(value) <= 1.0);
            c.attributes.stress = value;
            c.attributes.urge = 100;
            assert_eq!(thought(&c, &Localization::default()), "");
        }
        c.apply(Action::Bet {
            choice: BetChoice::Slots,
            stake_cents: 100,
        })
        .unwrap();
        c.attributes.urge = 59;
        assert_eq!(thought(&c, &Localization::default()), "");
        c.attributes.urge = 60;
        assert!(!thought(&c, &Localization::default()).is_empty());
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .insert_resource(State::new(GameState::Playing))
            .insert_resource(CampaignStore::for_test(
                Some(c.clone()),
                SnapshotState::Protected,
            ))
            .init_resource::<Presentation>()
            .init_resource::<GameplayOverlay>()
            .init_resource::<Localization>()
            .add_systems(Update, environment);
        let interior = app
            .world_mut()
            .spawn((
                InteriorLight,
                SpotLight {
                    intensity: 40_000.0,
                    shadow_maps_enabled: false,
                    ..default()
                },
            ))
            .id();
        let corridor = app
            .world_mut()
            .spawn(SpotLight {
                intensity: 40_000.0,
                ..default()
            })
            .id();
        app.update();
        assert!(app.world().get::<SpotLight>(interior).unwrap().intensity < 40_000.0);
        assert!(
            !app.world()
                .get::<SpotLight>(interior)
                .unwrap()
                .shadow_maps_enabled
        );
        assert_eq!(
            app.world().get::<SpotLight>(corridor).unwrap().intensity,
            40_000.0
        );
        assert_eq!(
            app.world().resource::<CampaignStore>().campaign().unwrap(),
            &c
        );
        c.clock.day = 4;
        c.attributes.stress = 0;
        app.insert_resource(CampaignStore::for_test(Some(c), SnapshotState::Protected));
        app.update();
        assert_eq!(
            app.world().get::<SpotLight>(interior).unwrap().intensity,
            0.0
        );
        app.world_mut().resource_mut::<Presentation>().title = true;
        app.update();
        assert_eq!(
            app.world().get::<SpotLight>(interior).unwrap().intensity,
            40_000.0 * bulb_brightness(app.world().resource::<Time<Real>>().elapsed_secs())
        );
        let fresh = Campaign::new(5);
        assert!(!visual_visible(HouseholdVisual::Food(1), None, false));
        assert!(visual_visible(
            HouseholdVisual::Food(3),
            Some(&fresh),
            false
        ));
        assert!(!visual_visible(
            HouseholdVisual::Food(4),
            Some(&fresh),
            false
        ));
        assert!(!visual_visible(
            HouseholdVisual::Waste(1),
            Some(&fresh),
            false
        ));
        assert!(!visual_visible(
            HouseholdVisual::Owned(HouseholdItem::Pipoca),
            Some(&fresh),
            true
        ));
        for name in ["Apartment_Bed_01.001", "Apartment_Shelf.002", "Title_Couch"] {
            assert!(item_for_name(name).is_none());
        }
    }
}
