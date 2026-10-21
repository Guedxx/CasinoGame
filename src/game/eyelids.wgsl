#import bevy_ui::ui_vertex_output::UiVertexOutput

@group(1) @binding(0) var<uniform> opening: vec4<f32>;

@fragment
fn fragment(in: UiVertexOutput) -> @location(0) vec4<f32> {
    if opening.x <= 0.0 {
        return vec4(0.0, 0.0, 0.0, 1.0);
    }
    let p = in.uv * 2.0 - 1.0;
    // Both lids meet at the corners; the aperture clears the entire viewport when open.
    let edge = abs(p.y) + 0.45 * p.x * p.x - 1.5 * opening.x;
    let feather = 2.0 / max(in.size.y, 1.0);
    return vec4(0.0, 0.0, 0.0, smoothstep(-feather, feather, edge));
}
