//! Three compact football pictures on the apartment's existing CRT.
use bevy::{
    asset::LoadState,
    gltf::GltfMaterialName,
    image::{ImageLoaderSettings, ImageSampler},
    mesh::{UvChannel, VertexAttributeValues},
    prelude::*,
    window::PrimaryWindow,
};
use std::time::Duration;

use super::{
    campaign::{Campaign, HouseholdItem},
    campaign_save::{CampaignReplaced, CampaignStore},
};
use crate::state::GameState;

pub(crate) struct TvPlugin;

// shortcut: the TV antenna reuses glass pixels; refit this unused patch if the atlas changes.
const ANTENNA_SOURCE: UVec2 = UVec2::new(108, 124);
const ANTENNA_TARGET: UVec2 = UVec2::new(128, 0);
const ANTENNA_PATCH: u32 = 8;

#[derive(Resource)]
pub(crate) struct TvAssets {
    frames: [Handle<Image>; 3],
    bound: bool,
    error: Option<String>,
}

#[derive(Component)]
struct TvMaterials {
    off: Handle<StandardMaterial>,
    on: Handle<StandardMaterial>,
}

#[derive(Component)]
struct TvScreen {
    timer: Timer,
    frame: usize,
}

impl Default for TvScreen {
    fn default() -> Self {
        Self {
            timer: Timer::from_seconds(0.5, TimerMode::Repeating),
            frame: 0,
        }
    }
}

impl TvScreen {
    fn advance(&mut self, delta: Duration, state: GameState, focused: bool, visible: bool) {
        if visible && focused && matches!(state, GameState::Intro | GameState::Playing) {
            self.timer.tick(delta);
            self.frame = (self.frame + self.timer.times_finished_this_tick() as usize) % 3;
        }
    }
}

impl Plugin for TvPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup)
            .add_observer(reset)
            .add_systems(Update, (bind, update).chain());
    }
}

pub(crate) fn readiness(assets: &AssetServer, tv: &TvAssets) -> Result<bool, String> {
    if let Some(error) = &tv.error {
        return Err(error.clone());
    }
    for (i, frame) in tv.frames.iter().enumerate() {
        if let LoadState::Failed(error) = assets.load_state(frame.id()) {
            return Err(format!("Could not load TV picture {}: {error}", i + 1));
        }
    }
    Ok(tv.bound
        && tv
            .frames
            .iter()
            .all(|frame| assets.is_loaded_with_dependencies(frame.id())))
}

fn setup(mut commands: Commands, assets: Res<AssetServer>) {
    let frames = std::array::from_fn(|i| {
        assets
            .load_builder()
            .with_settings(|settings: &mut ImageLoaderSettings| {
                settings.sampler = ImageSampler::nearest();
            })
            .load(format!("models/tv-football-{}.png", i + 1))
    });
    commands.insert_resource(TvAssets {
        frames,
        bound: false,
        error: None,
    });
}

fn glass_base(original: &Image, mask: &Image) -> Result<Image, String> {
    if original.size() != UVec2::splat(256) || mask.size() != UVec2::splat(512) {
        return Err("TV texture dimensions changed; refit the broadcast glass mask.".into());
    }
    let mut image = original.clone();
    image.sampler = ImageSampler::nearest();
    for y in 0..256 {
        for x in 0..256 {
            if mask
                .get_color_at(x * 2, y * 2)
                .map_err(|e| e.to_string())?
                .alpha()
                > 0.0
            {
                image
                    .set_color_at(x, y, Color::BLACK)
                    .map_err(|e| e.to_string())?;
            }
        }
    }
    for y in 0..ANTENNA_PATCH {
        for x in 0..ANTENNA_PATCH {
            let color = original
                .get_color_at(ANTENNA_SOURCE.x + x, ANTENNA_SOURCE.y + y)
                .map_err(|e| e.to_string())?;
            image
                .set_color_at(ANTENNA_TARGET.x + x, ANTENNA_TARGET.y + y, color)
                .map_err(|e| e.to_string())?;
        }
    }
    Ok(image)
}

