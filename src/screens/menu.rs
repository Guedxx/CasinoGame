use bevy::{
    asset::{LoadState, RecursiveDependencyLoadState},
    image::{ImageLoaderSettings, ImageSampler},
    input::mouse::{MouseScrollUnit, MouseWheel},
    input_focus::{
        FocusCause, InputFocus,
        tab_navigation::{TabGroup, TabIndex, TabNavigationPlugin},
    },
    prelude::*,
    world_serialization::{WorldInstance, WorldInstanceSpawner},
};

use super::title_render::TitleRender;
use crate::{
    graphics::{AntiAliasing, ApplyGraphics, GraphicsOptions, GraphicsSettings, Quality},
    state::GameState,
};

pub struct MenuPlugin;
impl Plugin for MenuPlugin {
    fn build(&self, app: &mut App) {
        super::title_render::install(app);
        app.add_plugins(TabNavigationPlugin)
            .init_resource::<SettingsOrigin>()
            .init_resource::<NewGameConfirmation>()
            .init_resource::<MenuSkin>()
            .add_systems(OnEnter(GameState::Title), (spawn_sleeper, setup_title))
            .add_systems(OnEnter(GameState::Menu), setup)
            .add_systems(OnEnter(GameState::Settings), setup)
            .add_systems(OnEnter(GameState::Playing), remove_sleeper)
            .add_systems(
                Update,
                (
                    actions.before(ApplyGraphics),
                    (
                        appearance,
                        status,
                        scroll,
                        breathe,
                        title_loading,
                        title_controls,
                        frame_title,
                    )
                        .after(ApplyGraphics),
                )
                    .run_if(not(in_state(GameState::Playing))),
            );
    }
}

const INK: Color = Color::srgb(0.91, 0.89, 0.83);
const MUTED: Color = Color::srgb(0.65, 0.66, 0.63);
const ACCENT: Color = Color::srgb(0.79, 0.64, 0.40);

#[derive(Component, Clone, Copy)]
enum Action {
    Start,
    NewGame,
    ConfirmNewGame,
    CancelNewGame,
    RetryCampaignSave,
    Resume,
    Settings,
    Save,
    Cancel,
    Reset,
    Preset(Quality),
    Lighting(Quality),
    AntiAliasing(AntiAliasing),
    TextureWarping(bool),
    VertexJitter(bool),
}
impl Action {
    fn selected(self, settings: GraphicsSettings) -> bool {
        match self {
            Self::Preset(q) => settings == GraphicsSettings::preset(q),
            Self::Lighting(q) => settings.lighting == q,
            Self::AntiAliasing(mode) => settings.anti_aliasing == mode,
            Self::TextureWarping(enabled) => settings.texture_warping == enabled,
            Self::VertexJitter(enabled) => settings.vertex_jitter == enabled,
            _ => false,
        }
    }
    fn setting_name(self) -> Option<&'static str> {
        match self {
            Self::Preset(_) => Some("Quality preset"),
            Self::Lighting(_) => Some("Lighting quality"),
            Self::AntiAliasing(_) => Some("Anti-aliasing"),
            Self::TextureWarping(_) => Some("Texture warping"),
            Self::VertexJitter(_) => Some("Model jitter"),
            _ => None,
        }
    }
}
#[derive(Component)]
struct ScrollPanel;
#[derive(Component)]
struct Status;
#[derive(Component)]
struct Fps;
#[derive(Component)]
struct Sleeper;
#[derive(Component)]
struct TitleStatus;
#[derive(Component)]
struct TitleLoadingCover;
#[derive(Component)]
struct TitleHeading;
#[derive(Component)]
struct TitleControls;
#[derive(Resource, Default)]
struct NewGameConfirmation(bool);

#[derive(Resource)]
struct SettingsOrigin(GameState);
impl Default for SettingsOrigin {
    fn default() -> Self {
        Self(GameState::Title)
    }
}

fn spawn_sleeper(
    mut commands: Commands,
    assets: Res<AssetServer>,
    sleepers: Query<(), With<Sleeper>>,
    mut cameras: Query<&mut Transform, With<crate::game::camera::MainCamera>>,
    mut render: ResMut<TitleRender>,
) {
    *render = default();
    if sleepers.is_empty() {
        commands.spawn((
            Name::new("Title sleeper"),
            Sleeper,
            WorldAssetRoot(
                assets.load(GltfAssetLabel::Scene(0).from_asset("models/menu_sleeper.glb")),
            ),
        ));
    }
    for mut camera in &mut cameras {
        *camera =
            Transform::from_xyz(0.90, 1.35, -2.60).looking_at(Vec3::new(3.0, 0.70, 1.0), Vec3::Y);
    }
}

fn remove_sleeper(
    mut commands: Commands,
    sleepers: Query<Entity, With<Sleeper>>,
    mut projections: Query<&mut Projection, With<crate::game::camera::MainCamera>>,
) {
    for entity in &sleepers {
        commands.entity(entity).despawn();
    }
    for mut projection in &mut projections {
        if let Projection::Perspective(projection) = &mut *projection {
            projection.fov = 75.0_f32.to_radians();
        }
    }
}

fn breath_weight(seconds: f32) -> f32 {
    (1.0 - (seconds * std::f32::consts::TAU / 4.0).cos()) * 0.5
}

