mod bind_groups;
mod cache;
mod data;
mod pipeline;
pub mod types;

pub use crate::raytracing::bevy::types::{
    BoxTreeGPUHost, BoxTreeGPUView, BoxTreeSpyGlass, RenderBevyPlugin, VhxViewSet, Viewport,
};
use crate::{
    boxtree::{Albedo, V3cf32, VoxelData},
    raytracing::bevy::{
        data::{handle_gpu_readback, sync_from_main_world, write_to_gpu},
        pipeline::prepare_bind_groups,
        types::{VhxLabel, VhxRenderNode, VhxRenderPipeline},
    },
};
use bendy::{decoding::FromBencode, encoding::ToBencode};
use bevy::{
    app::{App, Plugin},
    asset::LoadState,
    ecs::prelude::IntoScheduleConfigs,
    prelude::{AssetServer, Assets, ExtractSchedule, Handle, Image, Res, ResMut, Update, Vec4},
    render::{
        extract_resource::ExtractResourcePlugin,
        render_asset::RenderAssetUsages,
        render_graph::RenderGraph,
        render_resource::{Extent3d, TextureDimension, TextureFormat, TextureUsages},
        Render, RenderApp, RenderSet,
    },
};
use std::{
    hash::Hash,
    sync::{RwLockReadGuard, RwLockWriteGuard, TryLockResult},
};

impl From<Vec4> for Albedo {
    fn from(vec: Vec4) -> Self {
        Albedo::default()
            .with_red((vec.x * 255.).min(255.) as u8)
            .with_green((vec.y * 255.).min(255.) as u8)
            .with_blue((vec.z * 255.).min(255.) as u8)
            .with_alpha((vec.w * 255.).min(255.) as u8)
    }
}

impl From<Albedo> for Vec4 {
    fn from(color: Albedo) -> Self {
        Vec4::new(
            color.r as f32 / 255.,
            color.g as f32 / 255.,
            color.b as f32 / 255.,
            color.a as f32 / 255.,
        )
    }
}

impl VhxViewSet {
    pub fn new() -> Self {
        Self {
            changed: true,
            views: vec![],
            resources: vec![],
        }
    }

    /// Returns the number of views
    pub fn len(&self) -> usize {
        debug_assert_eq!(
            self.views.len(),
            self.resources.len(),
            "Expected views and their resources to match in count"
        );
        self.views.len()
    }

    /// Provides a view for immutable access; Blocks until view is available
    pub fn view(&self, index: usize) -> RwLockReadGuard<'_, BoxTreeGPUView> {
        self.views[index]
            .read()
            .expect("Expected to be able to lock data view")
    }

    /// Tries to provide a view for immutable access; Fails if view is not available
    pub fn try_view(&self, index: usize) -> TryLockResult<RwLockReadGuard<'_, BoxTreeGPUView>> {
        self.views[index].try_read()
    }

    /// Provides a view for mutable access; Blocks until view is available
    pub fn view_mut(&mut self, index: usize) -> RwLockWriteGuard<'_, BoxTreeGPUView> {
        self.views[index]
            .write()
            .expect("Expected to be able to lock data view")
    }

    /// Tries to provide a view for mutable access; Fails if view is not available
    pub fn try_view_mut(
        &mut self,
        index: usize,
    ) -> TryLockResult<RwLockWriteGuard<'_, BoxTreeGPUView>> {
        self.views[index].try_write()
    }

    /// Empties the viewset erasing all contained views
    pub fn clear(&mut self) {
        self.views.clear();
        self.resources.clear();
        self.changed = true;
    }
}

impl BoxTreeGPUView {
    /// Erases the whole view to be uploaded to the GPU again
    pub fn reload(&mut self) {
        self.reload = true;
    }

    /// Provides the handle to the output texture
    /// Warning! Handle will no longer being updated after resolution change
    pub fn output_texture(&self) -> &Handle<Image> {
        &self.spyglass.output_texture
    }

    /// Updates the resolution on which the view operates on.
    /// It will make a new output texture if size is larger, than the current output texture
    pub fn set_resolution(
        &mut self,
        resolution: [u32; 2],
        images: &mut ResMut<Assets<Image>>,
    ) -> Handle<Image> {
        if self.resolution != resolution {
            self.new_resolution = Some(resolution);
            self.new_output_texture = Some(create_output_texture(resolution, images));
            self.new_depth_texture = Some(create_depth_texture(resolution, images));
            self.rebuild = true;
            self.new_output_texture.as_ref().unwrap().clone_weak()
        } else {
            self.spyglass.output_texture.clone_weak()
        }
    }

    /// Provides currently used resolution for the view
    pub fn resolution(&self) -> [u32; 2] {
        self.resolution
    }
}

impl BoxTreeSpyGlass {
    pub fn viewport(&self) -> &Viewport {
        &self.viewport
    }
    pub fn view_frustum(&self) -> &V3cf32 {
        &self.viewport.frustum
    }
    pub fn view_fov(&self) -> f32 {
        self.viewport.fov
    }
    pub fn viewport_mut(&mut self) -> &mut Viewport {
        self.viewport_changed = true;
        &mut self.viewport
    }
}

impl Viewport {
    pub fn new(origin: V3cf32, direction: V3cf32, frustum: V3cf32, fov: f32) -> Self {
        Self {
            origin,
            direction,
            frustum,
            fov,
        }
    }
}

