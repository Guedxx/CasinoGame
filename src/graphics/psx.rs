//! Affine UVs and vertex snapping inspired by Math-Man/URP-PSX-FORKED.
use bevy::{
    prelude::*,
    shader::{Shader, Source},
};

use super::GraphicsOptions;

const PATHS: [&str; 3] = [
    "embedded://bevy_pbr/render/forward_io.wgsl",
    "embedded://bevy_pbr/prepass/prepass_io.wgsl",
    "embedded://bevy_pbr/render/view_transformations.wgsl",
];
const PROJECTION: &str = "fn position_world_to_clip(world_pos: vec3<f32>) -> vec4<f32> {\n    let clip_pos = view_bindings::view.clip_from_world * vec4(world_pos, 1.0);\n    return clip_pos;\n}";
const SNAPPED_PROJECTION: &str = r#"fn position_world_to_clip(world_pos: vec3<f32>) -> vec4<f32> {
    let clip_pos = view_bindings::view.clip_from_world * vec4(world_pos, 1.0);
    if clip_pos.w <= 0.00001 {
        return clip_pos;
    }
    // Two framebuffer pixels per cell; keep depth and perspective unchanged.
    let grid = max(view_bindings::view.viewport.zw * 0.25, vec2<f32>(0.5));
    let snapped = floor(clip_pos.xy / clip_pos.w * grid + 0.5) / grid;
    return vec4<f32>(snapped * clip_pos.w, clip_pos.zw);
}"#;

// shortcut: patch targets are Bevy 0.19.1 internals, review these when upgrading Bevy.
fn patch_sources(sources: &[String], warping: bool, jitter: bool) -> Option<Vec<String>> {
    let [forward, prepass, projection] = sources else {
        return None;
    };
    let mut patched = Vec::with_capacity(3);
    for source in [forward, prepass] {
        if source.matches("struct VertexOutput {").count() != 1 {
            return None;
        }
        let (input, output) = source.split_once("struct VertexOutput {")?;
        if output.contains("@interpolate(linear) uv") {
            return None;
        }
        let mut output = output.to_owned();
        for field in [" uv: vec2<f32>,", " uv_b: vec2<f32>,"] {
            if output.matches(field).count() != 1 {
                return None;
            }
            if warping {
                output = output.replace(field, &format!(" @interpolate(linear){field}"));
            }
        }
        patched.push(format!("{input}struct VertexOutput {{{output}"));
    }
    if projection.matches(PROJECTION).count() != 1 {
        return None;
    }
    patched.push(if jitter {
        projection.replace(PROJECTION, SNAPPED_PROJECTION)
    } else {
        projection.clone()
    });
    Some(patched)
}

pub(super) fn apply(
    asset_server: Res<AssetServer>,
    mut shaders: ResMut<Assets<Shader>>,
    options: Res<GraphicsOptions>,
    mut handles: Local<Vec<Handle<Shader>>>,
    mut originals: Local<Vec<String>>,
    mut previous: Local<Option<(bool, bool)>>,
    mut failed: Local<bool>,
) {
    let effects = (
        options.current.texture_warping,
        options.current.vertex_jitter,
    );
    if *failed || *previous == Some(effects) {
        return;
    }
    if handles.is_empty() {
        *handles = PATHS.map(|path| asset_server.load(path)).into();
    }
    if originals.is_empty() {
        let Some(loaded) = handles
            .iter()
            .map(|h| shaders.get(h))
            .collect::<Option<Vec<_>>>()
        else {
            if handles.iter().any(|handle| {
                matches!(
                    asset_server.load_state(handle.id()),
                    bevy::asset::LoadState::Failed(_)
                )
            }) {
                warn!("PSX shader loading failed; retaining original shaders");
                *failed = true;
            }
            return;
        };
        let Some(sources) = loaded
            .iter()
            .map(|shader| match &shader.source {
                Source::Wgsl(source) => Some(source.to_string()),
                _ => None,
            })
            .collect::<Option<Vec<_>>>()
        else {
            warn!("PSX effects require Bevy 0.19.1 WGSL; retaining original shaders");
            *failed = true;
            return;
        };
        *originals = sources;
    }
    let Some(patched) = patch_sources(&originals, effects.0, effects.1) else {
        warn!("PSX shader targets differ from Bevy 0.19.1; retaining original shaders");
        *failed = true;
        return;
    };
    if handles.iter().any(|handle| shaders.get(handle).is_none()) {
        return;
    }
    for (handle, source) in handles.iter().zip(patched) {
        if shaders.get(handle).unwrap().source.as_str() != source {
            // Preserve asset IDs and import metadata so dependent pipelines reload together.
            shaders.get_mut(handle).unwrap().source = Source::Wgsl(source.into());
        }
    }
    *previous = Some(effects);
    info!(
        "PSX effects: texture warping {}, model jitter {}",
        effects.0, effects.1
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sources() -> Vec<String> {
        let io = "struct Vertex {\n    @location(2) uv: vec2<f32>,\n}\nstruct VertexOutput {\n    @location(2) uv: vec2<f32>,\n    @location(3) uv_b: vec2<f32>,\n    @location(4) world_normal: vec3<f32>,\n}";
        vec![io.into(), io.into(), format!("before\n{PROJECTION}\nafter")]
    }

    #[test]
    fn psx_sources_toggle_independently_and_restore_originals() {
        let originals = sources();
        for warping in [false, true] {
            for jitter in [false, true] {
                let patched = patch_sources(&originals, warping, jitter).unwrap();
                for source in &patched[..2] {
                    let (input, output) = source.split_once("struct VertexOutput {").unwrap();
                    assert!(!input.contains("@interpolate"));
                    assert_eq!(
                        output.matches("@interpolate(linear)").count(),
                        if warping { 2 } else { 0 }
                    );
                    assert!(output.contains("@location(4) world_normal: vec3<f32>,"));
                }
                assert_eq!(patched[2].contains("let snapped ="), jitter);
                assert!(patched[2].starts_with("before\n") && patched[2].ends_with("\nafter"));
                if !warping && !jitter {
                    assert_eq!(patched, originals);
                }
                assert_eq!(patch_sources(&originals, false, false).unwrap(), originals);
            }
        }
    }

    #[test]
    fn psx_sources_reject_missing_duplicate_or_already_patched_targets() {
        for index in 0..3 {
            let mut originals = sources();
            originals[index].clear();
            assert!(patch_sources(&originals, true, true).is_none());
            let mut originals = sources();
            originals[index] = originals[index].repeat(2);
            assert!(patch_sources(&originals, true, true).is_none());
        }
        let patched = patch_sources(&sources(), true, true).unwrap();
        assert!(patch_sources(&patched, true, true).is_none());
        let mut originals = sources();
        originals[1] = originals[1].replace(" uv_b:", " missing_uv:");
        assert!(patch_sources(&originals, true, true).is_none());
    }
}