fn broadcast_mesh(original: &Mesh) -> Result<Mesh, String> {
    let (
        Some(VertexAttributeValues::Float32x2(uvs)),
        Some(VertexAttributeValues::Float32x3(normals)),
    ) = (
        original.attribute(Mesh::ATTRIBUTE_UV_0),
        original.attribute(Mesh::ATTRIBUTE_NORMAL),
    )
    else {
        return Err("The TV mesh needs its original UVs and normals.".into());
    };
    let mut uvs = uvs.clone();
    let mut relocated = 0;
    for (uv, normal) in uvs.iter_mut().zip(normals) {
        let pixel = Vec2::from(*uv) * 256.0;
        if normal[0] < 0.999
            && pixel.cmpge(ANTENNA_SOURCE.as_vec2()).all()
            && pixel
                .cmplt((ANTENNA_SOURCE + UVec2::splat(ANTENNA_PATCH)).as_vec2())
                .all()
        {
            *uv =
                ((pixel - ANTENNA_SOURCE.as_vec2() + ANTENNA_TARGET.as_vec2()) / 256.0).to_array();
            relocated += 1;
        }
    }
    if relocated != 18 {
        return Err("The TV antenna UVs changed; refit the broadcast material.".into());
    }
    let mut mesh = original.clone();
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_1, uvs);
    Ok(mesh)
}

fn bind(
    mut commands: Commands,
    mut tv: ResMut<TvAssets>,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    tv_meshes: Query<
        (
            Entity,
            &Mesh3d,
            &GltfMaterialName,
            &MeshMaterial3d<StandardMaterial>,
        ),
        (With<Mesh3d>, Without<TvScreen>),
    >,
    hierarchy: Query<(Option<&Name>, Option<&ChildOf>)>,
) {
    if tv.bound || tv.error.is_some() {
        return;
    }
    for (entity, mesh, name, handle) in &tv_meshes {
        if name.0 != "TV" {
            continue;
        }
        let mut ancestor = Some(entity);
        let mut found = false;
        while let Some(entity) = ancestor {
            let Ok((name, parent)) = hierarchy.get(entity) else {
                break;
            };
            if name.is_some_and(|name| name.as_str() == "Apartment_TV.002") {
                found = true;
                break;
            }
            ancestor = parent.map(ChildOf::parent);
        }
        if !found {
            continue;
        }
        let Some(mesh) = meshes.get(&mesh.0) else {
            return;
        };
        let mesh = match broadcast_mesh(mesh) {
            Ok(mesh) => mesh,
            Err(error) => {
                tv.error = Some(error);
                return;
            }
        };
        let Some(original) = materials.get(&handle.0) else {
            return;
        };
        let Some(base) = original.base_color_texture.as_ref() else {
            tv.error = Some("The TV material has no base texture.".into());
            return;
        };
        let (Some(base), Some(mask)) = (images.get(base), images.get(&tv.frames[0])) else {
            return;
        };
        let base = match glass_base(base, mask) {
            Ok(base) => base,
            Err(error) => {
                tv.error = Some(error);
                return;
            }
        };
        let mut on = original.clone();
        on.base_color_texture = Some(images.add(base));
        on.base_color_channel = UvChannel::Uv1;
        on.emissive = LinearRgba::WHITE;
        on.emissive_channel = UvChannel::Uv1;
        on.emissive_texture = Some(tv.frames[0].clone());
        commands.entity(entity).insert((
            Mesh3d(meshes.add(mesh)),
            TvScreen::default(),
            TvMaterials {
                off: handle.0.clone(),
                on: materials.add(on),
            },
        ));
        tv.bound = true;
        return;
    }
}

fn reset(_: On<CampaignReplaced>, mut screens: Query<&mut TvScreen>) {
    for mut screen in &mut screens {
        *screen = TvScreen::default();
    }
}

fn powered(campaign: Option<&Campaign>) -> bool {
    campaign.is_some_and(|c| c.has_power() && c.household.owns(HouseholdItem::Tv))
}

