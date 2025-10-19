mod bind_groups;

use crate::raytracing::bevy::{
    pipeline::bind_groups::{
        create_bind_group_layouts, create_spyglass_bind_group, create_stage_bind_groups,
        create_tree_bind_group,
    },
    types::{
        BoxTreeGPUView, BoxTreeRenderDataResources, VhxRenderNode, VhxRenderPipeline, VhxViewSet,
    },
};
use bevy::{
    asset::{AssetLoadError, AssetServer},
    ecs::{
        system::{Res, ResMut},
        world::{FromWorld, World},
    },
    prelude::Shader,
    render::{
        render_asset::RenderAssets,
        render_graph::{self},
        render_resource::{
            encase::{StorageBuffer, UniformBuffer},
            CachedPipelineState, CommandEncoderDescriptor, ComputePassDescriptor,
            ComputePipelineDescriptor, PipelineCache,
        },
        renderer::{RenderContext, RenderDevice, RenderQueue},
        texture::GpuImage,
    },
};
use std::borrow::Cow;

impl FromWorld for VhxRenderPipeline {
    fn from_world(world: &mut World) -> Self {
        let render_device = world.resource::<RenderDevice>();
        let (
            render_stage_bind_group_layout,
            spyglass_bind_group_layout,
            render_data_bind_group_layout,
        ) = create_bind_group_layouts(render_device);
        let shader = world.resource::<AssetServer>().add_async(async move {
            Ok::<Shader, AssetLoadError>(Shader::from_wgsl(
                include_str!("../viewport_render.wgsl"),
                file!(),
            ))
        });
        let pipeline_cache = world.resource::<PipelineCache>();
        let update_pipeline = pipeline_cache.queue_compute_pipeline(ComputePipelineDescriptor {
            zero_initialize_workgroup_memory: false,
            label: None,
            layout: vec![
                render_stage_bind_group_layout.clone(),
                spyglass_bind_group_layout.clone(),
                render_data_bind_group_layout.clone(),
            ],
            push_constant_ranges: Vec::new(),
            shader,
            shader_defs: vec![],
            entry_point: Some(Cow::from("update")),
        });

        VhxRenderPipeline {
            render_queue: world.resource::<RenderQueue>().clone(),
            render_stage_bind_group_layout,
            spyglass_bind_group_layout,
            render_data_bind_group_layout,
            update_pipeline,
        }
    }
}

//##############################################################################
//  ███████████   █████  █████ ██████   █████
// ░░███░░░░░███ ░░███  ░░███ ░░██████ ░░███
//  ░███    ░███  ░███   ░███  ░███░███ ░███
//  ░██████████   ░███   ░███  ░███░░███░███
//  ░███░░░░░███  ░███   ░███  ░███ ░░██████
//  ░███    ░███  ░███   ░███  ░███  ░░█████
//  █████   █████ ░░████████   █████  ░░█████
// ░░░░░   ░░░░░   ░░░░░░░░   ░░░░░    ░░░░░
//##############################################################################
const WORKGROUP_SIZE: u32 = 8;
impl render_graph::Node for VhxRenderNode {
    fn update(&mut self, world: &mut World) {
        let vhx_pipeline = world.resource::<VhxRenderPipeline>();
        let pipeline_cache = world.resource::<PipelineCache>();
        if !self.ready {
            if let CachedPipelineState::Ok(_) =
                pipeline_cache.get_compute_pipeline_state(vhx_pipeline.update_pipeline)
            {
                self.ready = !world.resource::<VhxViewSet>().views.is_empty();
            }
        }
    }

