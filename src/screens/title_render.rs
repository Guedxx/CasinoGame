use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use bevy::{
    prelude::*,
    render::{
        Render, RenderApp, RenderSystems,
        extract_resource::{ExtractResource, ExtractResourcePlugin},
        mesh::RenderMesh,
        render_asset::RenderAssets,
        render_resource::{CachedPipelineState, PipelineCache},
        texture::GpuImage,
    },
};

#[derive(Resource, Clone, Default, ExtractResource)]
pub(super) struct TitleRender {
    pub meshes: Vec<AssetId<Mesh>>,
    pub images: Vec<AssetId<Image>>,
    pub ready: Arc<AtomicBool>,
}

impl TitleRender {
    pub fn is_ready(&self) -> bool {
        self.ready.load(Ordering::Acquire)
    }
}

pub(super) fn install(app: &mut App) {
    app.init_resource::<TitleRender>();
    if app.get_sub_app(RenderApp).is_some() {
        app.add_plugins(ExtractResourcePlugin::<TitleRender>::default());
        app.sub_app_mut(RenderApp)
            .add_systems(Render, check_ready.after(RenderSystems::Render));
    }
}

fn check_ready(
    title: Res<TitleRender>,
    meshes: Res<RenderAssets<RenderMesh>>,
    images: Res<RenderAssets<GpuImage>>,
    pipelines: Res<PipelineCache>,
    mut previous_ready: Local<bool>,
) {
    if title.is_ready() {
        return;
    }
    let ready = !title.meshes.is_empty()
        && title.meshes.iter().all(|id| meshes.get(*id).is_some())
        && title.images.iter().all(|id| images.get(*id).is_some())
        && pipelines
            .pipelines()
            .all(|pipeline| matches!(pipeline.state, CachedPipelineState::Ok(_)));
    // Wait for a complete rendered frame after uploads and pipeline creation settle.
    title
        .ready
        .store(ready && *previous_ready, Ordering::Release);
    *previous_ready = ready;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn title_readiness_crosses_worlds_and_resets_for_a_new_scene() {
        let mut main = TitleRender::default();
        let render = main.clone();
        assert!(!main.is_ready());
        render.ready.store(true, Ordering::Release);
        assert!(main.is_ready());
        main = TitleRender::default();
        assert!(!main.is_ready());
    }
}