fn update(
    state: Res<State<GameState>>,
    time: Res<Time<Real>>,
    store: Res<CampaignStore>,
    assets: Res<AssetServer>,
    tv: Res<TvAssets>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut screens: Query<(
        &mut TvScreen,
        &TvMaterials,
        &mut MeshMaterial3d<StandardMaterial>,
    )>,
) {
    let focused = windows.single().is_ok_and(|window| window.focused);
    for (mut screen, handles, mut material) in &mut screens {
        if matches!(state.get(), GameState::Menu | GameState::Settings) {
            continue;
        }
        let visible = *state.get() != GameState::Title
            && powered(store.campaign())
            && readiness(&assets, &tv).unwrap_or(false);
        let wanted = if visible { &handles.on } else { &handles.off };
        if material.0 != *wanted {
            material.0 = wanted.clone();
        }
        screen.advance(
            time.delta().min(Duration::from_millis(100)),
            *state.get(),
            focused,
            visible,
        );
        if materials.get(&handles.on).is_some_and(|material| {
            material.emissive_texture.as_ref() != Some(&tv.frames[screen.frame])
        }) {
            materials.get_mut(&handles.on).unwrap().emissive_texture =
                Some(tv.frames[screen.frame].clone());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::campaign::Action;
    use super::*;

    fn actual_tv_mesh() -> Mesh {
        use bevy::{
            asset::RenderAssetUsages, mesh::Indices, render::render_resource::PrimitiveTopology,
        };
        let bytes = std::fs::read("assets/models/apartment_windows.glb").unwrap();
        let gltf = bevy::gltf::gltf::Gltf::from_slice(&bytes).unwrap();
        let node = gltf
            .nodes()
            .find(|node| node.name() == Some("Apartment_TV.002"))
            .unwrap();
        let mesh = node.mesh().unwrap();
        let primitive = mesh.primitives().next().unwrap();
        let reader = primitive.reader(|_| gltf.blob.as_deref());
        Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::default(),
        )
        .with_inserted_attribute(
            Mesh::ATTRIBUTE_POSITION,
            reader.read_positions().unwrap().collect::<Vec<_>>(),
        )
        .with_inserted_attribute(
            Mesh::ATTRIBUTE_NORMAL,
            reader.read_normals().unwrap().collect::<Vec<_>>(),
        )
        .with_inserted_attribute(
            Mesh::ATTRIBUTE_UV_0,
            reader
                .read_tex_coords(0)
                .unwrap()
                .into_f32()
                .collect::<Vec<_>>(),
        )
        .with_inserted_indices(Indices::U32(
            reader.read_indices().unwrap().into_u32().collect(),
        ))
    }

    #[test]
    fn real_antenna_uvs_keep_original_pixels_without_sampling_football() {
        use bevy::{
            asset::RenderAssetUsages,
            image::{CompressedImageFormats, ImageType},
        };
        let read_image = |path| {
            Image::from_buffer(
                &std::fs::read(path).unwrap(),
                ImageType::Extension("png"),
                CompressedImageFormats::NONE,
                true,
                ImageSampler::nearest(),
                RenderAssetUsages::default(),
            )
            .unwrap()
        };
        let original = actual_tv_mesh();
        let fixed = broadcast_mesh(&original).unwrap();
        let texture = read_image("base/Trailer_Park/Textures/TV.png");
        let emission = read_image("assets/models/tv-football-1.png");
        let base = glass_base(&texture, &emission).unwrap();
        let Some(VertexAttributeValues::Float32x2(before)) =
            original.attribute(Mesh::ATTRIBUTE_UV_0)
        else {
            panic!()
        };
        let Some(VertexAttributeValues::Float32x2(after)) = fixed.attribute(Mesh::ATTRIBUTE_UV_1)
        else {
            panic!()
        };
        let Some(VertexAttributeValues::Float32x3(positions)) =
            original.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            panic!()
        };
        let mut changed = 0;
        for ((old, new), position) in before.iter().zip(after).zip(positions) {
            if old == new {
                continue;
            }
            changed += 1;
            assert!(
                position[1] > 1.14,
                "Only antenna vertices may move in UV space"
            );
            let source = (Vec2::from(*old) * 256.0).as_uvec2();
            let target = (Vec2::from(*new) * 256.0).as_uvec2();
            assert!(
                emission
                    .get_color_at(source.x * 2, source.y * 2)
                    .unwrap()
                    .alpha()
                    > 0.0
            );
            assert_eq!(
                emission
                    .get_color_at(target.x * 2, target.y * 2)
                    .unwrap()
                    .to_srgba(),
                Srgba::NONE
            );
            assert_eq!(
                base.get_color_at(target.x, target.y).unwrap(),
                texture.get_color_at(source.x, source.y).unwrap()
            );
        }
        assert_eq!(changed, 18);
        for attr in [
            Mesh::ATTRIBUTE_POSITION,
            Mesh::ATTRIBUTE_NORMAL,
            Mesh::ATTRIBUTE_UV_0,
        ] {
            assert_eq!(original.attribute(attr), fixed.attribute(attr));
        }
        assert_eq!(original.indices(), fixed.indices());
        let indices: Vec<_> = original.indices().unwrap().iter().collect();
        for triangle in indices.chunks_exact(3) {
            let pixels: Vec<_> = triangle
                .iter()
                .map(|&i| Vec2::from(before[i]) * 256.0)
                .collect();
            let min = pixels.iter().copied().reduce(Vec2::min).unwrap();
            let max = pixels.iter().copied().reduce(Vec2::max).unwrap();
            assert!(
                !(max.x >= 128.0 && min.x < 136.0 && max.y >= 0.0 && min.y < 8.0),
                "Original triangles must not use the new antenna patch"
            );
        }
    }

    #[test]
    fn half_second_frames_wrap_and_freeze_outside_focused_play() {
        let mut screen = TvScreen::default();
        for (milliseconds, expected) in [(499, 0), (1, 1), (500, 2), (500, 0), (1000, 2)] {
            screen.advance(
                Duration::from_millis(milliseconds),
                GameState::Intro,
                true,
                true,
            );
            assert_eq!(screen.frame, expected);
        }
        let elapsed = screen.timer.elapsed();
        for state in [GameState::Title, GameState::Menu, GameState::Settings] {
            screen.advance(Duration::from_secs(1), state, true, true);
            assert_eq!(screen.frame, 2);
            assert_eq!(screen.timer.elapsed(), elapsed);
        }
        for (focused, visible) in [(false, true), (true, false)] {
            screen.advance(Duration::from_secs(1), GameState::Playing, focused, visible);
            assert_eq!(screen.frame, 2);
            assert_eq!(screen.timer.elapsed(), elapsed);
        }
        screen.advance(Duration::from_millis(500), GameState::Playing, true, true);
        assert_eq!(screen.frame, 0);
    }

    #[test]
    fn power_and_sale_turn_the_display_off() {
        let mut campaign = Campaign::new(17);
        assert!(!powered(None));
        assert!(powered(Some(&campaign)));
        campaign.clock.day = 6;
        assert!(!powered(Some(&campaign)));
        campaign.clock.day = 1;
        assert!(powered(Some(&campaign)));
        campaign.attributes.stress = 80;
        campaign.apply(Action::Sell(HouseholdItem::Tv)).unwrap();
        assert!(!powered(Some(&campaign)));
    }

    #[test]
    fn new_campaign_resets_to_first_frame() {
        let mut app = App::new();
        app.add_observer(reset);
        let mut screen = TvScreen::default();
        screen.advance(Duration::from_millis(1100), GameState::Playing, true, true);
        let entity = app.world_mut().spawn(screen).id();
        app.world_mut().trigger(CampaignReplaced);
        let screen = app.world().get::<TvScreen>(entity).unwrap();
        assert_eq!(screen.frame, 0);
        assert_eq!(screen.timer.elapsed(), Duration::ZERO);
    }

    #[test]
    fn broadcast_binds_existing_mesh_and_preserves_lit_case_and_collider() {
        use bevy::{
            asset::RenderAssetUsages,
            render::render_resource::{Extent3d, TextureDimension, TextureFormat},
        };
        let make_image = |size, color| {
            Image::new_fill(
                Extent3d {
                    width: size,
                    height: size,
                    depth_or_array_layers: 1,
                },
                TextureDimension::D2,
                color,
                TextureFormat::Rgba8UnormSrgb,
                RenderAssetUsages::default(),
            )
        };
        let original = make_image(256, &[96, 64, 32, 255]);
        let original_bytes = original.data.clone();
        let mut mask = make_image(512, &[0, 0, 0, 0]);
        mask.set_color_at(140, 176, Color::WHITE).unwrap();
        let cleared = glass_base(&original, &mask).unwrap();
        assert_eq!(
            cleared.get_color_at(70, 88).unwrap(),
            Color::srgb(0.0, 0.0, 0.0)
        );
        assert_eq!(
            cleared.get_color_at(69, 88).unwrap(),
            original.get_color_at(69, 88).unwrap()
        );
        assert!(glass_base(&original, &make_image(16, &[0, 0, 0, 0])).is_err());
        let mut app = App::new();
        app.init_resource::<Assets<Image>>()
            .init_resource::<Assets<Mesh>>()
            .init_resource::<Assets<StandardMaterial>>()
            .add_systems(Update, bind);
        let base = app
            .world_mut()
            .resource_mut::<Assets<Image>>()
            .add(original);
        let frame = app.world_mut().resource_mut::<Assets<Image>>().add(mask);
        app.insert_resource(TvAssets {
            frames: [frame.clone(), frame.clone(), frame.clone()],
            bound: false,
            error: None,
        });
        let material = app
            .world_mut()
            .resource_mut::<Assets<StandardMaterial>>()
            .add(StandardMaterial {
                base_color_texture: Some(base.clone()),
                perceptual_roughness: 0.73,
                cull_mode: None,
                ..default()
            });
        let node = app.world_mut().spawn(Name::new("Apartment_TV.002")).id();
        let mesh = app
            .world_mut()
            .resource_mut::<Assets<Mesh>>()
            .add(actual_tv_mesh());
        let entity = app
            .world_mut()
            .spawn((
                ChildOf(node),
                Mesh3d(mesh),
                GltfMaterialName("TV".into()),
                MeshMaterial3d(material.clone()),
                bevy_rapier3d::prelude::Collider::cuboid(0.1, 0.2, 0.3),
            ))
            .id();
        app.update();
        assert!(app.world().resource::<TvAssets>().bound);
        assert_eq!(
            app.world_mut()
                .query_filtered::<Entity, With<Mesh3d>>()
                .iter(app.world())
                .count(),
            1,
            "Binding must not spawn an overlay mesh"
        );
        let screen = app.world().entity(entity);
        assert!(screen.contains::<bevy_rapier3d::prelude::Collider>());
        assert_eq!(screen.get::<ChildOf>().unwrap().parent(), node);
        let handles = screen.get::<TvMaterials>().unwrap();
        assert_eq!(handles.off, material);
        let materials = app.world().resource::<Assets<StandardMaterial>>();
        let on = materials.get(&handles.on).unwrap();
        assert!(!on.unlit);
        assert_eq!(on.perceptual_roughness, 0.73);
        assert_eq!(on.cull_mode, None);
        assert_eq!(on.emissive_texture, Some(frame));
        assert_eq!(on.base_color_channel, UvChannel::Uv1);
        assert_eq!(on.emissive_channel, UvChannel::Uv1);
        assert_eq!(
            materials.get(&material).unwrap().base_color_texture,
            Some(base.clone())
        );
        assert_eq!(
            app.world()
                .resource::<Assets<Image>>()
                .get(&base)
                .unwrap()
                .data,
            original_bytes
        );
    }
}
