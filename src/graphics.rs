//! User-selected graphics quality; High preserves the original rendering path.
use std::collections::{HashMap, VecDeque};

use bevy::{
    asset::VisitAssetDependencies,
    camera::RenderTarget,
    image::ImageSampler,
    light::{DirectionalLightShadowMap, ShadowFilteringMethod},
    prelude::*,
    render::render_resource::TextureFormat,
    window::PrimaryWindow,
};
use serde::{Deserialize, Serialize};

const STORAGE_KEY: &str = "casino-game.graphics.v1";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Quality {
    Low,
    Medium,
    #[default]
    High,
}
impl Quality {
    pub const ALL: [Self; 3] = [Self::Low, Self::Medium, Self::High];
    pub fn label(self) -> &'static str {
        match self {
            Self::Low => "Low",
            Self::Medium => "Medium",
            Self::High => "High",
        }
    }
    pub fn scale(self) -> u32 {
        match self {
            Self::Low => 50,
            Self::Medium => 75,
            Self::High => 100,
        }
    }
    fn divisor(self) -> u32 {
        match self {
            Self::Low => 4,
            Self::Medium => 2,
            Self::High => 1,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GraphicsSettings {
    pub lighting: Quality,
    pub textures: Quality,
    pub render_scale: Quality,
    pub msaa: bool,
}
impl Default for GraphicsSettings {
    fn default() -> Self {
        Self::preset(Quality::High)
    }
}
impl GraphicsSettings {
    pub fn preset(quality: Quality) -> Self {
        Self {
            lighting: quality,
            textures: quality,
            render_scale: quality,
            msaa: quality == Quality::High,
        }
    }
    pub fn label(self) -> &'static str {
        Quality::ALL
            .into_iter()
            .find(|q| Self::preset(*q) == self)
            .map_or("Custom", Quality::label)
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SavedSettings {
    version: u8,
    graphics: GraphicsSettings,
}
fn decode_settings(json: &str) -> Result<GraphicsSettings, String> {
    let saved: SavedSettings = serde_json::from_str(json).map_err(|e| e.to_string())?;
    if saved.version != 1 {
        return Err("Unsupported settings version".into());
    }
    Ok(saved.graphics)
}

#[derive(Resource)]
pub struct GraphicsOptions {
    pub current: GraphicsSettings,
    pub saved: GraphicsSettings,
    pub notice: String,
}
impl Default for GraphicsOptions {
    fn default() -> Self {
        let (settings, notice) = match read_settings() {
            Ok(Some(raw)) => match decode_settings(&raw) {
                Ok(settings) => (settings, String::new()),
                Err(_) => (
                    GraphicsSettings::default(),
                    "Saved graphics settings were invalid. High quality restored.".into(),
                ),
            },
            Ok(None) => (GraphicsSettings::default(), String::new()),
            Err(_) => (
                GraphicsSettings::default(),
                "Local storage unavailable. Settings can still be used this session.".into(),
            ),
        };
        Self {
            current: settings,
            saved: settings,
            notice,
        }
    }
}
impl GraphicsOptions {
    pub fn cancel(&mut self) {
        self.current = self.saved;
    }
    pub fn save(&mut self) {
        self.saved = self.current;
        let result = serde_json::to_string(&SavedSettings {
            version: 1,
            graphics: self.saved,
        })
        .map_err(|e| e.to_string())
        .and_then(|json| write_settings(&json));
        self.notice = match result {
            Ok(()) => "Graphics settings saved on this device.".into(),
            Err(error) => {
                warn!("Cannot save graphics settings: {error}");
                "Settings kept for this session only; local storage is unavailable.".into()
            }
        };
    }
}

#[cfg(target_arch = "wasm32")]
fn storage() -> Result<web_sys::Storage, String> {
    web_sys::window()
        .ok_or("No browser window")?
        .local_storage()
        .map_err(|e| format!("{e:?}"))?
        .ok_or("Local storage is disabled".into())
}
#[cfg(target_arch = "wasm32")]
fn read_settings() -> Result<Option<String>, String> {
    storage()?
        .get_item(STORAGE_KEY)
        .map_err(|e| format!("{e:?}"))
}
#[cfg(target_arch = "wasm32")]
fn write_settings(json: &str) -> Result<(), String> {
    storage()?
        .set_item(STORAGE_KEY, json)
        .map_err(|e| format!("{e:?}"))
}
#[cfg(not(target_arch = "wasm32"))]
fn settings_path() -> Result<std::path::PathBuf, String> {
    use std::{env, path::PathBuf};
    let root = if cfg!(target_os = "windows") {
        env::var_os("APPDATA").map(PathBuf::from)
    } else if cfg!(target_os = "macos") {
        env::var_os("HOME").map(|p| PathBuf::from(p).join("Library/Application Support"))
    } else {
        env::var_os("XDG_CONFIG_HOME")
            .filter(|p| !p.is_empty())
            .map(PathBuf::from)
            .or_else(|| env::var_os("HOME").map(|p| PathBuf::from(p).join(".config")))
    }
    .ok_or("No user configuration directory")?;
    Ok(root.join("CasinoGame").join(format!("{STORAGE_KEY}.json")))
}
#[cfg(not(target_arch = "wasm32"))]
fn read_settings() -> Result<Option<String>, String> {
    match std::fs::read_to_string(settings_path()?) {
        Ok(json) => Ok(Some(json)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}
#[cfg(not(target_arch = "wasm32"))]
fn write_settings(json: &str) -> Result<(), String> {
    let path = settings_path()?;
    std::fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
    let temporary = path.with_extension("tmp");
    std::fs::write(&temporary, json).map_err(|e| e.to_string())?;
    std::fs::rename(temporary, path).map_err(|e| e.to_string())
}

pub struct GraphicsPlugin;
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct ApplyGraphics;
impl Plugin for GraphicsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GraphicsOptions>()
            .init_resource::<TextureWork>()
            .add_systems(Startup, setup_scaling)
            .add_systems(
                Update,
                (apply_lighting, apply_scale, update_textures).in_set(ApplyGraphics),
            );
    }
}

#[derive(Resource)]
struct ScaledView {
    camera: Entity,
    surface: Entity,
    image: Handle<Image>,
    last: Option<(Quality, UVec2)>,
}
fn setup_scaling(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    let image = images.add(render_image(UVec2::ONE));
    let camera = commands
        .spawn((
            Camera2d,
            Camera {
                order: 1,
                is_active: false,
                ..default()
            },
            Msaa::Off,
        ))
        .id();
    let surface = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                width: percent(100),
                height: percent(100),
                display: Display::None,
                ..default()
            },
            ImageNode::new(image.clone()),
            UiTargetCamera(camera),
            GlobalZIndex(-100),
            bevy::ui::FocusPolicy::Pass,
        ))
        .id();
    commands.insert_resource(ScaledView {
        camera,
        surface,
        image,
        last: None,
    });
}
fn render_image(size: UVec2) -> Image {
    let mut image = Image::new_target_texture(size.x, size.y, TextureFormat::Rgba8UnormSrgb, None);
    image.sampler = ImageSampler::linear();
    image
}
fn scaled_size(size: UVec2, quality: Quality) -> UVec2 {
    (size * quality.scale() / 100).max(UVec2::ONE)
}
fn apply_scale(
    options: Res<GraphicsOptions>,
    window: Query<&Window, With<PrimaryWindow>>,
    mut view: ResMut<ScaledView>,
    mut images: ResMut<Assets<Image>>,
    mut cameras: ParamSet<(
        Query<&mut RenderTarget, With<Camera3d>>,
        Query<&mut Camera, With<Camera2d>>,
    )>,
    mut nodes: Query<&mut Node>,
) {
    let Ok(window) = window.single() else { return };
    let size = UVec2::new(window.physical_width(), window.physical_height());
    if size.min_element() == 0 {
        return;
    }
    let quality = options.current.render_scale;
    if view.last == Some((quality, size)) {
        return;
    }
    let scaled = quality != Quality::High;
    {
        let mut ui_cameras = cameras.p1();
        let Ok(mut camera) = ui_cameras.get_mut(view.camera) else {
            return;
        };
        camera.is_active = scaled;
        // Bevy selects the default UI camera by order, including inactive cameras.
        camera.order = if scaled { 1 } else { -1 };
    }
    for mut target in &mut cameras.p0() {
        *target = if scaled {
            RenderTarget::Image(view.image.clone().into())
        } else {
            RenderTarget::default()
        };
    }
    if let Some(mut image) = images.get_mut(&view.image) {
        *image = render_image(if scaled {
            scaled_size(size, quality)
        } else {
            UVec2::ONE
        });
    }
    if let Ok(mut node) = nodes.get_mut(view.surface) {
        node.display = if scaled { Display::Flex } else { Display::None };
    }
    info!(
        "Graphics render scale {}%, 3D target {:?}",
        quality.scale(),
        scaled_size(size, quality)
    );
    view.last = Some((quality, size));
}
fn apply_lighting(
    mut commands: Commands,
    options: Res<GraphicsOptions>,
    mut previous: Local<Option<GraphicsSettings>>,
    mut shadows: ResMut<DirectionalLightShadowMap>,
    cameras: Query<Entity, With<Camera3d>>,
    mut spots: Query<&mut SpotLight>,
    mut suns: Query<&mut DirectionalLight>,
) {
    let settings = options.current;
    let changed = previous.is_none_or(|old| old != settings);
    let shadows_on = settings.lighting != Quality::Low;
    if changed {
        shadows.size = if settings.lighting == Quality::Medium {
            1024
        } else {
            2048
        };
        for camera in &cameras {
            commands.entity(camera).insert((
                if settings.msaa {
                    Msaa::Sample4
                } else {
                    Msaa::Off
                },
                if settings.lighting == Quality::High {
                    ShadowFilteringMethod::Gaussian
                } else {
                    ShadowFilteringMethod::Hardware2x2
                },
            ));
        }
        info!("Graphics quality {}: {:?}", settings.label(), settings);
    }
    for mut light in &mut spots {
        if (changed || light.is_added()) && light.shadow_maps_enabled != shadows_on {
            light.shadow_maps_enabled = shadows_on;
        }
    }
    for mut light in &mut suns {
        if (changed || light.is_added()) && light.shadow_maps_enabled != shadows_on {
            light.shadow_maps_enabled = shadows_on;
        }
    }
    *previous = Some(settings);
}