    fn run(
        &self,
        _graph: &mut render_graph::RenderGraphContext,
        render_context: &mut RenderContext,
        world: &World,
    ) -> Result<(), render_graph::NodeRunError> {
        if let (Some(vhx_pipeline), Some(viewset)) = (
            world.get_resource::<VhxRenderPipeline>().as_mut(),
            world.get_resource::<VhxViewSet>().as_mut(),
        ) {
            if viewset.is_empty() {
                return Ok(()); // Nothing to do without views..
            }
            let view = viewset.view(0).unwrap();

            if self.ready && view.resources.is_some() {
                let resources = view.resources.as_ref().unwrap();
                let pipeline_cache = world.resource::<PipelineCache>();
                let command_encoder = render_context.command_encoder();

                {
                    let mut prepass =
                        command_encoder.begin_compute_pass(&ComputePassDescriptor::default());

                    prepass.set_bind_group(0, &resources.render_stage_prepass_bind_group, &[]);
                    prepass.set_bind_group(1, &resources.spyglass_bind_group, &[]);
                    prepass.set_bind_group(2, &resources.tree_bind_group, &[]);
                    let pipeline = pipeline_cache
                        .get_compute_pipeline(vhx_pipeline.update_pipeline)
                        .unwrap();
                    prepass.set_pipeline(pipeline);
                    prepass.dispatch_workgroups(
                        (view.resolution[0] / 2) / WORKGROUP_SIZE,
                        (view.resolution[1] / 2) / WORKGROUP_SIZE,
                        1,
                    );
                }

                {
                    let mut main_pass =
                        command_encoder.begin_compute_pass(&ComputePassDescriptor::default());

                    main_pass.set_bind_group(0, &resources.render_stage_main_bind_group, &[]);
                    main_pass.set_bind_group(1, &resources.spyglass_bind_group, &[]);
                    main_pass.set_bind_group(2, &resources.tree_bind_group, &[]);
                    let pipeline = pipeline_cache
                        .get_compute_pipeline(vhx_pipeline.update_pipeline)
                        .unwrap();
                    main_pass.set_pipeline(pipeline);
                    main_pass.dispatch_workgroups(
                        view.resolution[0] / WORKGROUP_SIZE,
                        view.resolution[1] / WORKGROUP_SIZE,
                        1,
                    );
                }
            }
        }
        Ok(())
    }
}

//##############################################################################
//    █████████  ███████████   ██████████   █████████   ███████████ ██████████
//   ███░░░░░███░░███░░░░░███ ░░███░░░░░█  ███░░░░░███ ░█░░░███░░░█░░███░░░░░█
//  ███     ░░░  ░███    ░███  ░███  █ ░  ░███    ░███ ░   ░███  ░  ░███  █ ░
// ░███          ░██████████   ░██████    ░███████████     ░███     ░██████
// ░███          ░███░░░░░███  ░███░░█    ░███░░░░░███     ░███     ░███░░█
// ░░███     ███ ░███    ░███  ░███ ░   █ ░███    ░███     ░███     ░███ ░   █
//  ░░█████████  █████   █████ ██████████ █████   █████    █████    ██████████
//   ░░░░░░░░░  ░░░░░   ░░░░░ ░░░░░░░░░░ ░░░░░   ░░░░░    ░░░░░    ░░░░░░░░░░
//  █████   █████ █████ ██████████ █████   ███   █████    ███████████   ██████████  █████████
// ░░███   ░░███ ░░███ ░░███░░░░░█░░███   ░███  ░░███    ░░███░░░░░███ ░░███░░░░░█ ███░░░░░███
//  ░███    ░███  ░███  ░███  █ ░  ░███   ░███   ░███     ░███    ░███  ░███  █ ░ ░███    ░░░
//  ░███    ░███  ░███  ░██████    ░███   ░███   ░███     ░██████████   ░██████   ░░█████████
//  ░░███   ███   ░███  ░███░░█    ░░███  █████  ███      ░███░░░░░███  ░███░░█    ░░░░░░░░███
//   ░░░█████░    ░███  ░███ ░   █  ░░░█████░█████░       ░███    ░███  ░███ ░   █ ███    ░███
//     ░░███      █████ ██████████    ░░███ ░░███         █████   █████ ██████████░░█████████
//      ░░░      ░░░░░ ░░░░░░░░░░      ░░░   ░░░         ░░░░░   ░░░░░ ░░░░░░░░░░  ░░░░░░░░░
//##############################################################################
/// Creates the resource collector for the given view
fn create_view_resources(
    pipeline: &mut VhxRenderPipeline,
    render_device: Res<RenderDevice>,
    gpu_images: Res<RenderAssets<GpuImage>>,
    tree_view: &BoxTreeGPUView,
) -> BoxTreeRenderDataResources {
    let (spyglass_bind_group, viewport_buffer) =
        create_spyglass_bind_group(pipeline, &render_device, tree_view);

    let (render_stage_prepass_bind_group, render_stage_main_bind_group) =
        create_stage_bind_groups(&gpu_images, pipeline, &render_device, tree_view);

    let (
        tree_bind_group,
        boxtree_meta_buffer,
        node_metadata_buffer,
        node_children_buffer,
        node_mips_buffer,
        node_ocbits_buffer,
        voxels_buffer,
        color_palette_buffer,
    ) = create_tree_bind_group(pipeline, render_device, tree_view);

    BoxTreeRenderDataResources {
        render_stage_prepass_bind_group,
        render_stage_main_bind_group,
        spyglass_bind_group,
        viewport_buffer,
        tree_bind_group,
        boxtree_meta_buffer,
        node_metadata_buffer,
        node_children_buffer,
        node_mips_buffer,
        node_ocbits_buffer,
        voxels_buffer,
        color_palette_buffer,
    }
}