fn frame_title(
    sleepers: Query<(), With<Sleeper>>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    mut projections: Query<&mut Projection, With<crate::game::camera::MainCamera>>,
    mut headings: Query<&mut TextFont, With<TitleHeading>>,
) {
    let Ok(window) = windows.single() else { return };
    if sleepers.is_empty() {
        return;
    }
    // Preserve the horizontal composition when the window aspect ratio changes.
    for mut projection in &mut projections {
        if let Projection::Perspective(projection) = &mut *projection {
            projection.fov = 2.0
                * ((70.0_f32.to_radians() / 2.0).tan() * (16.0 / 9.0) * window.height()
                    / window.width().max(1.0))
                .atan();
        }
    }
    for mut heading in &mut headings {
        heading.font_size = FontSize::Px((window.width() * 0.05).clamp(32.0, 64.0));
    }
}

fn breathe(
    time: Res<Time<Real>>,
    sleepers: Query<Entity, With<Sleeper>>,
    children: Query<&Children>,
    mut weights: Query<&mut bevy::mesh::morph::MorphWeights>,
) {
    for root in &sleepers {
        for entity in children.iter_descendants(root) {
            if let Ok(mut weights) = weights.get_mut(entity) {
                weights.weights_mut()[0] = breath_weight(time.elapsed_secs());
            }
        }
    }
}

fn title_loading(
    assets: Res<AssetServer>,
    spawner: Res<WorldInstanceSpawner>,
    scenes: Query<
        (&WorldAssetRoot, Option<&WorldInstance>),
        Or<(With<Sleeper>, With<crate::game::player::Apartment>)>,
    >,
    campaign: Res<crate::game::campaign_save::CampaignStore>,
    mut texts: Query<&mut Text, With<TitleStatus>>,
    mut covers: Query<&mut Node, With<TitleLoadingCover>>,
    meshes: Res<Assets<Mesh>>,
    images: Res<Assets<Image>>,
    mut render: ResMut<TitleRender>,
    mut announced: Local<bool>,
) {
    if texts.is_empty() {
        return;
    }
    let mut message = if scenes.iter().count() < 2 {
        "Loading apartment…"
    } else {
        ""
    };
    for (root, instance) in &scenes {
        match assets.get_load_states(root.0.id()) {
            Some((LoadState::Failed(_), _, _))
            | Some((_, _, RecursiveDependencyLoadState::Failed(_))) => {
                message = "Could not load the title scene. Restart the game to retry.";
                break;
            }
            _ if !assets.is_loaded_with_dependencies(root.0.id())
                || instance.is_none_or(|id| !spawner.instance_is_ready(**id)) =>
            {
                message = "Loading apartment…";
            }
            _ => {}
        }
    }
    if message.is_empty() && render.meshes.is_empty() {
        render.meshes = meshes
            .iter()
            .filter(|(_, mesh)| {
                mesh.asset_usage
                    .contains(bevy::asset::RenderAssetUsages::RENDER_WORLD)
            })
            .map(|(id, _)| id)
            .collect();
        render.images = images
            .iter()
            .filter(|(_, image)| {
                image
                    .asset_usage
                    .contains(bevy::asset::RenderAssetUsages::RENDER_WORLD)
            })
            .map(|(id, _)| id)
            .collect();
    }
    if message.is_empty() && !render.is_ready() {
        message = "Preparing apartment…";
    }
    for mut cover in &mut covers {
        cover.display = if render.is_ready() && message.is_empty() {
            Display::None
        } else {
            Display::Flex
        };
    }
    for mut text in &mut texts {
        **text = if campaign.notice.is_empty() {
            message.into()
        } else {
            campaign.notice.clone()
        };
    }
    if message.is_empty() && !*announced {
        info!("Title screen ready");
        *announced = true;
    }
}

