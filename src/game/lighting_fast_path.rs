use bevy::{
    prelude::*,
    shader::{Shader, Source},
};

const SHADER_PATH: &str = "embedded://bevy_pbr/render/pbr_functions.wgsl";
const SPOT_START: &str = "    // Spot lights (direct)\n";
const SPOT_END: &str = "    // directional lights (direct)\n";
const SHADOW_START: &str = "        var shadow: f32 = 1.0;\n";
const LIGHT_START: &str = "        let light_contrib = lighting::spot_light(light_id, &lighting_input, enable_diffuse);\n";
const LIGHT_END: &str = "        direct_light += light_contrib * shadow;\n";
const GUARD_START: &str = "#ifndef NO_ARRAY_TEXTURES_SUPPORT\n        if casino_spot_light_can_contribute(light_id, lighting_input.P) {\n#endif\n";
const GUARD_END: &str = "#ifndef NO_ARRAY_TEXTURES_SUPPORT\n        }\n#endif\n";
const ATTENUATION: &str = r#"
#ifndef NO_ARRAY_TEXTURES_SUPPORT
fn casino_spot_light_can_contribute(light_id: u32, position: vec3<f32>) -> bool {
    let light = &view_bindings::clustered_lights.data[light_id];
    let light_to_frag = (*light).position_radius.xyz - position;
    let distance_square = dot(light_to_frag, light_to_frag);
    if lighting::getDistanceAttenuation(distance_square, (*light).color_inverse_square_range.w) == 0.0 {
        return false;
    }
    var spot_dir = vec3<f32>((*light).light_custom_data.x, 0.0, (*light).light_custom_data.y);
    spot_dir.y = sqrt(max(0.0, 1.0 - spot_dir.x * spot_dir.x - spot_dir.z * spot_dir.z));
    if ((*light).flags & mesh_view_types::POINT_LIGHT_FLAGS_SPOT_LIGHT_Y_NEGATIVE) != 0u {
        spot_dir.y = -spot_dir.y;
    }
    let cd = dot(-spot_dir, normalize(light_to_frag));
    let attenuation = saturate(cd * (*light).light_custom_data.z + (*light).light_custom_data.w);
    let spot_attenuation = attenuation * attenuation;
    return spot_attenuation != 0.0;
}
#endif
"#;

/// Install after DefaultPlugins; add this system to Update until the embedded shader loads.
pub fn install(
    asset_server: Res<AssetServer>,
    mut shaders: ResMut<Assets<Shader>>,
    mut handle: Local<Option<Handle<Shader>>>,
    mut finished: Local<bool>,
) {
    if *finished {
        return;
    }
    let handle = handle.get_or_insert_with(|| asset_server.load(SHADER_PATH));
    let Some(mut shader) = shaders.get_mut(&*handle) else {
        return;
    };
    *finished = true;
    let Source::Wgsl(source) = &shader.source else {
        warn!("Spotlight fast path requires Bevy 0.19.1 WGSL; retaining the original shader");
        return;
    };
    let Some(patched) = patch_source(source) else {
        warn!("Bevy spotlight shader differs from 0.19.1; retaining the original shader");
        return;
    };
    // Keep the original asset ID and import metadata so dependent pipelines reload together.
    shader.source = Source::Wgsl(patched.into());
    info!("Exact-zero spotlight fast path installed");
}

fn patch_source(source: &str) -> Option<String> {
    if source.contains("fn casino_spot_light_can_contribute(")
        || [SPOT_START, SPOT_END]
            .iter()
            .any(|marker| source.matches(*marker).count() != 1)
    {
        return None;
    }
    let (before, rest) = source.split_once(SPOT_START)?;
    let (spot, after) = rest.split_once(SPOT_END)?;
    if [SHADOW_START, LIGHT_START, LIGHT_END]
        .iter()
        .any(|marker| spot.matches(*marker).count() != 1)
    {
        return None;
    }
    let shadow = spot.find(SHADOW_START)?;
    let light = spot.find(LIGHT_START)?;
    let end = spot.find(LIGHT_END)? + LIGHT_END.len();
    if shadow >= light || light >= end {
        return None;
    }
    // The displaced transmission lobe stays outside this guard; WebGL keeps uniform shadow reads.
    Some(format!(
        "{before}{SPOT_START}{}{GUARD_START}{}        if shadow != 0.0 {{\n{}        }}\n{GUARD_END}{}{SPOT_END}{after}{ATTENUATION}",
        &spot[..shadow],
        &spot[shadow..light],
        &spot[light..end],
        &spot[end..],
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> String {
        format!(
            "point lights stay untouched\n{SPOT_START}spot setup\n{SHADOW_START}        shadow = fetch_shadow();\n{LIGHT_START}{LIGHT_END}\n#ifdef STANDARD_MATERIAL_DIFFUSE_TRANSMISSION\ntransmission stays untouched\n#endif\n{SPOT_END}sun and fog stay untouched\n"
        )
    }

    #[test]
    fn preserves_positive_lighting_transmission_and_other_lights() {
        let source = fixture();
        let patched = patch_source(&source).unwrap();
        assert!(patched.starts_with("point lights stay untouched\n"));
        assert!(patched.contains(&format!("{SHADOW_START}        shadow = fetch_shadow();\n")));
        assert!(patched.contains(&format!("{LIGHT_START}{LIGHT_END}")));
        assert!(patched.contains(&format!(
            "{GUARD_END}\n#ifdef STANDARD_MATERIAL_DIFFUSE_TRANSMISSION\ntransmission stays untouched\n#endif\n{SPOT_END}sun and fog stay untouched\n"
        )));
        assert!(patched.ends_with(ATTENUATION));
        assert_eq!(patched.matches(GUARD_START).count(), 1);
        assert!(patch_source(&patched).is_none());
    }

    #[test]
    fn rejects_missing_duplicate_and_reordered_patch_targets() {
        let source = fixture();
        for marker in [SPOT_START, SPOT_END, SHADOW_START, LIGHT_START, LIGHT_END] {
            assert!(patch_source(&source.replace(marker, "")).is_none());
            assert!(patch_source(&source.replace(marker, &marker.repeat(2))).is_none());
        }
        let reordered = source
            .replace(SHADOW_START, "")
            .replace(LIGHT_END, &format!("{LIGHT_END}{SHADOW_START}"));
        assert!(patch_source(&reordered).is_none());
    }
}