impl<T> Default for RenderBevyPlugin<T>
where
    T: Default + Clone + Eq + VoxelData + Send + Sync + 'static,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<T> RenderBevyPlugin<T>
where
    T: Default + Clone + Eq + VoxelData + Send + Sync + 'static,
{
    pub fn new() -> Self {
        RenderBevyPlugin {
            dummy: std::marker::PhantomData,
        }
    }
}

pub(crate) fn create_output_texture(
    resolution: [u32; 2],
    images: &mut ResMut<Assets<Image>>,
) -> Handle<Image> {
    let mut output_texture = Image::new_fill(
        Extent3d {
            width: resolution[0],
            height: resolution[1],
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &[0, 0, 0, 255],
        TextureFormat::Rgba8Unorm,
        RenderAssetUsages::RENDER_WORLD,
    );
    output_texture.texture_descriptor.usage =
        TextureUsages::COPY_DST | TextureUsages::STORAGE_BINDING | TextureUsages::TEXTURE_BINDING;

    images.add(output_texture)
}

/// Create a depth texture for the given output resolutions
/// Depth texture resolution should cover a single voxel
pub(crate) fn create_depth_texture(
    resolution: [u32; 2],
    images: &mut ResMut<Assets<Image>>,
) -> Handle<Image> {
    let mut depth_texture = Image::new_fill(
        Extent3d {
            width: resolution[0] / 2,
            height: resolution[1] / 2,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &[0, 0, 0, 255],
        TextureFormat::R32Float,
        RenderAssetUsages::RENDER_WORLD,
    );
    depth_texture.texture_descriptor.usage =
        TextureUsages::COPY_DST | TextureUsages::STORAGE_BINDING | TextureUsages::TEXTURE_BINDING;

    images.add(depth_texture)
}

pub(crate) fn handle_resolution_updates(
    mut viewset: Option<ResMut<VhxViewSet>>,
    images: ResMut<Assets<Image>>,
    server: Res<AssetServer>,
) {
    if let Some(viewset) = viewset.as_mut() {
        if 0 == viewset.views.len() {
            return; // Nothing to do without views..
        }
        let mut current_view = viewset.views[0].write().unwrap();
        if current_view.new_resolution.is_some() {
            // see if a new output texture is loaded for the requested resolution yet
            let new_out_tex = current_view
                .new_output_texture
                .as_ref()
                .unwrap()
                .clone_weak();
            if images.get(&new_out_tex).is_some()
                || matches!(server.get_load_state(&new_out_tex), Some(LoadState::Loaded))
            {
                current_view.resolution = current_view.new_resolution.take().unwrap();
                current_view.spyglass.output_texture =
                    current_view.new_output_texture.clone().unwrap();
            }
        }

        if current_view.new_depth_texture.is_some() {
            let new_depth_tex = current_view
                .new_depth_texture
                .as_ref()
                .unwrap()
                .clone_weak();
            if images.get(&new_depth_tex).is_some()
                || matches!(
                    server.get_load_state(&new_depth_tex),
                    Some(LoadState::Loaded)
                )
            {
                current_view.spyglass.depth_texture =
                    current_view.new_depth_texture.clone().unwrap();
            }
        }
    }
}

impl<
        #[cfg(all(feature = "bytecode", feature = "serialization"))] T: FromBencode
            + ToBencode
            + Serialize
            + DeserializeOwned
            + Default
            + Eq
            + Clone
            + Hash
            + VoxelData
            + Send
            + Sync
            + 'static,
        #[cfg(all(feature = "bytecode", not(feature = "serialization")))] T: FromBencode + ToBencode + Default + Eq + Clone + Hash + VoxelData + Send + Sync + 'static,
        #[cfg(all(not(feature = "bytecode"), feature = "serialization"))] T: Serialize
            + DeserializeOwned
            + Default
            + Eq
            + Clone
            + Hash
            + VoxelData
            + Send
            + Sync
            + 'static,
        #[cfg(all(not(feature = "bytecode"), not(feature = "serialization")))] T: Default + Eq + Clone + Hash + VoxelData + Send + Sync + 'static,
    > Plugin for RenderBevyPlugin<T>
{
    fn build(&self, app: &mut App) {
        app.add_plugins((ExtractResourcePlugin::<BoxTreeGPUHost<T>>::default(),));
        app.add_systems(Update, handle_resolution_updates);
        let render_app = app.sub_app_mut(RenderApp);
        render_app.add_systems(ExtractSchedule, sync_from_main_world);
        render_app.add_systems(
            Render,
            (
                write_to_gpu::<T>.in_set(RenderSet::PrepareAssets),
                prepare_bind_groups.in_set(RenderSet::PrepareBindGroups),
                handle_gpu_readback.in_set(RenderSet::Cleanup),
            ),
        );
        let mut render_graph = render_app.world_mut().resource_mut::<RenderGraph>();
        render_graph.add_node(VhxLabel, VhxRenderNode { ready: false });
        render_graph.add_node_edge(VhxLabel, bevy::render::graph::CameraDriverLabel);
    }

    fn finish(&self, app: &mut App) {
        let render_app = app.sub_app_mut(RenderApp);
        render_app.init_resource::<VhxRenderPipeline>();
    }
}