#[derive(Resource, Default)]
pub struct TextureWork {
    originals: HashMap<AssetId<Image>, Image>,
    applied: HashMap<AssetId<Image>, Quality>,
    pending: VecDeque<AssetId<Image>>,
    desired: Option<Quality>,
    pub problem: Option<String>,
}
impl TextureWork {
    pub fn busy(&self) -> bool {
        !self.pending.is_empty()
    }
    pub fn remaining(&self) -> usize {
        self.pending.len()
    }
}
fn update_textures(
    options: Res<GraphicsOptions>,
    assets: Res<AssetServer>,
    mut images: ResMut<Assets<Image>>,
    mut events: MessageReader<AssetEvent<Image>>,
    mut work: ResMut<TextureWork>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let added = events.read().fold(false, |added, event| {
        matches!(event, AssetEvent::Added { .. }) || added
    });
    let quality = options.current.textures;
    if work.desired != Some(quality) || added {
        work.pending = images
            .iter()
            .filter_map(|(id, _)| {
                let path = assets.get_path(id)?;
                (path.path() == std::path::Path::new("models/apartment_windows.glb")
                    && work.applied.get(&id).copied().unwrap_or(Quality::High) != quality)
                    .then_some(id)
            })
            .collect();
        work.desired = Some(quality);
        work.problem = None;
    }
    let mut resized = Vec::new();
    for _ in 0..4 {
        let Some(id) = work.pending.pop_front() else {
            break;
        };
        let Some(mut image) = images.get_mut(id) else {
            continue;
        };
        if quality == Quality::High {
            if let Some(original) = work.originals.remove(&id) {
                *image = original;
            }
        } else {
            let original = work.originals.entry(id).or_insert_with(|| image.clone());
            match reduce_texture(original, quality) {
                Ok(reduced) => *image = reduced,
                Err(error) => {
                    warn!("Texture quality: {error}");
                    work.problem =
                        Some("Some textures could not be resized; originals retained.".into());
                }
            }
        }
        work.applied.insert(id, quality);
        resized.push(id.untyped());
    }
    if !resized.is_empty() {
        let affected: Vec<_> = materials
            .iter()
            .filter_map(|(id, material)| {
                let mut affected = false;
                material.visit_dependencies(&mut |image| affected |= resized.contains(&image));
                affected.then_some(id)
            })
            .collect();
        // Resizing replaces GPU texture views; rebuild the materials' cached bindings.
        for id in affected {
            materials.get_mut(id).unwrap().into_inner();
        }
    }
}
fn reduce_texture(original: &Image, quality: Quality) -> Result<Image, String> {
    let format = original.texture_descriptor.format;
    if !matches!(
        format,
        TextureFormat::Rgba8UnormSrgb | TextureFormat::Rgba8Unorm
    ) || original.texture_descriptor.size.depth_or_array_layers != 1
    {
        return Err(format!("Unsupported texture format {format:?}"));
    }
    let bytes = original.data.as_ref().ok_or("Texture pixels unavailable")?;
    let (width, height) = (original.width(), original.height());
    if bytes.len() != (width as usize * height as usize * 4) {
        return Err("Unexpected texture data length".into());
    }
    let srgb = format == TextureFormat::Rgba8UnormSrgb;
    let linear = image::ImageBuffer::from_fn(width, height, |x, y| {
        let offset = ((y * width + x) * 4) as usize;
        let alpha = bytes[offset + 3] as f32 / 255.0;
        let mut pixel = [0.0; 4];
        for c in 0..3 {
            let value = bytes[offset + c] as f32 / 255.0;
            pixel[c] = (if srgb { srgb_to_linear(value) } else { value }) * alpha;
        }
        pixel[3] = alpha;
        image::Rgba(pixel)
    });
    let size = UVec2::new(
        (width / quality.divisor()).max(1),
        (height / quality.divisor()).max(1),
    );
    let resized = image::imageops::resize(
        &linear,
        size.x,
        size.y,
        image::imageops::FilterType::Triangle,
    );
    let mut data = Vec::with_capacity((size.x * size.y * 4) as usize);
    for pixel in resized.pixels() {
        for c in 0..3 {
            let value = if pixel[3] > 0.0 {
                pixel[c] / pixel[3]
            } else {
                0.0
            };
            let encoded = if srgb { linear_to_srgb(value) } else { value };
            data.push((encoded.clamp(0.0, 1.0) * 255.0).round() as u8);
        }
        data.push((pixel[3].clamp(0.0, 1.0) * 255.0).round() as u8);
    }
    let mut result = original.clone();
    result.texture_descriptor.size.width = size.x;
    result.texture_descriptor.size.height = size.y;
    result.texture_descriptor.mip_level_count = 1;
    result.data = Some(data);
    Ok(result)
}
fn srgb_to_linear(v: f32) -> f32 {
    if v <= 0.04045 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}
