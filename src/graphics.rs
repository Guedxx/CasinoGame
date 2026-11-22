//! User-selected graphics quality at native resolution and original texture sizes.
use bevy::{
    asset::AssetEventSystems,
    image::{ImageFilterMode, ImageSampler},
    light::DirectionalLightShadowMap,
    prelude::*,
    text::FontSmoothing,
    ui::UiSystems,
};
use serde::{Deserialize, Serialize};

mod psx;

// Keep the existing storage location so version-one preferences can migrate.
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
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum LegacyAa {
    Off,
    Fxaa,
    Smaa,
    Msaa4,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GraphicsSettings {
    pub lighting: Quality,
    #[serde(default = "enabled")]
    pub texture_warping: bool,
    #[serde(default = "enabled")]
    pub vertex_jitter: bool,
}
fn enabled() -> bool {
    true
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
            texture_warping: true,
            vertex_jitter: true,
        }
    }
    pub fn label(self) -> &'static str {
        if self.texture_warping && self.vertex_jitter {
            self.lighting.label()
        } else {
            "Custom"
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SavedSettings<T = GraphicsSettings> {
    version: u8,
    graphics: T,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacyGraphics {
    lighting: Quality,
    #[serde(rename = "textures")]
    _textures: Quality,
    #[serde(rename = "render_scale")]
    _render_scale: Quality,
    #[serde(rename = "msaa")]
    _msaa: bool,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacyAaGraphics {
    lighting: Quality,
    #[serde(rename = "anti_aliasing")]
    _anti_aliasing: LegacyAa,
    #[serde(default = "enabled")]
    texture_warping: bool,
    #[serde(default = "enabled")]
    vertex_jitter: bool,
}
fn decode_settings(json: &str) -> Result<GraphicsSettings, String> {
    if let Ok(saved) = serde_json::from_str::<SavedSettings<LegacyGraphics>>(json)
        && saved.version == 1
    {
        return Ok(GraphicsSettings {
            lighting: saved.graphics.lighting,
            ..GraphicsSettings::default()
        });
    }
    if let Ok(saved) = serde_json::from_str::<SavedSettings<LegacyAaGraphics>>(json)
        && matches!(saved.version, 2 | 3)
    {
        return Ok(GraphicsSettings {
            lighting: saved.graphics.lighting,
            texture_warping: saved.graphics.texture_warping,
            vertex_jitter: saved.graphics.vertex_jitter,
        });
    }
    let saved: SavedSettings = serde_json::from_str(json).map_err(|e| e.to_string())?;
    if saved.version != 3 {
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
            version: 3,
            graphics: self.saved,
        })
        .map_err(|e| e.to_string())
        .and_then(|json| write_settings(&json));
        self.notice = match result {
            Ok(()) => String::new(),
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
            .add_systems(Update, apply_graphics.in_set(ApplyGraphics))
            .add_systems(Update, psx::apply.after(ApplyGraphics))
            .add_systems(PostUpdate, unfilter_text.before(UiSystems::Prepare))
            .add_systems(PostUpdate, unfilter_images.after(AssetEventSystems));
    }
}

fn unfilter_text(
    mut fonts: Query<(&mut TextFont, Option<&Text>), Or<(Changed<TextFont>, Changed<Text>)>>,
) {
    for (mut font, text) in &mut fonts {
        let smoothing = if text.is_some_and(|text| {
            text.0.chars().any(|ch| {
                (!ch.is_ascii() && ch.is_alphabetic()) || matches!(ch, '\u{0300}'..='\u{036f}')
            })
        }) {
            FontSmoothing::AntiAliased
        } else {
            FontSmoothing::None
        };
        if font.font_smoothing != smoothing {
            font.font_smoothing = smoothing;
        }
    }
}

fn unfilter_sampler(sampler: &mut ImageSampler) {
    let descriptor = sampler.get_or_init_descriptor();
    descriptor.set_filter(ImageFilterMode::Nearest);
    descriptor.anisotropy_clamp = 1;
}

fn unfilter_images(
    mut events: MessageReader<AssetEvent<Image>>,
    assets: Res<AssetServer>,
    mut images: ResMut<Assets<Image>>,
) {
    for event in events.read() {
        if let AssetEvent::Added { id } | AssetEvent::Modified { id } = event
            && let Some(path) = assets.get_path(*id)
            && (path.path().starts_with("models") || path.path().starts_with("ui"))
            && let Some(image) = images.get_mut_untracked(*id)
        {
            // The existing asset event uploads this change; avoid emitting another event.
            unfilter_sampler(&mut image.sampler);
        }
    }
}

fn apply_graphics(
    options: Res<GraphicsOptions>,
    mut previous: Local<Option<GraphicsSettings>>,
    mut shadows: ResMut<DirectionalLightShadowMap>,
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

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::{
        core_pipeline::tonemapping::{DebandDither, Tonemapping},
        image::{ImageAddressMode, ImageSamplerDescriptor},
        light::ShadowFilteringMethod,
        ui_render::UiAntiAlias,
    };

    #[test]
    fn settings_round_trip_validate_and_migrate_without_filters() {
        for quality in Quality::ALL {
            let settings = GraphicsSettings::preset(quality);
            assert_eq!(settings.label(), quality.label());
            let json = serde_json::to_string(&SavedSettings {
                version: 3,
                graphics: settings,
            })
            .unwrap();
            assert_eq!(decode_settings(&json).unwrap(), settings);
            assert_eq!(
                serde_json::from_str::<serde_json::Value>(&json).unwrap(),
                serde_json::json!({ "version": 3, "graphics": { "lighting": quality, "texture_warping": true, "vertex_jitter": true } })
            );
            for mode in ["off", "fxaa", "smaa", "msaa4"] {
                let legacy = serde_json::json!({ "version": 2, "graphics": {
                    "lighting": quality, "anti_aliasing": mode,
                }});
                assert_eq!(decode_settings(&legacy.to_string()).unwrap(), settings);
            }
            for msaa in [false, true] {
                let legacy = serde_json::json!({ "version": 1, "graphics": {
                    "lighting": quality, "textures": "low", "render_scale": "low", "msaa": msaa,
                }});
                assert_eq!(decode_settings(&legacy.to_string()).unwrap(), settings);
            }
        }
        for invalid in [
            "{}",
            "null",
            r#"{"version":3,"graphics":{"lighting":"high","texture_warping":1}}"#,
            r#"{"version":3,"graphics":{"lighting":"high","vertex_jitter":"true"}}"#,
            r#"{"version":4,"graphics":{"lighting":"high"}}"#,
            r#"{"version":2,"graphics":{"lighting":"high","anti_aliasing":"msaa8"}}"#,
            r#"{"version":2,"graphics":{"lighting":"high","anti_aliasing":"off","render_scale":"low"}}"#,
            r#"{"version":1,"graphics":{"lighting":"medium","textures":"invalid","render_scale":"low","msaa":false}}"#,
            r#"{"version":3,"graphics":{"lighting":"invalid"}}"#,
            r#"{"version":3,"graphics":{"lighting":"high","anti_aliasing":"msaa8"}}"#,
            r#"{"version":2,"graphics":{"lighting":"high"}}"#,
        ] {
            assert!(decode_settings(invalid).is_err(), "{invalid}");
        }
        let mut options = GraphicsOptions {
            current: GraphicsSettings::preset(Quality::Low),
            saved: GraphicsSettings::default(),
            notice: String::new(),
        };
        options.cancel();
        assert_eq!(options.current, options.saved);
    }

    #[test]
    fn psx_settings_preserve_effect_choices_and_migrate_both_version_three_formats() {
        for warping in [false, true] {
            for jitter in [false, true] {
                let settings = GraphicsSettings {
                    lighting: Quality::High,
                    texture_warping: warping,
                    vertex_jitter: jitter,
                };
                let saved = serde_json::to_string(&SavedSettings {
                    version: 3,
                    graphics: settings,
                })
                .unwrap();
                assert_eq!(decode_settings(&saved).unwrap(), settings);
                let old_psx = serde_json::json!({ "version": 3, "graphics": {
                    "lighting": "high", "anti_aliasing": "smaa",
                    "texture_warping": warping, "vertex_jitter": jitter,
                }});
                assert_eq!(decode_settings(&old_psx.to_string()).unwrap(), settings);
                assert_eq!(
                    settings.label(),
                    if warping && jitter { "High" } else { "Custom" }
                );
                let mut options = GraphicsOptions {
                    current: settings,
                    saved: GraphicsSettings::default(),
                    notice: String::new(),
                };
                options.cancel();
                assert_eq!(options.current, options.saved);
            }
        }
        assert_eq!(
            decode_settings(r#"{"version":3,"graphics":{"lighting":"high"}}"#).unwrap(),
            GraphicsSettings::default()
        );
    }

    #[test]
    fn nearest_sampler_preserves_addressing_and_text_stays_unsmoothed() {
        let mut sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
            address_mode_u: ImageAddressMode::Repeat,
            address_mode_v: ImageAddressMode::MirrorRepeat,
            anisotropy_clamp: 16,
            ..ImageSamplerDescriptor::linear()
        });
        unfilter_sampler(&mut sampler);
        let descriptor = sampler.get_or_init_descriptor();
        assert_eq!(descriptor.mag_filter, ImageFilterMode::Nearest);
        assert_eq!(descriptor.min_filter, ImageFilterMode::Nearest);
        assert_eq!(descriptor.mipmap_filter, ImageFilterMode::Nearest);
        assert_eq!(descriptor.anisotropy_clamp, 1);
        assert_eq!(descriptor.address_mode_u, ImageAddressMode::Repeat);
        assert_eq!(descriptor.address_mode_v, ImageAddressMode::MirrorRepeat);
        let mut app = App::new();
        app.add_systems(Update, unfilter_text);
        let text = app.world_mut().spawn(TextFont::default()).id();
        app.update();
        assert_eq!(
            app.world().get::<TextFont>(text).unwrap().font_smoothing,
            FontSmoothing::None
        );
        app.world_mut()
            .get_mut::<TextFont>(text)
            .unwrap()
            .font_smoothing = FontSmoothing::AntiAliased;
        app.update();
        assert_eq!(
            app.world().get::<TextFont>(text).unwrap().font_smoothing,
            FontSmoothing::None
        );
    }

    #[test]
    fn accented_ui_text_keeps_antialiasing_after_text_changes() {
        let mut app = App::new();
        app.add_systems(Update, unfilter_text);
        let text = app.world_mut().spawn(Text::new("Settings")).id();
        app.update();
        assert_eq!(app.world().get::<TextFont>(text).unwrap().font_smoothing, FontSmoothing::None);

        app.world_mut().get_mut::<Text>(text).unwrap().0 = "Ação · açúcar · cafe\u{0301}".into();
        app.update();
        assert_eq!(app.world().get::<TextFont>(text).unwrap().font_smoothing, FontSmoothing::AntiAliased);
    }

    #[test]
    fn lighting_presets_preserve_unfiltered_camera_and_shadow_quality() {
        let mut app = App::new();
        app.init_resource::<DirectionalLightShadowMap>()
            .insert_resource(GraphicsOptions {
                current: GraphicsSettings::default(),
                saved: GraphicsSettings::default(),
                notice: String::new(),
            })
            .insert_resource(ClearColor::default())
            .add_plugins(crate::game::camera::CameraPlugin)
            .add_systems(Update, apply_graphics);
        let spot = app.world_mut().spawn(SpotLight::default()).id();
        for quality in Quality::ALL {
            app.world_mut().resource_mut::<GraphicsOptions>().current =
                GraphicsSettings::preset(quality);
            app.update();
            let world = app.world_mut();
            let mut cameras = world.query::<(
                &Msaa,
                &Tonemapping,
                &DebandDither,
                &UiAntiAlias,
                &ShadowFilteringMethod,
            )>();
            let (msaa, tone, dither, ui, shadow) = cameras.single(world).unwrap();
            assert_eq!(*msaa, Msaa::Off);
            assert_eq!(*tone, Tonemapping::None);
            assert_eq!(*dither, DebandDither::Disabled);
            assert_eq!(*ui, UiAntiAlias::Off);
            assert_eq!(*shadow, ShadowFilteringMethod::Hardware2x2);
            assert_eq!(
                world.get::<SpotLight>(spot).unwrap().shadow_maps_enabled,
                quality != Quality::Low
            );
            assert_eq!(
                world.resource::<DirectionalLightShadowMap>().size,
                if quality == Quality::Medium {
                    1024
                } else {
                    2048
                }
            );
        }
    }
}
