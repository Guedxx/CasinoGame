use bevy::{color::Mix, light::CascadeShadowConfigBuilder, prelude::*};

use crate::{
    game::{camera::MainCamera, campaign::Period, campaign_save::CampaignStore, routine::spawn_bills},
    state::GameState,
};

const MORNING_SUN: f32 = 3_000.0;
const MORNING_AMBIENT: f32 = 180.0;
const BLACKOUT_AMBIENT_FACTOR: f32 = 0.25;
const MORNING_SKY: Color = Color::srgb(0.36, 0.40, 0.44);
const TRANSITION_SECS: f32 = 1.5;

#[derive(Component)]
struct Sunlight;

pub struct PlayingPlugin;

impl Plugin for PlayingPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ClearColor(MORNING_SKY))
            .insert_resource(GlobalAmbientLight {
                brightness: MORNING_AMBIENT,
                color: Color::srgb(0.78, 0.84, 0.94),
                ..default()
            })
            .add_systems(Startup, (setup, spawn_bills))
            .add_systems(PostUpdate, daylight)
            .add_systems(Update, return_to_menu.run_if(in_state(GameState::Playing)));
    }
}

fn setup(
    mut commands: Commands,
    assets: Res<AssetServer>,
    apartment: Query<(), With<crate::game::player::Apartment>>,
) {
    if !apartment.is_empty() {
        return;
    }
    commands.spawn((
        Name::new("Apartment and window scenery"),
        crate::game::player::Apartment,
        WorldAssetRoot(
            assets.load(GltfAssetLabel::Scene(0).from_asset("models/apartment_windows.glb")),
        ),
    ));
    commands.spawn((
        Name::new("Broom by the bathroom back wall"),
        WorldAssetRoot(assets.load(GltfAssetLabel::Scene(0).from_asset("models/PSXBroom.glb"))),
        // shortcut: fitted to the baked bathroom geometry, recalibrate if the room or model changes.
        Transform::from_xyz(1.93, 0.746, -4.30)
            .with_rotation(
                Quat::from_rotation_x(-0.08)
                    * Quat::from_rotation_y(std::f32::consts::FRAC_PI_2),
            )
            .with_scale(Vec3::splat(0.21)),
    ));
    commands.spawn((
        Sunlight,
        DirectionalLight {
            illuminance: MORNING_SUN,
            color: Color::srgb(0.78, 0.84, 0.94),
            shadow_maps_enabled: true,
            shadow_normal_bias: 0.4,
            ..default()
        },
        CascadeShadowConfigBuilder {
            num_cascades: 1,
            maximum_distance: 40.0,
            ..default()
        }
        .build(),
        Transform::from_xyz(-3.0, 8.0, -2.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    for (index, position) in [
        Vec3::new(1.761, 2.48, -0.491),
        Vec3::new(1.761, 2.48, 4.773),
        Vec3::new(1.761, 2.48, 7.848),
        // Preserve the warm light at the closed door seam without loading the corridor.
        Vec3::new(-1.25, 2.48, -2.5),
        Vec3::new(-1.25, 2.48, 3.5),
        Vec3::new(-1.25, 2.48, 7.5),
    ]
    .into_iter()
    .enumerate()
    {
        let mut light = commands.spawn((
            SpotLight {
                color: Color::srgb(1.0, 0.84, 0.62),
                intensity: 40_000.0,
                range: 7.0,
                inner_angle: 0.8,
                outer_angle: 1.3,
                shadow_maps_enabled: true,
                ..default()
            },
            Transform::from_translation(position).looking_to(Vec3::NEG_Y, Vec3::Z),
        ));
        if index < 3 {
            light.insert(crate::game::household::InteriorLight);
        }
    }
}

fn daylight(
    state: Res<State<GameState>>,
    store: Res<CampaignStore>,
    time: Res<Time<Virtual>>,
    mut transition: Local<(Option<(Period, bool)>, f32)>,
    mut sky: ResMut<ClearColor>,
    mut ambient: ResMut<GlobalAmbientLight>,
    mut sun: Query<&mut DirectionalLight, With<Sunlight>>,
    mut fog: Query<&mut DistanceFog, With<MainCamera>>,
) {
    let (previous, remaining) = &mut *transition;
    let (period, powered, blend) = match state.get() {
        GameState::Playing => {
            let (period, powered) = store.campaign().map_or((Period::Morning, true), |c| {
                (c.clock.period(), c.has_power())
            });
            let blend = match previous.replace((period, powered)) {
                None => 1.0,
                Some(last) => {
                    if last != (period, powered) {
                        *remaining = TRANSITION_SECS;
                    }
                    if *remaining == 0.0 {
                        return;
                    }
                    let step = time.delta_secs().min(*remaining);
                    let blend = step / *remaining;
                    *remaining -= step;
                    blend
                }
            };
            (period, powered, blend)
        }
        GameState::Title | GameState::Intro => {
            *previous = None;
            *remaining = 0.0;
            (Period::Morning, true, 1.0)
        }
        GameState::Menu | GameState::Settings => return,
    };
    let (sun_target, mut ambient_target, sky_target) = match period {
        Period::Morning => (MORNING_SUN, MORNING_AMBIENT, MORNING_SKY),
        Period::Afternoon => (1_350.0, 110.0, Color::srgb(0.29, 0.31, 0.35)),
        Period::Night => (90.0, 34.0, Color::srgb(0.07, 0.10, 0.16)),
    };
    if !powered {
        ambient_target *= BLACKOUT_AMBIENT_FACTOR;
    }
    let value = |current, target| current * (1.0 - blend) + target * blend;
    let color = if blend == 1.0 {
        sky_target
    } else {
        sky.0.mix(&sky_target, blend)
    };
    if sky.0 != color {
        sky.0 = color;
    }
    if ambient.brightness != ambient_target {
        ambient.brightness = value(ambient.brightness, ambient_target);
    }
    if let Ok(mut sun) = sun.single_mut()
        && sun.illuminance != sun_target
    {
        sun.illuminance = value(sun.illuminance, sun_target);
    }
    if let Ok(mut fog) = fog.single_mut()
        && fog.color != color
    {
        fog.color = color;
    }
}

fn return_to_menu(
    keys: Res<ButtonInput<KeyCode>>,
    overlay: Res<crate::game::routine::GameplayOverlay>,
    mut next: ResMut<NextState<GameState>>,
) {
    if keys.just_pressed(KeyCode::Escape) && !overlay.escape_consumed && !overlay.blocks_world() {
        next.set(GameState::Menu);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::{
        campaign::{Action, BillKind, Campaign},
        campaign_save::SnapshotState,
    };
    use std::time::Duration;

    #[test]
    fn exterior_fades_with_the_campaign_and_retains_indoor_lights() {
        let mut app = App::new();
        app.insert_resource(State::new(GameState::Playing))
            .insert_resource(CampaignStore::for_test(
                Some(Campaign::new(5)),
                SnapshotState::Protected,
            ))
            .init_resource::<Time<Virtual>>()
            .init_resource::<ClearColor>()
            .init_resource::<GlobalAmbientLight>()
            .add_systems(Update, daylight);
        let sun = app
            .world_mut()
            .spawn((Sunlight, DirectionalLight::default()))
            .id();
        let camera = app
            .world_mut()
            .spawn((MainCamera, DistanceFog::default()))
            .id();
        let bulb = app
            .world_mut()
            .spawn(SpotLight {
                intensity: 40_000.0,
                ..default()
            })
            .id();
        let frame = |app: &mut App, seconds| {
            app.world_mut()
                .resource_mut::<Time<Virtual>>()
                .advance_by(Duration::from_secs_f32(seconds));
            app.update();
            let world = app.world();
            let sky = world.resource::<ClearColor>().0;
            assert_eq!(world.get::<DistanceFog>(camera).unwrap().color, sky);
            (world.get::<DirectionalLight>(sun).unwrap().illuminance, sky)
        };
        let advance_period = |app: &mut App| {
            for _ in 0..2 {
                app.world_mut()
                    .resource_mut::<CampaignStore>()
                    .apply(Action::Wait)
                    .unwrap();
            }
        };
        let morning = frame(&mut app, 0.0);
        advance_period(&mut app);
        let halfway = frame(&mut app, TRANSITION_SECS / 2.0);
        let afternoon = frame(&mut app, TRANSITION_SECS / 2.0);
        assert!(afternoon.0 < halfway.0 && halfway.0 < morning.0);
        advance_period(&mut app);
        let halfway = frame(&mut app, TRANSITION_SECS / 2.0);
        app.insert_resource(State::new(GameState::Menu));
        assert_eq!(frame(&mut app, TRANSITION_SECS), halfway);
        app.insert_resource(State::new(GameState::Playing));
        let night = frame(&mut app, TRANSITION_SECS / 2.0);
        assert!(night.0 < halfway.0 && halfway.0 < afternoon.0);
        assert!(night.1.to_srgba().blue < afternoon.1.to_srgba().blue);
        assert!(app.world().resource::<GlobalAmbientLight>().brightness < MORNING_AMBIENT);

        app.insert_resource(State::new(GameState::Title));
        assert_eq!(frame(&mut app, 0.0), morning);
        app.insert_resource(State::new(GameState::Playing));
        assert_eq!(frame(&mut app, 0.0), night);
        for action in [Action::Sleep, Action::AcknowledgeReport] {
            app.world_mut()
                .resource_mut::<CampaignStore>()
                .apply(action)
                .unwrap();
        }
        assert_eq!(frame(&mut app, TRANSITION_SECS), morning);
        assert_eq!(
            app.world().get::<SpotLight>(bulb).unwrap().intensity,
            40_000.0
        );

        {
            let mut store = app.world_mut().resource_mut::<CampaignStore>();
            store.apply(Action::Work).unwrap();
            for _ in 2..4 {
                store.apply(Action::Sleep).unwrap();
                store.apply(Action::AcknowledgeReport).unwrap();
            }
        }
        assert_eq!(frame(&mut app, TRANSITION_SECS), morning);
        assert_eq!(
            app.world().resource::<GlobalAmbientLight>().brightness,
            MORNING_AMBIENT * BLACKOUT_AMBIENT_FACTOR
        );
        app.world_mut()
            .resource_mut::<CampaignStore>()
            .apply(Action::PayBill(BillKind::Electricity))
            .unwrap();
        assert_eq!(frame(&mut app, TRANSITION_SECS), morning);
        assert_eq!(app.world().resource::<GlobalAmbientLight>().brightness, MORNING_AMBIENT);
    }
}
