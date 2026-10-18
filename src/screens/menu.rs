use bevy::{
    input::mouse::{MouseScrollUnit, MouseWheel},
    input_focus::{
        FocusCause, InputFocus,
        tab_navigation::{TabGroup, TabIndex, TabNavigationPlugin},
    },
    prelude::*,
};

use crate::{
    graphics::{ApplyGraphics, GraphicsOptions, GraphicsSettings, Quality, TextureWork},
    state::GameState,
};

pub struct MenuPlugin;
impl Plugin for MenuPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(TabNavigationPlugin)
            .add_systems(Startup, load_skin)
            .add_systems(OnEnter(GameState::Menu), setup)
            .add_systems(OnEnter(GameState::Settings), setup)
            .add_systems(
                Update,
                (
                    actions.before(ApplyGraphics),
                    (appearance, status, scroll).after(ApplyGraphics),
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
    Resume,
    Settings,
    Save,
    Cancel,
    Reset,
    Preset(Quality),
    Lighting(Quality),
    Textures(Quality),
    Scale(Quality),
    Msaa(bool),
}
impl Action {
    fn selected(self, settings: GraphicsSettings) -> bool {
        match self {
            Self::Preset(q) => settings == GraphicsSettings::preset(q),
            Self::Lighting(q) => settings.lighting == q,
            Self::Textures(q) => settings.textures == q,
            Self::Scale(q) => settings.render_scale == q,
            Self::Msaa(on) => settings.msaa == on,
            _ => false,
        }
    }
    fn setting_name(self) -> Option<&'static str> {
        match self {
            Self::Preset(_) => Some("Quality preset"),
            Self::Lighting(_) => Some("Lighting quality"),
            Self::Textures(_) => Some("Texture resolution"),
            Self::Scale(_) => Some("Render resolution"),
            Self::Msaa(_) => Some("Anti-aliasing"),
            _ => None,
        }
    }
    fn waits_for_textures(self) -> bool {
        matches!(self, Self::Resume | Self::Save)
    }
}
#[derive(Component)]
struct ScrollPanel;
#[derive(Component)]
struct Status;
#[derive(Component)]
struct Fps;

#[derive(Resource)]
struct MenuSkin {
    panel: Handle<Image>,
    font: Handle<Font>,
}
fn load_skin(mut commands: Commands, assets: Res<AssetServer>) {
    commands.insert_resource(MenuSkin {
        panel: assets.load("ui/pause-panel.png"),
        font: assets.load("fonts/LiberationSans-Bold.ttf"),
    });
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
                border: BorderRect::all(44.0),
                center_scale_mode: SliceScaleMode::Stretch,
                sides_scale_mode: SliceScaleMode::Stretch,
                max_corner_scale: corners,
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
    let main = matches!(action, Action::Resume | Action::Settings);
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
    detail: &str,
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
                label(copy, skin, detail, 18.0, MUTED);
            });
            row.spawn(Node {
                column_gap: px(8),
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
                                            "Low disables shadows. All lights stay on.",
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
                                            "Textures",
                                            "1/4 size / 1/2 size / original",
                                            &[
                                                ("Low", Action::Textures(Quality::Low)),
                                                ("Medium", Action::Textures(Quality::Medium)),
                                                ("High", Action::Textures(Quality::High)),
                                            ],
                                            &mut index,
                                        );
                                        choices(
                                            body,
                                            &skin,
                                            "Render resolution",
                                            "Lower values improve speed.",
                                            &[
                                                ("50%", Action::Scale(Quality::Low)),
                                                ("75%", Action::Scale(Quality::Medium)),
                                                ("100%", Action::Scale(Quality::High)),
                                            ],
                                            &mut index,
                                        );
                                        choices(
                                            body,
                                            &skin,
                                            "Anti-aliasing",
                                            "Smooths the edges of objects.",
                                            &[
                                                ("Off", Action::Msaa(false)),
                                                ("4x MSAA", Action::Msaa(true)),
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
                                                label(hint, &skin, "ESC to resume", 18.0, MUTED);
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
    mut focus: ResMut<InputFocus>,
    keys: Res<ButtonInput<KeyCode>>,
    state: Res<State<GameState>>,
    mut options: ResMut<GraphicsOptions>,
    work: Res<TextureWork>,
    mut next: ResMut<NextState<GameState>>,
    buttons: Query<(Entity, &Action, &Interaction), With<Button>>,
    changed: Query<(Entity, &Action, &Interaction), (With<Button>, Changed<Interaction>)>,
) {
    let action = if keys.just_pressed(KeyCode::Escape) {
        Some(if *state.get() == GameState::Settings {
            Action::Cancel
        } else {
            Action::Resume
        })
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
    if action.waits_for_textures() && work.busy() {
        return;
    }
    match action {
        Action::Resume => next.set(GameState::Playing),
        Action::Settings => {
            options.cancel();
            next.set(GameState::Settings);
        }
        Action::Save => {
            options.save();
            next.set(GameState::Menu);
        }
        Action::Cancel => {
            options.cancel();
            next.set(GameState::Menu);
        }
        Action::Reset => options.current = GraphicsSettings::default(),
        Action::Preset(q) => options.current = GraphicsSettings::preset(q),
        Action::Lighting(q) => options.current.lighting = q,
        Action::Textures(q) => options.current.textures = q,
        Action::Scale(q) => options.current.render_scale = q,
        Action::Msaa(on) => options.current.msaa = on,
    }
}
fn appearance(
    mut commands: Commands,
    options: Res<GraphicsOptions>,
    work: Res<TextureWork>,
    focus: Res<InputFocus>,
    mut buttons: Query<(
        Entity,
        &Action,
        &Interaction,
        &mut ImageNode,
        &mut BorderColor,
        Has<bevy::ui::InteractionDisabled>,
        Option<&mut bevy::a11y::AccessibilityNode>,
        Has<bevy::ui::Checkable>,
        Has<bevy::ui::Checked>,
    )>,
) {
    for (
        entity,
        action,
        interaction,
        mut image,
        mut border,
        was_disabled,
        accessible,
        checkable,
        checked,
    ) in &mut buttons
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
        let disabled = action.waits_for_textures() && work.busy();
        if disabled != was_disabled {
            if disabled {
                commands
                    .entity(entity)
                    .insert(bevy::ui::InteractionDisabled);
            } else {
                commands
                    .entity(entity)
                    .remove::<bevy::ui::InteractionDisabled>();
            }
        }
        image.color = if disabled {
            Color::linear_rgb(0.5, 0.5, 0.5)
        } else if action.selected(options.current) {
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
    work: Res<TextureWork>,
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
            **text = if work.busy() {
                format!(
                    "Updating textures... {} remaining. Cancel is available.",
                    work.remaining()
                )
            } else if let Some(problem) = &work.problem {
                problem.clone()
            } else if !options.notice.is_empty() {
                options.notice.clone()
            } else if *state.get() == GameState::Settings {
                "Live preview. Esc cancels.".into()
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
