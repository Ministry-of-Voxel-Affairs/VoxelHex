mod pipeline;
mod streaming;
pub mod types;
mod view;

pub use crate::raytracing::bevy::types::{
    BoxTreeGPUHost, BoxTreeGPUView, BoxTreeSpyGlass, RenderBevyPlugin, VhxViewSet, Viewport,
};
use crate::{
    boxtree::{
        types::{BoxTreeNodeAccessStack, BoxTreeUpdatedSignalParams},
        Albedo, BoxTree, V3c, VoxelData,
    },
    raytracing::bevy::{
        pipeline::prepare_bind_groups,
        streaming::{types::UploadQueueUpdateTask, upload, upload_queue::rebuild},
        types::{VhxLabel, VhxRenderNode, VhxRenderPipeline},
        view::{handle_resolution_updates_main_world, handle_resolution_updates_render_world},
    },
    spatial::Cube,
};
use bevy::{
    app::{App, Plugin},
    ecs::prelude::IntoScheduleConfigs,
    prelude::{Commands, ExtractSchedule, FixedUpdate, Res, ResMut, Vec4},
    render::{
        extract_resource::ExtractResourcePlugin, render_graph::RenderGraph, Render, RenderApp,
        RenderSystems,
    },
    tasks::AsyncComputeTaskPool,
};
use std::{
    collections::VecDeque,
    sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard, TryLockResult},
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

/// Handles data sync between Bevy main(CPU) world and rendering world
/// Logic here should be as lightweight as possible!
pub(crate) fn sync_from_main_world(
    mut commands: Commands,
    mut world: ResMut<bevy::render::MainWorld>,
    render_world_viewset: Option<Res<VhxViewSet>>,
) {
    let Some(mut main_world_viewset) = world.get_resource_mut::<VhxViewSet>() else {
        return; // Nothing to do without a viewset..
    };

    if render_world_viewset.is_none() || main_world_viewset.changed {
        commands.insert_resource(main_world_viewset.clone());
        main_world_viewset.changed = false;
        return;
    }

    if main_world_viewset.is_empty() {
        return; // Nothing else to do without views..
    }

    let Some(render_world_viewset) = render_world_viewset else {
        // This shouldn't happen ?! In case main world already has an available viewset
        // where the view images are updated, there should already be a viewset in the render world
        commands.insert_resource(main_world_viewset.clone());
        return;
    };

    if render_world_viewset.view(0).unwrap().new_images_ready
        && !main_world_viewset.view(0).unwrap().new_images_ready
    {
        main_world_viewset.view_mut(0).unwrap().new_images_ready = true;
    }
}

fn handle_viewport_position_updates<T: VoxelData>(
    mut commands: Commands,
    mut viewset: Option<ResMut<VhxViewSet>>,
    mut tree_gpu_host: Option<Res<BoxTreeGPUHost<T>>>,
    upload_queue_update: Option<Res<UploadQueueUpdateTask>>,
) {
    // let tree_host_clone: Res<BoxTreeGPUHost<T>> = Res::from(tree_gpu_host.clone().unwrap());
    if let (Some(tree_host), Some(viewset)) = (tree_gpu_host.as_mut(), viewset.as_mut()) {
        if viewset.is_empty() {
            return; // Nothing to do without views..
        }
        let Some(mut view) = viewset.view_mut(0) else {
            return;
        };

        // There have been movement lately
        if view.spyglass.viewport.origin_delta != V3c::unit(0.) {
            // Check if the new origin fits into the brick slot
            if !view.brick_slot.contains(&view.spyglass.viewport.origin) {
                view.data_handler.upload_range = Cube {
                    min_position: view.spyglass.viewport.origin
                        - V3c::unit(view.spyglass.viewport.frustum.z / 2.),
                    size: view.spyglass.viewport.frustum.z,
                };

                if upload_queue_update.is_none() {
                    // rebuild upload queue if movement was large enough
                    let thread_pool = AsyncComputeTaskPool::get();
                    let viewport_center = view.spyglass.viewport.origin;
                    let viewing_distance = view.spyglass.viewport.frustum.z;
                    let brick_ownership = view.data_handler.upload_targets.brick_ownership.clone();
                    let tree_arc = tree_host.tree.clone();
                    let nodes_to_see = view.data_handler.upload_targets.nodes_to_see.clone();
                    commands.insert_resource(UploadQueueUpdateTask(thread_pool.spawn(
                        async move {
                            rebuild::<T>(
                                &tree_arc
                                    .read()
                                    .expect("Expected to be able to read tree from GPU host"),
                                viewport_center,
                                viewing_distance,
                                brick_ownership,
                                nodes_to_see,
                            )
                        },
                    )));
                } else {
                    // upload queue update already in progress! store pending viewport request
                    view.data_handler.pending_upload_queue_update = Some((
                        view.spyglass.viewport.origin,
                        view.spyglass.viewport.frustum.z,
                    ));
                }
                view.brick_slot = Cube::brick_slot_for(
                    &view.spyglass.viewport.origin,
                    tree_host
                        .tree
                        .read()
                        .expect("Expected to be able to read tree from GPU host")
                        .brick_dim,
                );
            }

            let resolution = view.resolution;
            view.spyglass.viewport.origin_delta = V3c::unit(0.);
            view.spyglass.viewport.update_matrices(resolution);
        }
    }
}

