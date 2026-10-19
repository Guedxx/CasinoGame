//! User-selected graphics quality at native resolution and original texture sizes.
use bevy::{
    anti_alias::{fxaa::Fxaa, smaa::Smaa},
    light::{DirectionalLightShadowMap, ShadowFilteringMethod},
    prelude::*,
};
use serde::{Deserialize, Serialize};

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

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AntiAliasing {
    Off,
    Fxaa,
    Smaa,
    #[default]
    Msaa4,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GraphicsSettings {
    pub lighting: Quality,
    pub anti_aliasing: AntiAliasing,
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
            anti_aliasing: match quality {
                Quality::Low => AntiAliasing::Off,
                Quality::Medium => AntiAliasing::Fxaa,
                Quality::High => AntiAliasing::Msaa4,
            },
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
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacySettings {
    version: u8,
    graphics: LegacyGraphics,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacyGraphics {
    lighting: Quality,
    #[serde(rename = "textures")]
    _textures: Quality,
    #[serde(rename = "render_scale")]
    _render_scale: Quality,
    msaa: bool,
}
fn decode_settings(json: &str) -> Result<GraphicsSettings, String> {
    if let Ok(saved) = serde_json::from_str::<LegacySettings>(json)
        && saved.version == 1
    {
        return Ok(GraphicsSettings {
            lighting: saved.graphics.lighting,
            anti_aliasing: if saved.graphics.msaa {
                AntiAliasing::Msaa4
            } else {
                AntiAliasing::Off
            },
        });
    }
    let saved: SavedSettings = serde_json::from_str(json).map_err(|e| e.to_string())?;
    if saved.version != 2 {
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
            version: 2,
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
            .add_systems(Update, apply_graphics.in_set(ApplyGraphics));
    }
}

fn apply_graphics(
    mut commands: Commands,
    options: Res<GraphicsOptions>,
    mut previous: Local<Option<GraphicsSettings>>,
    mut shadows: ResMut<DirectionalLightShadowMap>,
    cameras: Query<(Entity, Ref<Camera3d>), With<crate::game::camera::MainCamera>>,
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
    for (entity, camera) in &cameras {
        if !changed && !camera.is_added() {
            continue;
        }
        let mut camera = commands.entity(entity);
        camera.remove::<(Fxaa, Smaa)>().insert((
            if settings.anti_aliasing == AntiAliasing::Msaa4 {
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
        match settings.anti_aliasing {
            AntiAliasing::Fxaa => {
                camera.insert(Fxaa::default());
            }
            AntiAliasing::Smaa => {
                camera.insert(Smaa::default());
            }
            AntiAliasing::Off | AntiAliasing::Msaa4 => {}
        }
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
    use bevy::{camera::RenderTarget, core_pipeline::prepass::DepthPrepass};

    #[test]
    fn settings_round_trip_validate_and_migrate_without_resolution_controls() {
        for quality in Quality::ALL {
            let settings = GraphicsSettings::preset(quality);
            assert_eq!(settings.label(), quality.label());
            for anti_aliasing in [
                AntiAliasing::Off,
                AntiAliasing::Fxaa,
                AntiAliasing::Smaa,
                AntiAliasing::Msaa4,
            ] {
                let settings = GraphicsSettings {
                    anti_aliasing,
                    ..settings
                };
                let json = serde_json::to_string(&SavedSettings {
                    version: 2,
                    graphics: settings,
                })
                .unwrap();
                assert_eq!(decode_settings(&json).unwrap(), settings);
                assert!(!json.contains("textures") && !json.contains("render_scale"));
            }
        }
        for msaa in [false, true] {
            let json = format!(
                r#"{{"version":1,"graphics":{{"lighting":"medium","textures":"low","render_scale":"low","msaa":{msaa}}}}}"#
            );
            assert_eq!(
                decode_settings(&json).unwrap(),
                GraphicsSettings {
                    lighting: Quality::Medium,
                    anti_aliasing: if msaa {
                        AntiAliasing::Msaa4
                    } else {
                        AntiAliasing::Off
                    },
                }
            );
        }
        for invalid in [
            "{}",
            "null",
            r#"{"version":3,"graphics":{"lighting":"high","anti_aliasing":"msaa4"}}"#,
            r#"{"version":2,"graphics":{"lighting":"high","anti_aliasing":"msaa8"}}"#,
            r#"{"version":2,"graphics":{"lighting":"high","anti_aliasing":"off","render_scale":"low"}}"#,
            r#"{"version":1,"graphics":{"lighting":"medium","textures":"invalid","render_scale":"low","msaa":false}}"#,
        ] {
            assert!(decode_settings(invalid).is_err());
        }
        let mut options = GraphicsOptions {
            current: GraphicsSettings {
                lighting: Quality::High,
                anti_aliasing: AntiAliasing::Smaa,
            },
            saved: GraphicsSettings::default(),
            notice: String::new(),
        };
        assert_eq!(options.current.label(), "Custom");
        options.cancel();
        assert_eq!(options.current, options.saved);
    }

    #[test]
    fn switching_antialiasing_is_exclusive_and_preserves_the_window_camera() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<DirectionalLightShadowMap>()
            .init_resource::<UiScale>()
            .insert_resource(GraphicsOptions {
                current: GraphicsSettings::default(),
                saved: GraphicsSettings::default(),
                notice: String::new(),
            })
            .add_systems(Update, apply_graphics);
        let camera = app
            .world_mut()
            .spawn((
                Camera3d::default(),
                DepthPrepass,
                crate::game::camera::MainCamera,
            ))
            .id();
        for mode in [
            AntiAliasing::Msaa4,
            AntiAliasing::Fxaa,
            AntiAliasing::Smaa,
            AntiAliasing::Off,
            AntiAliasing::Smaa,
            AntiAliasing::Fxaa,
            AntiAliasing::Msaa4,
        ] {
            app.world_mut()
                .resource_mut::<GraphicsOptions>()
                .current
                .anti_aliasing = mode;
            app.update();
            let camera = app.world().entity(camera);
            assert_eq!(camera.contains::<Fxaa>(), mode == AntiAliasing::Fxaa);
            assert_eq!(camera.contains::<Smaa>(), mode == AntiAliasing::Smaa);
            assert_eq!(
                *camera.get::<Msaa>().unwrap(),
                if mode == AntiAliasing::Msaa4 {
                    Msaa::Sample4
                } else {
                    Msaa::Off
                }
            );
            assert!(camera.contains::<DepthPrepass>());
            assert!(matches!(
                camera.get::<RenderTarget>(),
                Some(RenderTarget::Window(_))
            ));
            assert!(camera.get::<Camera>().unwrap().viewport.is_none());
            assert_eq!(app.world().resource::<UiScale>().0, 1.0);
        }
        let world = app.world_mut();
        assert_eq!(world.query::<&Camera>().iter(world).count(), 1);
    }
}