//##############################################################################
//  ███████████  ███████████   ██████████ ███████████    █████████   ███████████   ██████████
// ░░███░░░░░███░░███░░░░░███ ░░███░░░░░█░░███░░░░░███  ███░░░░░███ ░░███░░░░░███ ░░███░░░░░█
//  ░███    ░███ ░███    ░███  ░███  █ ░  ░███    ░███ ░███    ░███  ░███    ░███  ░███  █ ░
//  ░██████████  ░██████████   ░██████    ░██████████  ░███████████  ░██████████   ░██████
//  ░███░░░░░░   ░███░░░░░███  ░███░░█    ░███░░░░░░   ░███░░░░░███  ░███░░░░░███  ░███░░█
//  ░███         ░███    ░███  ░███ ░   █ ░███         ░███    ░███  ░███    ░███  ░███ ░   █
//  █████        █████   █████ ██████████ █████        █████   █████ █████   █████ ██████████
// ░░░░░        ░░░░░   ░░░░░ ░░░░░░░░░░ ░░░░░        ░░░░░   ░░░░░ ░░░░░   ░░░░░ ░░░░░░░░░░
//  ███████████  █████ ██████   █████ ██████████
// ░░███░░░░░███░░███ ░░██████ ░░███ ░░███░░░░███
//  ░███    ░███ ░███  ░███░███ ░███  ░███   ░░███
//  ░██████████  ░███  ░███░░███░███  ░███    ░███
//  ░███░░░░░███ ░███  ░███ ░░██████  ░███    ░███
//  ░███    ░███ ░███  ░███  ░░█████  ░███    ███
//  ███████████  █████ █████  ░░█████ ██████████
// ░░░░░░░░░░░  ░░░░░ ░░░░░    ░░░░░ ░░░░░░░░░░
//    █████████  ███████████      ███████    █████  █████ ███████████   █████████
//   ███░░░░░███░░███░░░░░███   ███░░░░░███ ░░███  ░░███ ░░███░░░░░███ ███░░░░░███
//  ███     ░░░  ░███    ░███  ███     ░░███ ░███   ░███  ░███    ░███░███    ░░░
// ░███          ░██████████  ░███      ░███ ░███   ░███  ░██████████ ░░█████████
// ░███    █████ ░███░░░░░███ ░███      ░███ ░███   ░███  ░███░░░░░░   ░░░░░░░░███
// ░░███  ░░███  ░███    ░███ ░░███     ███  ░███   ░███  ░███         ███    ░███
//  ░░█████████  █████   █████ ░░░███████░   ░░████████   █████       ░░█████████
//   ░░░░░░░░░  ░░░░░   ░░░░░    ░░░░░░░      ░░░░░░░░   ░░░░░         ░░░░░░░░░
//##############################################################################
/// Constructs buffers, bing groups and uploads rendering data at initialization and whenever prompted
pub(crate) fn prepare_bind_groups(
    gpu_images: Res<RenderAssets<GpuImage>>,
    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
    mut pipeline: Option<ResMut<VhxRenderPipeline>>,
    mut viewset: Option<ResMut<VhxViewSet>>,
) {
    if let (Some(pipeline), Some(viewset)) = (pipeline.as_mut(), viewset.as_mut()) {
        let Some(mut view) = viewset.view_mut(0) else {
            return; // Nothing to do without views..
        };

        // Refresh view because of texture updates
        if view.rebuild && view.resources.is_some() && view.new_images_ready && !view.resize {
            let (spyglass_bind_group, viewport_buffer) =
                create_spyglass_bind_group(pipeline, &render_device, &view);

            // Update View resources
            let (render_stage_prepass_bind_group, render_stage_main_bind_group) =
                create_stage_bind_groups(&gpu_images, pipeline, &render_device, &view);

            let view_resources = view.resources.as_mut().unwrap();
            view_resources.render_stage_prepass_bind_group = render_stage_prepass_bind_group;
            view_resources.render_stage_main_bind_group = render_stage_main_bind_group;
            view_resources.spyglass_bind_group = spyglass_bind_group;
            view_resources.viewport_buffer = viewport_buffer;

            // Update view to clear temporary objects
            view.new_output_texture = None;
            view.new_depth_texture = None;
            view.rebuild = false;
            return;
        }

        if !view.new_images_ready || (!view.rebuild && !view.resize && view.resources.is_some()) {
            return; // Can't or won't rebuild the pipeline
        }

        let Some(resources) = view.resources.as_ref() else {
            // build everything from the ground up if no resources are available
            view.resources = Some(create_view_resources(
                pipeline,
                render_device,
                gpu_images,
                &view,
            ));
            view.rebuild = false;
            return;
        };

        // Re-create buffers for the larger data requirements, move the data there
        if view.resize {
            // Re-create resources
            let mut command_encoder =
                render_device.create_command_encoder(&CommandEncoderDescriptor {
                    label: Some("VhxBufferMoverOnResize"),
                });
            let new_resources = create_view_resources(pipeline, render_device, gpu_images, &view);

            // Node data is built up from host again, because:
            // 1. resize request is triggered by a failed allocation
            // 2. a failed allocation also interrupts gpu updates
            //   --> As insufficient space doesn't let node data to be uploaded to GPU buffers
            // 3. because the gpu updates have been interrupted, the data may not be in sync at the point of interruption
            // 4. node data does not count as significant, e.g. an area of 1024^3 can more or less be covered by 50 nodes
            //   --> That's a magnitude of a few kilobytes at worst, so the runtime penalty is acceptable as of this commit
            debug_assert!(
                resources.node_metadata_buffer.size() <= new_resources.node_metadata_buffer.size(),
                "Expected resized metadata buffer size >= than old buffer size"
            );
            debug_assert!(
                resources.node_children_buffer.size() <= new_resources.node_children_buffer.size(),
                "Expected resized node_children_buffer buffer size >= than old buffer size"
            );
            debug_assert!(
                resources.node_mips_buffer.size() <= new_resources.node_mips_buffer.size(),
                "Expected resized node_mips_buffer buffer size >= than old buffer size"
            );
            debug_assert!(
                resources.node_ocbits_buffer.size() <= new_resources.node_ocbits_buffer.size(),
                "Expected resized node_ocbits_buffer buffer size >= than old buffer size"
            );
            debug_assert!(
                resources.voxels_buffer.size() <= new_resources.voxels_buffer.size(),
                "Expected resized voxels_buffer buffer size >= than old buffer size"
            );
            debug_assert!(
                resources.color_palette_buffer.size() <= new_resources.color_palette_buffer.size(),
                "Expected resized voxels_buffer buffer size >= than old buffer size"
            );

            // Copy the voxel and color palette data to their new buffers
            command_encoder.copy_buffer_to_buffer(
                &resources.voxels_buffer,
                0,
                &new_resources.voxels_buffer,
                0,
                resources.voxels_buffer.size(),
            );
            command_encoder.copy_buffer_to_buffer(
                &resources.color_palette_buffer,
                0,
                &new_resources.color_palette_buffer,
                0,
                resources.color_palette_buffer.size(),
            );

            render_queue.submit([command_encoder.finish()]);

            view.resources = Some(new_resources);
            view.resize = false;
            return;
        }

        // Just re-populate buffer data and update available resources
        let render_data = &view.data_handler.render_data;

        let mut buffer = UniformBuffer::new(Vec::<u8>::new());
        buffer.write(&render_data.boxtree_meta).unwrap();
        pipeline
            .render_queue
            .write_buffer(&resources.boxtree_meta_buffer, 0, &buffer.into_inner());

        let mut buffer = StorageBuffer::new(Vec::<u8>::new());
        buffer.write(&render_data.node_metadata).unwrap();
        pipeline.render_queue.write_buffer(
            &resources.node_metadata_buffer,
            0,
            &buffer.into_inner(),
        );

        let mut buffer = StorageBuffer::new(Vec::<u8>::new());
        buffer.write(&render_data.node_children).unwrap();
        pipeline.render_queue.write_buffer(
            &resources.node_children_buffer,
            0,
            &buffer.into_inner(),
        );

        let mut buffer = StorageBuffer::new(Vec::<u8>::new());
        buffer.write(&render_data.node_mips).unwrap();
        pipeline
            .render_queue
            .write_buffer(&resources.node_mips_buffer, 0, &buffer.into_inner());

        let mut buffer = StorageBuffer::new(Vec::<u8>::new());
        buffer.write(&render_data.node_ocbits).unwrap();
        pipeline
            .render_queue
            .write_buffer(&resources.node_ocbits_buffer, 0, &buffer.into_inner());

        let mut buffer = StorageBuffer::new(Vec::<u8>::new());
        buffer.write(&render_data.color_palette).unwrap();
        pipeline.render_queue.write_buffer(
            &resources.color_palette_buffer,
            0,
            &buffer.into_inner(),
        );
        view.rebuild = false;
    }
}