impl<T: VoxelData> BoxTreeGPUHost<T> {
    pub fn new(mut tree: BoxTree<T>) -> Self {
        let changes_buffer: Arc<RwLock<VecDeque<BoxTreeUpdatedSignalParams>>> = Arc::default();
        let changes_arc = changes_buffer.clone();
        tree.update_triggers.push(Arc::new(
            move |node_stack: BoxTreeNodeAccessStack, updated_sectants: Vec<u8>| {
                changes_arc
                    .write()
                    .expect("Expected to be able to update BoxTree changes buffer")
                    .push_back((node_stack, updated_sectants));
            },
        ));
        BoxTreeGPUHost {
            tree: Arc::new(RwLock::new(tree)),
            changes_buffer,
        }
    }
}

//##############################################################################
//  █████   █████ █████ ██████████ █████   ███   █████  █████████  ██████████ ███████████
// ░░███   ░░███ ░░███ ░░███░░░░░█░░███   ░███  ░░███  ███░░░░░███░░███░░░░░█░█░░░███░░░█
//  ░███    ░███  ░███  ░███  █ ░  ░███   ░███   ░███ ░███    ░░░  ░███  █ ░ ░   ░███  ░
//  ░███    ░███  ░███  ░██████    ░███   ░███   ░███ ░░█████████  ░██████       ░███
//  ░░███   ███   ░███  ░███░░█    ░░███  █████  ███   ░░░░░░░░███ ░███░░█       ░███
//   ░░░█████░    ░███  ░███ ░   █  ░░░█████░█████░    ███    ░███ ░███ ░   █    ░███
//     ░░███      █████ ██████████    ░░███ ░░███     ░░█████████  ██████████    █████
//      ░░░      ░░░░░ ░░░░░░░░░░      ░░░   ░░░       ░░░░░░░░░  ░░░░░░░░░░    ░░░░░
//##############################################################################
impl Default for VhxViewSet {
    fn default() -> Self {
        Self::new()
    }
}

impl VhxViewSet {
    pub fn new() -> Self {
        Self {
            changed: true,
            views: vec![],
        }
    }

    /// Returns the number of views
    pub fn len(&self) -> usize {
        self.views.len()
    }

    /// True if the viewset is empty
    pub fn is_empty(&self) -> bool {
        0 == self.len()
    }

    /// Provides a view for immutable access; Blocks until view is available
    pub fn view(&self, index: usize) -> Option<RwLockReadGuard<'_, BoxTreeGPUView>> {
        if index < self.views.len() {
            Some(
                self.views[index]
                    .read()
                    .expect("Expected to be able to lock data view for read access"),
            )
        } else {
            None
        }
    }

    /// Tries to provide a view for immutable access; Fails if view is not available
    pub fn try_view(
        &self,
        index: usize,
    ) -> Option<TryLockResult<RwLockReadGuard<'_, BoxTreeGPUView>>> {
        if index < self.views.len() {
            Some(self.views[index].try_read())
        } else {
            None
        }
    }

    /// Provides a view for mutable access; Blocks until view is available
    pub fn view_mut(&mut self, index: usize) -> Option<RwLockWriteGuard<'_, BoxTreeGPUView>> {
        if index < self.views.len() {
            Some(
                self.views[index]
                    .write()
                    .expect("Expected to be able to lock data view for write access"),
            )
        } else {
            None
        }
    }

    /// Tries to provide a view for mutable access; Fails if view is not available
    pub fn try_view_mut(
        &mut self,
        index: usize,
    ) -> Option<TryLockResult<RwLockWriteGuard<'_, BoxTreeGPUView>>> {
        if index < self.views.len() {
            Some(self.views[index].try_write())
        } else {
            None
        }
    }

    /// Empties the viewset erasing all contained views
    pub fn clear(&mut self) {
        self.views.clear();
        self.changed = true;
    }
}

//##############################################################################
//  ███████████  █████       █████  █████   █████████  █████ ██████   █████
// ░░███░░░░░███░░███       ░░███  ░░███   ███░░░░░███░░███ ░░██████ ░░███
//  ░███    ░███ ░███        ░███   ░███  ███     ░░░  ░███  ░███░███ ░███
//  ░██████████  ░███        ░███   ░███ ░███          ░███  ░███░░███░███
//  ░███░░░░░░   ░███        ░███   ░███ ░███    █████ ░███  ░███ ░░██████
//  ░███         ░███      █ ░███   ░███ ░░███  ░░███  ░███  ░███  ░░█████
//  █████        ███████████ ░░████████   ░░█████████  █████ █████  ░░█████
// ░░░░░        ░░░░░░░░░░░   ░░░░░░░░     ░░░░░░░░░  ░░░░░ ░░░░░    ░░░░░
//##############################################################################
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

impl<T: VoxelData> Plugin for RenderBevyPlugin<T> {
    fn build(&self, app: &mut App) {
        app.add_plugins((ExtractResourcePlugin::<BoxTreeGPUHost<T>>::default(),));
        app.add_systems(
            FixedUpdate,
            (
                handle_resolution_updates_main_world,
                handle_viewport_position_updates::<T>,
                upload::<T>,
            ),
        );
        let render_app = app.sub_app_mut(RenderApp);
        render_app.add_systems(ExtractSchedule, sync_from_main_world);
        render_app.add_systems(
            Render,
            (
                upload::<T>.in_set(RenderSystems::PrepareAssets),
                prepare_bind_groups.in_set(RenderSystems::PrepareBindGroups),
                handle_resolution_updates_render_world,
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
