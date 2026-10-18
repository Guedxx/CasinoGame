#![cfg(any(target_arch = "wasm32", test))]

use bevy::{light::cluster::GlobalClusterSettings, prelude::*};

/// Run in Startup because PbrPlugin::finish creates the device-specific settings.
pub fn use_cpu_clustering(mut settings: ResMut<GlobalClusterSettings>) {
    settings.gpu_clustering = None;
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::light::cluster::GlobalClusterGpuSettings;

    #[test]
    fn startup_preserves_device_capabilities_and_limits() {
        let original = GlobalClusterSettings {
            supports_storage_buffers: true,
            clustered_decals_are_usable: true,
            gpu_clustering: Some(GlobalClusterGpuSettings {
                initial_z_slice_list_capacity: 1024,
                initial_index_list_capacity: 65536,
            }),
            max_uniform_buffer_clusterable_objects: 123,
            view_cluster_bindings_max_indices: 23456,
        };
        let mut app = App::new();
        app.insert_resource(original.clone())
            .add_systems(Startup, use_cpu_clustering);
        app.update();

        let settings = app.world().resource::<GlobalClusterSettings>();
        assert!(settings.gpu_clustering.is_none());
        assert_eq!(
            settings.supports_storage_buffers,
            original.supports_storage_buffers
        );
        assert_eq!(
            settings.clustered_decals_are_usable,
            original.clustered_decals_are_usable
        );
        assert_eq!(
            settings.max_uniform_buffer_clusterable_objects,
            original.max_uniform_buffer_clusterable_objects
        );
        assert_eq!(
            settings.view_cluster_bindings_max_indices,
            original.view_cluster_bindings_max_indices
        );
    }
}
