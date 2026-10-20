//! Subtle affine UVs and camera-only vertex snapping inspired by Math-Man/URP-PSX-FORKED.
use bevy::{
    prelude::*,
    shader::{Shader, Source},
};

use super::GraphicsOptions;

const PATHS: [&str; 4] = [
    "embedded://bevy_pbr/render/forward_io.wgsl",
    "embedded://bevy_pbr/render/view_transformations.wgsl",
    "embedded://bevy_pbr/render/mesh.wgsl",
    "embedded://bevy_pbr/render/pbr.wgsl",
];
const PROJECTION: &str = "fn position_world_to_clip(world_pos: vec3<f32>) -> vec4<f32> {\n    let clip_pos = view_bindings::view.clip_from_world * vec4(world_pos, 1.0);\n    return clip_pos;\n}";
// Bevy 0.19.1 camera prepasses use the default projection key; shadow prepasses set it explicitly.
const CAMERA_PASS: &str = r#"
fn psx_camera_pass() -> bool {
#ifdef PREPASS_PIPELINE
#ifndef VIEW_PROJECTION_NONSTANDARD
    return false;
#else
    return true;
#endif
#else
    return true;
#endif
}
"#;
const SNAPPED_PROJECTION: &str = r#"fn position_world_to_clip(world_pos: vec3<f32>) -> vec4<f32> {
    let clip_pos = view_bindings::view.clip_from_world * vec4(world_pos, 1.0);
    if !psx_camera_pass() || clip_pos.w <= 0.00001 {
        return clip_pos;
    }
    // One framebuffer pixel per cell; shadow lookup retains the original world position.
    let grid = max(view_bindings::view.viewport.zw * 0.5, vec2<f32>(0.5));
    let snapped = floor(clip_pos.xy / clip_pos.w * grid + 0.5) / grid;
    return vec4<f32>(snapped * clip_pos.w, clip_pos.zw);
}"#;
const WARP_UVS: &str = r#"
fn psx_warp_uvs(vertex: VertexOutput) -> VertexOutput {
    var out = vertex;
#ifndef MAY_DISCARD
    // Preserve cutout silhouettes in the camera depth pass and every light's shadow map.
    // Cap displacement on large tiled surfaces and triangles crossing the near plane.
#ifdef VERTEX_UVS_A
    out.uv = vertex.uv + clamp((vertex.psx_affine_uv.xy - vertex.uv) * 0.2, vec2(-0.01), vec2(0.01));
#endif
#ifdef VERTEX_UVS_B
    out.uv_b = vertex.uv_b + clamp((vertex.psx_affine_uv.zw - vertex.uv_b) * 0.2, vec2(-0.01), vec2(0.01));
#endif
#endif
    return out;
}
"#;
const AFFINE_UVS: &str = r#"
#ifdef VERTEX_UVS_A
    out.psx_affine_uv.x = vertex.uv.x;
    out.psx_affine_uv.y = vertex.uv.y;
#endif
#ifdef VERTEX_UVS_B
    out.psx_affine_uv.z = vertex.uv_b.x;
    out.psx_affine_uv.w = vertex.uv_b.y;
#endif
"#;

// shortcut: patch targets and shadow classification are Bevy 0.19.1 internals, review on engine upgrades.
fn patch_sources(sources: &[String], warping: bool, jitter: bool) -> Option<Vec<String>> {
    if sources.len() != PATHS.len() || sources.iter().any(|s| s.contains("psx_")) {
        return None;
    }
    let mut patched = sources.to_vec();
    let target = "struct VertexOutput {";
    if patched[0].matches(target).count() != 1 {
        return None;
    }
    let (_, output) = patched[0].split_once(target)?;
    if [" uv: vec2<f32>,", " uv_b: vec2<f32>,"]
        .iter()
        .any(|field| output.matches(field).count() != 1)
    {
        return None;
    }
    if warping {
        patched[0] = patched[0].replace(
            target,
            &format!("{target}\n    @location(8) @interpolate(linear) psx_affine_uv: vec4<f32>,"),
        );
        patched[0].push_str(WARP_UVS);
    }
    if patched[1].matches(PROJECTION).count() != 1 {
        return None;
    }
    if jitter {
        patched[1] = patched[1].replace(PROJECTION, SNAPPED_PROJECTION);
        patched[1].push_str(CAMERA_PASS);
    }
    let target = "    return out;";
    if patched[2].matches(target).count() != 1 {
        return None;
    }
    if warping {
        patched[2] = patched[2].replace(target, &format!("{AFFINE_UVS}{target}"));
    }
    let target = "    var in = vertex_output;";
    if patched[3].matches(target).count() != 1 {
        return None;
    }
    if warping {
        // Camera depth only samples UVs for cutouts, which deliberately retain perspective UVs.
        patched[3] = patched[3].replace(
            target,
            r#"#ifdef PREPASS_PIPELINE
    var in = vertex_output;
#else
    var in = bevy_pbr::forward_io::psx_warp_uvs(vertex_output);
#endif"#,
        );
    }
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
        vec![
            "struct Vertex {\n    @location(2) uv: vec2<f32>,\n}\nstruct VertexOutput {\n    @location(2) uv: vec2<f32>,\n    @location(3) uv_b: vec2<f32>,\n    @location(4) world_normal: vec3<f32>,\n}".into(),
            format!("before\n{PROJECTION}\nafter"),
            "@vertex fn vertex() {\n    return out;\n}\n@fragment fn fragment() {}".into(),
            "    var in = vertex_output;".into(),
        ]
    }

    #[test]
    fn psx_sources_toggle_independently_and_restore_originals() {
        let originals = sources();
        for warping in [false, true] {
            for jitter in [false, true] {
                let patched = patch_sources(&originals, warping, jitter).unwrap();
                let (input, output) = patched[0].split_once("struct VertexOutput {").unwrap();
                assert!(!input.contains("@interpolate"));
                assert_eq!(output.contains("@interpolate(linear)"), warping);
                assert!(output.contains("@location(2) uv: vec2<f32>,"));
                assert!(output.contains("@location(4) world_normal: vec3<f32>,"));
                assert_eq!(patched[1].contains("let snapped ="), jitter);
                assert_eq!(patched[1].contains("if !psx_camera_pass()"), jitter);
                assert_eq!(patched[2].contains("psx_affine_uv"), warping);
                assert_eq!(patched[3].contains("psx_warp_uvs"), warping);
                if !warping && !jitter {
                    assert_eq!(patched, originals);
                }
            }
        }
    }

    #[test]
    fn psx_sources_reject_missing_duplicate_or_already_patched_targets() {
        for index in 0..PATHS.len() {
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
        originals[0] = originals[0].replace(" uv_b:", " missing_uv:");
        assert!(patch_sources(&originals, true, true).is_none());
    }
}