#[derive(Resource)]
struct MenuSkin {
    panel: Handle<Image>,
    font: Handle<Font>,
}
impl FromWorld for MenuSkin {
    fn from_world(world: &mut World) -> Self {
        let assets = world.resource::<AssetServer>();
        Self {
            panel: assets
                .load_builder()
                .with_settings(|settings: &mut ImageLoaderSettings| {
                    settings.sampler = ImageSampler::nearest();
                })
                .load("ui/pause-panel.png"),
            font: assets.load("fonts/LiberationSans-Bold.ttf"),
        }
    }
}
impl MenuSkin {
    fn font(&self, size: f32) -> TextFont {
        TextFont {
            font: self.font.clone().into(),
            font_size: FontSize::Px(size),
            ..default()
        }
    }
    fn image(&self, corners: f32) -> ImageNode {
        ImageNode {
            image: self.panel.clone(),
            image_mode: bevy::ui::widget::NodeImageMode::Sliced(TextureSlicer {
                border: BorderRect::all(11.0),
                center_scale_mode: SliceScaleMode::Stretch,
                sides_scale_mode: SliceScaleMode::Stretch,
                max_corner_scale: corners * 4.0,
            }),
            ..default()
        }
    }
}
fn label(parent: &mut ChildSpawnerCommands, skin: &MenuSkin, text: &str, size: f32, color: Color) {
    parent.spawn((Text::new(text), skin.font(size), TextColor(color)));
}
fn button(
    parent: &mut ChildSpawnerCommands,
    skin: &MenuSkin,
    text: &str,
    action: Action,
    index: &mut i32,
) -> Entity {
    let main = matches!(
        action,
        Action::Start
            | Action::NewGame
            | Action::ConfirmNewGame
            | Action::Resume
            | Action::Settings
    );
    let entity = parent
        .spawn((
            Button,
            action,
            TabIndex(*index),
            skin.image(0.35),
            Node {
                min_height: px(if main { 66 } else { 54 }),
                flex_shrink: 0.0,
                flex_grow: if main || matches!(action, Action::Reset) {
                    0.0
                } else {
                    1.0
                },
                border: UiRect::all(px(2)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BorderColor::all(Color::NONE),
            children![(
                Text::new(text),
                skin.font(24.0),
                TextColor(INK),
                Node {
                    margin: UiRect::axes(px(18), px(12)),
                    ..default()
                }
            )],
        ))
        .id();
    *index += 1;
    entity
}
fn choices(
    parent: &mut ChildSpawnerCommands,
    skin: &MenuSkin,
    title: &str,
    items: &[(&str, Action)],
    index: &mut i32,
) {
    parent
        .spawn(Node {
            flex_wrap: FlexWrap::Wrap,
            column_gap: px(20),
            row_gap: px(10),
            align_items: AlignItems::Center,
            flex_shrink: 0.0,
            ..default()
        })
        .with_children(|row| {
            row.spawn(Node {
                flex_direction: FlexDirection::Column,
                row_gap: px(5),
                flex_basis: percent(38),
                min_width: px(230),
                max_width: percent(100),
                flex_grow: 1.0,
                ..default()
            })
            .with_children(|copy| {
                label(copy, skin, title, 24.0, INK);
            });
            row.spawn(Node {
                column_gap: px(8),
                row_gap: px(8),
                flex_wrap: FlexWrap::Wrap,
                flex_basis: percent(55),
                min_width: px(320),
                max_width: percent(100),
                flex_grow: 1.0,
                ..default()
            })
            .with_children(|buttons| {
                for (text, action) in items {
                    button(buttons, skin, text, *action, index);
                }
            });
        });
}
// The apartment is the title art: sleeper left, warm ivory type and familiar controls right.
fn setup_title(mut commands: Commands, skin: Res<MenuSkin>, mut focus: ResMut<InputFocus>) {
    commands.spawn((
        DespawnOnExit(GameState::Title),
        TitleLoadingCover,
        GlobalZIndex(1000),
        BackgroundColor(Color::BLACK),
        Node {
            position_type: PositionType::Absolute,
            width: percent(100),
            height: percent(100),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        },
        children![(
            TitleStatus,
            Text::new("Loading apartment…"),
            skin.font(18.0),
            TextColor(INK)
        )],
    ));
    let mut index = 0;
    let mut first = None;
    commands
        .spawn((
            DespawnOnExit(GameState::Title),
            GlobalZIndex(100),
            TabGroup::modal(),
            BackgroundGradient(vec![
                LinearGradient::to_right(vec![
                    ColorStop::new(Color::NONE, percent(30)),
                    ColorStop::new(Color::srgba(0.015, 0.018, 0.016, 0.90), percent(65)),
                    ColorStop::new(Color::srgba(0.015, 0.018, 0.016, 0.96), percent(100)),
                ])
                .into(),
            ]),
            Node {
                width: percent(100),
                height: percent(100),
                justify_content: JustifyContent::End,
                ..default()
            },
        ))
        .with_children(|overlay| {
            overlay
                .spawn(Node {
                    width: percent(45),
                    height: percent(100),
                    padding: UiRect::axes(vw(3), px(24)),
                    justify_content: JustifyContent::Center,
                    flex_direction: FlexDirection::Column,
                    row_gap: px(32),
                    ..default()
                })
                .with_children(|panel| {
                    panel.spawn((
                        Text::new("Let It Ride"),
                        TitleHeading,
                        skin.font(64.0),
                        TextColor(INK),
                        TextShadow::default(),
                    ));
                    panel
                        .spawn((
                            TitleControls,
                            Node {
                                width: percent(100),
                                max_width: px(360),
                                flex_direction: FlexDirection::Column,
                                row_gap: px(14),
                                ..default()
                            },
                        ))
                        .with_children(|buttons| {
                            first = Some(button(
                                buttons,
                                &skin,
                                "Continue",
                                Action::Start,
                                &mut index,
                            ));
                            button(buttons, &skin, "New Game", Action::NewGame, &mut index);
                            button(buttons, &skin, "Settings", Action::Settings, &mut index);
                        });
                    panel.spawn((
                        TitleStatus,
                        Text::new("Loading apartment…"),
                        skin.font(18.0),
                        TextColor(INK),
                    ));
                });
        });
    if let Some(entity) = first {
        focus.set(entity, FocusCause::Navigated);
    }
}

fn title_controls(
    mut commands: Commands,
    skin: Res<MenuSkin>,
    store: Res<crate::game::campaign_save::CampaignStore>,
    confirmation: Res<NewGameConfirmation>,
    roots: Query<(Entity, &Children), With<TitleControls>>,
    mut focus: ResMut<InputFocus>,
    mut previous: Local<String>,
) {
    for (entity, children) in &roots {
        let signature = format!(
            "{entity:?}{}{}{:?}{}",
            confirmation.0,
            store.campaign().is_some(),
            store.snapshot_state(),
            store.notice.is_empty()
        );
        if *previous == signature {
            continue;
        }
        *previous = signature;
        for child in children.iter() {
            commands.entity(child).despawn();
        }
        let mut first = None;
        let mut index = 0;
        commands.entity(entity).with_children(|buttons| {
            if confirmation.0 {
                label(buttons, &skin, "Replace the existing campaign? This permanently replaces saved and unsaved progress, including an unreadable save.", 21.0, INK);
                first = Some(button(buttons, &skin, "Keep existing campaign", Action::CancelNewGame, &mut index));
                button(buttons, &skin, "Replace / New Game", Action::ConfirmNewGame, &mut index);
            } else {
                if store.campaign().is_some() {
                    first = Some(button(buttons, &skin, "Continue", Action::Start, &mut index));
                }
                let new = button(buttons, &skin, "New Game", Action::NewGame, &mut index);
                first.get_or_insert(new);
                button(buttons, &skin, "Settings", Action::Settings, &mut index);
                if store.campaign().is_some() && !store.notice.is_empty() {
                    button(buttons, &skin, "Retry campaign save", Action::RetryCampaignSave, &mut index);
                }
            }
        });
        if let Some(first) = first {
            focus.set(first, FocusCause::Navigated);
        }
    }
}

fn setup(
    mut commands: Commands,
    state: Res<State<GameState>>,
    skin: Res<MenuSkin>,
    mut focus: ResMut<InputFocus>,
) {
    let settings = *state.get() == GameState::Settings;
    let mut first = None;
    let mut index = 0;
    commands
        .spawn((
            DespawnOnExit(*state.get()),
            GlobalZIndex(100),
            TabGroup::modal(),
            Node {
                width: percent(100),
                height: percent(100),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                padding: UiRect::all(px(16)),
                ..default()
            },
            BackgroundColor(Color::srgba(
                0.015,
                0.018,
                0.016,
                if settings { 0.24 } else { 0.48 },
            )),
        ))
        .with_children(|overlay| {
            overlay
                .spawn((
                    skin.image(if settings { 0.7 } else { 1.0 }),
                    Node {
                        width: px(if settings { 960 } else { 600 }),
                        max_width: percent(100),
                        height: if settings { percent(94) } else { auto() },
                        max_height: percent(100),
                        flex_direction: FlexDirection::Column,
                        ..default()
                    },
                ))
                .with_children(|frame| {
                    // ImageNode paints its content box; keep padding on a separate child.
                    frame
                        .spawn(Node {
                            width: percent(100),
                            height: if settings { percent(100) } else { auto() },
                            padding: UiRect::all(px(if settings { 32 } else { 56 })),
                            flex_direction: FlexDirection::Column,
                            row_gap: px(if settings { 12 } else { 24 }),
                            ..default()
                        })
                        .with_children(|panel| {
                            panel
                                .spawn(Node {
                                    flex_direction: FlexDirection::Column,
                                    align_items: AlignItems::Center,
                                    row_gap: px(8),
                                    flex_shrink: 0.0,
                                    ..default()
                                })
                                .with_children(|heading| {
                                    label(
                                        heading,
                                        &skin,
                                        if settings { "GRAPHICS" } else { "PAUSED" },
                                        48.0,
                                        INK,
                                    );
                                });
                            panel
                                .spawn((
                                    ScrollPanel,
                                    ScrollPosition::default(),
                                    Node {
                                        flex_direction: FlexDirection::Column,
                                        row_gap: px(if settings { 18 } else { 14 }),
                                        flex_grow: if settings { 1.0 } else { 0.0 },
                                        min_height: px(0),
                                        overflow: Overflow::scroll_y(),
                                        ..default()
                                    },
                                ))
                                .with_children(|body| {
                                    if settings {
                                        body.spawn(Node {
                                            flex_direction: FlexDirection::Column,
                                            row_gap: px(10),
                                            flex_shrink: 0.0,
                                            ..default()
                                        })
                                        .with_children(
                                            |presets| {
                                                label(presets, &skin, "Quality preset", 24.0, INK);
                                                presets
                                                    .spawn(Node {
                                                        width: percent(100),
                                                        column_gap: px(10),
                                                        ..default()
                                                    })
                                                    .with_children(|row| {
                                                        for quality in Quality::ALL {
                                                            let entity = button(
                                                                row,
                                                                &skin,
                                                                quality.label(),
                                                                Action::Preset(quality),
                                                                &mut index,
                                                            );
                                                            first.get_or_insert(entity);
                                                        }
                                                    });
                                            },
                                        );
                                        choices(
                                            body,
                                            &skin,
                                            "Lighting",
                                            &[
                                                ("Low", Action::Lighting(Quality::Low)),
                                                ("Medium", Action::Lighting(Quality::Medium)),
                                                ("High", Action::Lighting(Quality::High)),
                                            ],
                                            &mut index,
                                        );
                                        choices(
                                            body,
                                            &skin,
                                            "Anti-aliasing",
                                            &[
                                                ("Off", Action::AntiAliasing(AntiAliasing::Off)),
                                                ("FXAA", Action::AntiAliasing(AntiAliasing::Fxaa)),
                                                ("SMAA", Action::AntiAliasing(AntiAliasing::Smaa)),
                                                (
                                                    "4x MSAA",
                                                    Action::AntiAliasing(AntiAliasing::Msaa4),
                                                ),
                                            ],
                                            &mut index,
                                        );
                                        choices(
                                            body,
                                            &skin,
                                            "Texture warping",
                                            &[
                                                ("Off", Action::TextureWarping(false)),
                                                ("On", Action::TextureWarping(true)),
                                            ],
                                            &mut index,
                                        );
                                        choices(
                                            body,
                                            &skin,
                                            "Model jitter",
                                            &[
                                                ("Off", Action::VertexJitter(false)),
                                                ("On", Action::VertexJitter(true)),
                                            ],
                                            &mut index,
                                        );
                                        button(
                                            body,
                                            &skin,
                                            "Restore High defaults",
                                            Action::Reset,
                                            &mut index,
                                        );
                                    } else {
                                        first = Some(button(
                                            body,
                                            &skin,
                                            "RESUME",
                                            Action::Resume,
                                            &mut index,
                                        ));
                                        button(
                                            body,
                                            &skin,
                                            "SETTINGS",
                                            Action::Settings,
                                            &mut index,
                                        );
                                    }
                                });
                            panel
                                .spawn(Node {
                                    flex_direction: FlexDirection::Column,
                                    row_gap: px(10),
                                    flex_shrink: 0.0,
                                    ..default()
                                })
                                .with_children(|footer| {
                                    if settings {
                                        footer
                                            .spawn(Node {
                                                flex_wrap: FlexWrap::Wrap,
                                                column_gap: px(16),
                                                row_gap: px(6),
                                                justify_content: JustifyContent::SpaceBetween,
                                                align_items: AlignItems::Center,
                                                ..default()
                                            })
                                            .with_children(|stats| {
                                                stats.spawn((
                                                    Status,
                                                    Text::new(""),
                                                    skin.font(18.0),
                                                    TextColor(MUTED),
                                                    Node {
                                                        flex_basis: percent(55),
                                                        flex_grow: 1.0,
                                                        min_width: px(200),
                                                        max_width: percent(100),
                                                        ..default()
                                                    },
                                                ));
                                                stats.spawn((
                                                    Fps,
                                                    Text::new(""),
                                                    skin.font(18.0),
                                                    TextColor(ACCENT),
                                                ));
                                            });
                                        footer
                                            .spawn(Node {
                                                column_gap: px(12),
                                                width: percent(100),
                                                ..default()
                                            })
                                            .with_children(|row| {
                                                button(
                                                    row,
                                                    &skin,
                                                    "CANCEL",
                                                    Action::Cancel,
                                                    &mut index,
                                                );
                                                button(
                                                    row,
                                                    &skin,
                                                    "SAVE SETTINGS",
                                                    Action::Save,
                                                    &mut index,
                                                );
                                            });
                                    } else {
                                        footer.spawn((
                                            Status,
                                            Text::new(""),
                                            skin.font(18.0),
                                            TextColor(MUTED),
                                        ));
                                        footer
                                            .spawn(Node {
                                                justify_content: JustifyContent::Center,
                                                ..default()
                                            })
                                            .with_children(|hint| {
                                                label(hint, &skin, "Select Resume to continue. Esc again releases mouse.", 18.0, MUTED);
                                            });
                                    }
                                });
                        });
                });
        });
    if let Some(first) = first {
        focus.set(first, FocusCause::Navigated);
    }
}
fn actions(
    mut commands: Commands,
    mut confirmation: ResMut<NewGameConfirmation>,
    mut focus: ResMut<InputFocus>,
    keys: Res<ButtonInput<KeyCode>>,
    state: Res<State<GameState>>,
    mut origin: ResMut<SettingsOrigin>,
    mut options: ResMut<GraphicsOptions>,
    mut next: ResMut<NextState<GameState>>,
    mut campaign: ResMut<crate::game::campaign_save::CampaignStore>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    buttons: Query<(Entity, &Action, &Interaction), With<Button>>,
    changed: Query<(Entity, &Action, &Interaction), (With<Button>, Changed<Interaction>)>,
    covers: Query<&Node, With<TitleLoadingCover>>,
) {
    if covers.iter().any(|cover| cover.display != Display::None) {
        return;
    }
    if !windows.single().is_ok_and(|window| window.focused) {
        return;
    }
    let action = if keys.just_pressed(KeyCode::Escape) {
        match *state.get() {
            GameState::Title if confirmation.0 => Some(Action::CancelNewGame),
            GameState::Settings => Some(Action::Cancel),
            GameState::Menu => Some(Action::Resume),
            _ => None,
        }
    } else if keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::Space) {
        focus
            .get()
            .and_then(|entity| buttons.get(entity).ok())
            .map(|(_, action, _)| *action)
    } else {
        changed
            .iter()
            .find(|(_, _, interaction)| **interaction == Interaction::Pressed)
            .map(|(entity, action, _)| {
                focus.set(entity, FocusCause::Pressed);
                *action
            })
    };
    let Some(action) = action else { return };
    if confirmation.0 && !matches!(action, Action::ConfirmNewGame | Action::CancelNewGame) {
        return;
    }
    commands.trigger(crate::game::audio::Sound::Click);
    match action {
        Action::Start => {
            // Retained session-only progress is newer than a failed/older snapshot.
            if campaign.campaign().is_some() {
                next.set(GameState::Playing);
            } else if campaign.notice.is_empty() {
                campaign.notice = "No campaign to continue. Choose New Game.".into();
            }
        }
        Action::NewGame | Action::ConfirmNewGame => {
            let confirmed = matches!(action, Action::ConfirmNewGame) && confirmation.0;
            if matches!(action, Action::ConfirmNewGame) && !confirmed {
                return;
            }
            if campaign.needs_replace_confirmation() && !confirmed {
                confirmation.0 = true;
            } else {
                match campaign.new_campaign(confirmed) {
                    Ok(_) => {
                        confirmation.0 = false;
                        commands.trigger(crate::game::campaign_save::CampaignReplaced);
                        next.set(GameState::Playing);
                    }
                    Err(error) => campaign.notice = error,
                }
            }
        }
        Action::CancelNewGame => confirmation.0 = false,
        Action::RetryCampaignSave => {
            let _ = campaign.save();
        }
        Action::Resume => next.set(GameState::Playing),
        Action::Settings => {
            options.cancel();
            origin.0 = *state.get();
            next.set(GameState::Settings);
        }
        Action::Save => {
            options.save();
            next.set(origin.0);
        }
        Action::Cancel => {
            options.cancel();
            next.set(origin.0);
        }
        Action::Reset => options.current = GraphicsSettings::default(),
        Action::Preset(q) => options.current = GraphicsSettings::preset(q),
        Action::Lighting(q) => options.current.lighting = q,
        Action::AntiAliasing(mode) => options.current.anti_aliasing = mode,
        Action::TextureWarping(enabled) => options.current.texture_warping = enabled,
        Action::VertexJitter(enabled) => options.current.vertex_jitter = enabled,
    }
}
fn appearance(
    mut commands: Commands,
    options: Res<GraphicsOptions>,
    focus: Res<InputFocus>,
    mut buttons: Query<(
        Entity,
        &Action,
        &Interaction,
        &mut ImageNode,
        &mut BorderColor,
        Option<&mut bevy::a11y::AccessibilityNode>,
        Has<bevy::ui::Checkable>,
        Has<bevy::ui::Checked>,
    )>,
) {
    for (entity, action, interaction, mut image, mut border, accessible, checkable, checked) in
        &mut buttons
    {
        if let Some(mut accessible) = accessible
            && let Some(name) = action.setting_name()
        {
            accessible.set_description(name);
            if !checkable {
                commands.entity(entity).insert(bevy::ui::Checkable);
            }
            if checked != action.selected(options.current) {
                if action.selected(options.current) {
                    commands.entity(entity).insert(bevy::ui::Checked);
                } else {
                    commands.entity(entity).remove::<bevy::ui::Checked>();
                }
            }
        }
        image.color = if action.selected(options.current) {
            Color::linear_rgb(3.5, 2.8, 1.7)
        } else if *interaction != Interaction::None || focus.get() == Some(entity) {
            Color::linear_rgb(2.0, 2.0, 1.8)
        } else {
            Color::WHITE
        };
        *border = BorderColor::all(if focus.get() == Some(entity) {
            INK
        } else if action.selected(options.current) {
            ACCENT
        } else {
            Color::NONE
        });
    }
}
fn status(
    time: Res<Time<Real>>,
    state: Res<State<GameState>>,
    options: Res<GraphicsOptions>,
    campaign: Res<crate::game::campaign_save::CampaignStore>,
    mut sample: Local<(u32, f64, f64)>,
    mut texts: Query<(&mut Text, Has<Status>, Has<Fps>)>,
) {
    sample.0 += 1;
    sample.1 += time.delta_secs_f64();
    if sample.1 >= 0.5 {
        sample.2 = sample.0 as f64 / sample.1;
        sample.0 = 0;
        sample.1 = 0.0;
    }
    for (mut text, status, fps) in &mut texts {
        if status {
            **text = if !campaign.notice.is_empty() {
                campaign.notice.clone()
            } else if !options.notice.is_empty() {
                options.notice.clone()
            } else if *state.get() == GameState::Settings {
                "Live preview. Cancel restores saved settings.".into()
            } else {
                String::new()
            };
        } else if fps {
            **text = format!("{} | {:.0} FPS", options.current.label(), sample.2);
        }
    }
}
fn scroll(
    mut wheel: MessageReader<MouseWheel>,
    focus: Res<InputFocus>,
    mut panels: Query<(&mut ScrollPosition, &ComputedNode, &UiGlobalTransform), With<ScrollPanel>>,
    buttons: Query<(&ComputedNode, &UiGlobalTransform), With<Button>>,
) {
    let delta: f32 = wheel
        .read()
        .map(|event| {
            -event.y
                * if event.unit == MouseScrollUnit::Line {
                    32.0
                } else {
                    1.0
                }
        })
        .sum();
    for (mut scroll, node, transform) in &mut panels {
        scroll.y += delta;
        if focus.is_changed()
            && let Some((button, position)) =
                focus.get().and_then(|entity| buttons.get(entity).ok())
        {
            let top = transform.translation.y - node.size().y / 2.0;
            let bottom = transform.translation.y + node.size().y / 2.0;
            let button_top = position.translation.y - button.size().y / 2.0;
            let button_bottom = position.translation.y + button.size().y / 2.0;
            if button_top < top {
                scroll.y += (button_top - top) * node.inverse_scale_factor();
            } else if button_bottom > bottom {
                scroll.y += (button_bottom - bottom) * node.inverse_scale_factor();
            }
        }
        scroll.y = scroll.y.clamp(
            0.0,
            ((node.content_size().y - node.size().y) * node.inverse_scale_factor()).max(0.0),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initial_title_can_enter_before_startup() {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            bevy::asset::AssetPlugin::default(),
            bevy::state::app::StatesPlugin,
        ))
        .init_asset::<Image>()
        .init_asset::<Font>()
        .init_asset::<bevy::world_serialization::WorldAsset>()
        .init_resource::<InputFocus>()
        .init_state::<GameState>()
        .add_plugins(MenuPlugin);
        app.world_mut().run_schedule(OnEnter(GameState::Title));
        assert!(app.world().contains_resource::<MenuSkin>());
        assert_eq!(
            app.world_mut()
                .query_filtered::<&BackgroundColor, With<TitleLoadingCover>>()
                .single(app.world())
                .unwrap()
                .0,
            Color::BLACK
        );
        assert_eq!(
            app.world_mut()
                .query_filtered::<Entity, With<TitleHeading>>()
                .iter(app.world())
                .count(),
            1
        );
        assert_eq!(
            app.world_mut()
                .query::<&Action>()
                .iter(app.world())
                .filter(|a| matches!(a, Action::Start))
                .count(),
            1
        );
    }

    fn app(state: GameState) -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::state::app::StatesPlugin))
            .insert_state(state)
            .init_resource::<InputFocus>()
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<SettingsOrigin>()
            .init_resource::<NewGameConfirmation>()
            .insert_resource(crate::game::campaign_save::CampaignStore::for_test(
                Some(crate::game::campaign::Campaign::new(5)),
                crate::game::campaign_save::SnapshotState::Available,
            ))
            .insert_resource(GraphicsOptions {
                current: GraphicsSettings::default(),
                saved: GraphicsSettings::default(),
                notice: String::new(),
            })
            .add_systems(Update, actions)
            .add_systems(OnEnter(GameState::Playing), remove_sleeper);
        app.world_mut().spawn((
            Window {
                focused: true,
                ..default()
            },
            bevy::window::PrimaryWindow,
        ));
        app.update();
        app
    }

    fn key(app: &mut App, key: KeyCode) {
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(key);
        app.update();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .reset(key);
        app.update();
    }

    fn select(app: &mut App, action: Action) {
        let button = app
            .world_mut()
            .spawn((Button, action, Interaction::None))
            .id();
        app.world_mut()
            .resource_mut::<InputFocus>()
            .set(button, FocusCause::Navigated);
        key(app, KeyCode::Enter);
    }

    #[test]
    fn title_requires_start_and_removes_only_the_sleeper() {
        assert_eq!(GameState::default(), GameState::Title);
        let mut app = app(GameState::Title);
        let sleeper = app.world_mut().spawn(Sleeper).id();
        let apartment = app.world_mut().spawn(crate::game::player::Apartment).id();
        key(&mut app, KeyCode::Escape);
        assert_eq!(
            *app.world().resource::<State<GameState>>().get(),
            GameState::Title
        );
        select(&mut app, Action::Start);
        assert_eq!(
            *app.world().resource::<State<GameState>>().get(),
            GameState::Playing
        );
        assert!(app.world().get_entity(sleeper).is_err());
        assert!(app.world().get_entity(apartment).is_ok());
    }

    #[test]
    fn title_reentry_restores_one_sleeper_and_world_framing_without_touching_auxiliary_camera() {
        use crate::game::camera::MainCamera;
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::asset::AssetPlugin::default()))
            .init_asset::<bevy::world_serialization::WorldAsset>()
            .init_resource::<TitleRender>()
            .add_systems(Update, spawn_sleeper);
        let main = app
            .world_mut()
            .spawn((MainCamera, Transform::from_xyz(9.0, 9.0, 9.0), Msaa::Off))
            .id();
        let auxiliary = app
            .world_mut()
            .spawn((Camera3d::default(), Transform::from_xyz(7.0, 8.0, 9.0)))
            .id();
        let expected =
            Transform::from_xyz(0.90, 1.35, -2.60).looking_at(Vec3::new(3.0, 0.70, 1.0), Vec3::Y);
        for _ in 0..2 {
            app.update();
            app.update();
            assert_eq!(app.world().get::<Transform>(main).unwrap(), &expected);
            assert_eq!(*app.world().get::<Msaa>(main).unwrap(), Msaa::Off);
            assert_eq!(
                app.world().get::<Transform>(auxiliary).unwrap().translation,
                Vec3::new(7.0, 8.0, 9.0)
            );
            let sleepers: Vec<_> = app
                .world_mut()
                .query_filtered::<Entity, With<Sleeper>>()
                .iter(app.world())
                .collect();
            assert_eq!(sleepers.len(), 1);
            app.world_mut().despawn(sleepers[0]);
            app.world_mut()
                .get_mut::<Transform>(main)
                .unwrap()
                .translation = Vec3::ONE;
        }
    }

    #[test]
    fn protected_start_preserves_snapshot_and_loaded_start_preserves_progress() {
        use crate::game::{
            campaign::Campaign,
            campaign_save::{CampaignStore, SnapshotState},
        };
        let mut app = app(GameState::Title);
        app.insert_resource(CampaignStore::for_test(None, SnapshotState::Protected));
        select(&mut app, Action::Start);
        assert_eq!(
            *app.world().resource::<State<GameState>>().get(),
            GameState::Title
        );
        assert_eq!(
            app.world().resource::<CampaignStore>().snapshot_state(),
            SnapshotState::Protected
        );
        assert!(app.world().resource::<CampaignStore>().campaign().is_none());
        assert!(!app.world().resource::<CampaignStore>().notice.is_empty());
        let mut campaign = Campaign::new(5);
        campaign.apply(crate::game::campaign::Action::Wait).unwrap();
        app.insert_resource(CampaignStore::for_test(
            Some(campaign.clone()),
            SnapshotState::Available,
        ));
        let main = app
            .world_mut()
            .spawn((
                crate::game::camera::MainCamera,
                Projection::Perspective(PerspectiveProjection {
                    fov: 0.5,
                    ..default()
                }),
            ))
            .id();
        let extra = app
            .world_mut()
            .spawn((
                Camera3d::default(),
                Projection::Perspective(PerspectiveProjection {
                    fov: 0.4,
                    ..default()
                }),
            ))
            .id();
        select(&mut app, Action::Start);
        assert_eq!(
            *app.world().resource::<State<GameState>>().get(),
            GameState::Playing
        );
        assert_eq!(
            app.world().resource::<CampaignStore>().campaign(),
            Some(&campaign)
        );
        let Projection::Perspective(main) = app.world().get::<Projection>(main).unwrap() else {
            panic!()
        };
        let Projection::Perspective(extra) = app.world().get::<Projection>(extra).unwrap() else {
            panic!()
        };
        assert_eq!(main.fov, 75.0_f32.to_radians());
        assert_eq!(extra.fov, 0.4);
    }

    #[test]
    fn new_game_requires_confirmation_for_protected_or_live_progress_and_cancel_preserves_it() {
        use crate::game::{
            campaign::Campaign,
            campaign_save::{CampaignStore, SnapshotState},
        };
        for live in [None, Some(Campaign::new(7))] {
            let mut app = app(GameState::Title);
            app.insert_resource(CampaignStore::for_test(
                live.clone(),
                SnapshotState::Protected,
            ));
            select(&mut app, Action::ConfirmNewGame);
            assert!(!app.world().resource::<NewGameConfirmation>().0);
            select(&mut app, Action::NewGame);
            assert!(app.world().resource::<NewGameConfirmation>().0);
            assert_eq!(
                *app.world().resource::<State<GameState>>().get(),
                GameState::Title
            );
            key(&mut app, KeyCode::Escape);
            assert!(!app.world().resource::<NewGameConfirmation>().0);
            assert_eq!(
                app.world().resource::<CampaignStore>().campaign(),
                live.as_ref()
            );
            assert_eq!(
                app.world().resource::<CampaignStore>().snapshot_state(),
                SnapshotState::Protected
            );
        }
    }

    #[test]
    fn continue_retains_unsaved_progress_and_restores_report_and_ending_stage() {
        use crate::game::{
            campaign::{Action as CampaignAction, Campaign, CampaignStage},
            campaign_save::{CampaignStore, SnapshotState},
        };
        for report in [false, true] {
            let mut campaign = Campaign::new(7);
            if report {
                campaign.apply(CampaignAction::Sleep).unwrap();
            } else {
                campaign.support.rent_negotiated = true;
                campaign.bills[1].due_day = 10;
                for _ in 1..=10 {
                    campaign.apply(CampaignAction::Sleep).unwrap();
                    campaign.apply(CampaignAction::AcknowledgeReport).unwrap();
                }
            }
            assert_ne!(campaign.stage, CampaignStage::Playing);
            let mut app = app(GameState::Title);
            let mut store =
                CampaignStore::for_test(Some(campaign.clone()), SnapshotState::Protected);
            store.notice = "Progress kept only for this session".into();
            app.insert_resource(store);
            select(&mut app, Action::Start);
            assert_eq!(
                *app.world().resource::<State<GameState>>().get(),
                GameState::Playing
            );
            assert_eq!(
                app.world().resource::<CampaignStore>().campaign(),
                Some(&campaign)
            );
            assert!(!app.world().resource::<CampaignStore>().notice.is_empty());
        }
    }

    #[test]
    fn settings_cancel_restores_saved_values_and_returns_to_its_origin() {
        for origin in [GameState::Title, GameState::Menu] {
            let mut app = app(origin);
            select(&mut app, Action::Settings);
            assert_eq!(
                *app.world().resource::<State<GameState>>().get(),
                GameState::Settings
            );
            select(&mut app, Action::TextureWarping(false));
            assert!(
                !app.world()
                    .resource::<GraphicsOptions>()
                    .current
                    .texture_warping
            );
            assert!(
                app.world()
                    .resource::<GraphicsOptions>()
                    .current
                    .vertex_jitter
            );
            select(&mut app, Action::VertexJitter(false));
            assert!(
                !app.world()
                    .resource::<GraphicsOptions>()
                    .current
                    .vertex_jitter
            );
            select(&mut app, Action::Reset);
            assert_eq!(
                app.world().resource::<GraphicsOptions>().current,
                GraphicsSettings::default()
            );
            select(&mut app, Action::TextureWarping(false));
            select(&mut app, Action::VertexJitter(false));
            app.world_mut()
                .resource_mut::<GraphicsOptions>()
                .current
                .lighting = Quality::Low;
            key(&mut app, KeyCode::Escape);
            assert_eq!(*app.world().resource::<State<GameState>>().get(), origin);
            assert_eq!(
                app.world().resource::<GraphicsOptions>().current,
                GraphicsSettings::default()
            );
        }
    }

    #[test]
    fn chest_breathes_smoothly_within_one_morph_over_four_seconds() {
        assert_eq!(breath_weight(0.0), 0.0);
        assert_eq!(breath_weight(2.0), 1.0);
        assert!(breath_weight(4.0).abs() < 0.00001);
        for step in 0..400 {
            assert!((0.0..=1.0).contains(&breath_weight(step as f32 / 100.0)));
        }
        let mut app = App::new();
        app.init_resource::<Time<Real>>()
            .add_systems(Update, breathe);
        let root = app.world_mut().spawn(Sleeper).id();
        let chest = app
            .world_mut()
            .spawn((
                ChildOf(root),
                bevy::mesh::morph::MorphWeights::new(vec![0.0], None).unwrap(),
            ))
            .id();
        app.world_mut()
            .resource_mut::<Time<Real>>()
            .advance_by(std::time::Duration::from_secs(2));
        app.update();
        assert_eq!(
            app.world()
                .get::<bevy::mesh::morph::MorphWeights>(chest)
                .unwrap()
                .weights(),
            &[1.0]
        );
    }
}