fn linear_to_srgb(v: f32) -> f32 {
    if v <= 0.0031308 {
        v * 12.92
    } else {
        1.055 * v.powf(1.0 / 2.4) - 0.055
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn scaling_keeps_ui_on_the_active_window_camera() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<Assets<Image>>()
            .insert_resource(GraphicsOptions {
                current: GraphicsSettings::default(),
                saved: GraphicsSettings::default(),
                notice: String::new(),
            })
            .add_systems(Startup, setup_scaling)
            .add_systems(Update, apply_scale);
        app.world_mut().spawn((Window::default(), PrimaryWindow));
        let game_camera = app.world_mut().spawn(Camera3d::default()).id();
        for quality in [Quality::High, Quality::Low, Quality::Medium, Quality::High] {
            app.world_mut()
                .resource_mut::<GraphicsOptions>()
                .current
                .render_scale = quality;
            app.update();
            let ui_camera = app.world().resource::<ScaledView>().camera;
            let expected = if quality == Quality::High {
                game_camera
            } else {
                ui_camera
            };
            let world = app.world_mut();
            let mut cameras = world.query::<(Entity, &Camera, &RenderTarget)>();
            let selected = cameras
                .iter(world)
                .filter(|(_, _, target)| matches!(target, RenderTarget::Window(_)))
                .max_by_key(|(id, camera, _)| (camera.order, *id))
                .unwrap();
            assert_eq!(selected.0, expected);
            assert!(selected.1.is_active);
            assert_eq!(
                cameras
                    .iter(world)
                    .filter(|(_, camera, _)| camera.is_active)
                    .count(),
                if quality == Quality::High { 1 } else { 2 }
            );
        }
    }
    #[test]
    fn presets_custom_and_storage_validation() {
        for quality in Quality::ALL {
            let settings = GraphicsSettings::preset(quality);
            assert_eq!(settings.label(), quality.label());
            let encoded = serde_json::to_string(&SavedSettings {
                version: 1,
                graphics: settings,
            })
            .unwrap();
            assert_eq!(decode_settings(&encoded).unwrap(), settings);
        }
        let mut custom = GraphicsSettings::default();
        custom.msaa = false;
        assert_eq!(custom.label(), "Custom");
        for invalid in [
            "{}",
            "null",
            r#"{"version":2,"graphics":{"lighting":"high","textures":"high","render_scale":"high","msaa":true}}"#,
            r#"{"version":1,"graphics":{"lighting":"ultra","textures":"high","render_scale":"high","msaa":true}}"#,
        ] {
            assert!(decode_settings(invalid).is_err());
        }
        assert_eq!(
            scaled_size(UVec2::new(3073, 1920), Quality::Medium),
            UVec2::new(2304, 1440)
        );
        assert_eq!(scaled_size(UVec2::ONE, Quality::Low), UVec2::ONE);
    }
    #[test]
    fn texture_reduction_preserves_alpha_and_original_for_restoration() {
        use bevy::{
            asset::RenderAssetUsages,
            render::render_resource::{Extent3d, TextureDimension},
        };
        let original = Image::new(
            Extent3d {
                width: 2,
                height: 2,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            vec![255, 0, 0, 255, 0, 0, 255, 0, 255, 0, 0, 255, 0, 0, 255, 0],
            TextureFormat::Rgba8UnormSrgb,
            RenderAssetUsages::default(),
        );
        let before = original.data.clone();
        let reduced = reduce_texture(&original, Quality::Medium).unwrap();
        assert_eq!(reduced.size(), UVec2::ONE);
        assert_eq!(reduced.data.as_ref().unwrap(), &[255, 0, 0, 128]);
        assert_eq!(original.data, before);
        assert_eq!(
            reduced.texture_descriptor.format,
            original.texture_descriptor.format
        );
        let mut options = GraphicsOptions {
            current: GraphicsSettings::preset(Quality::Low),
            saved: GraphicsSettings::default(),
            notice: String::new(),
        };
        options.cancel();
        assert_eq!(options.current, GraphicsSettings::default());
    }
}
