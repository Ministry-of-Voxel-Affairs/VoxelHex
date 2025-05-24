use crate::raytracing::bevy::{
    bind_groups::{
        create_bind_group_layouts, create_spyglass_bind_group, create_stage_bind_groups,
        create_tree_bind_group,
    },
    types::{
        BoxTreeGPUView, BoxTreeRenderDataResources, VhxRenderNode, VhxRenderPipeline, VhxViewSet,
    },
};
use bevy::{
    asset::AssetServer,
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
            BufferDescriptor, BufferUsages, CachedPipelineState, ComputePassDescriptor,
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
        let shader = world.resource::<AssetServer>().add(Shader::from_wgsl(
            include_str!("viewport_render.wgsl"),
            file!(),
        ));
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
            entry_point: Cow::from("update"),
        });

        VhxRenderPipeline {
            render_queue: world.resource::<RenderQueue>().clone(),
            update_tree: true,
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
            world.get_resource::<VhxRenderPipeline>(),
            world.get_resource::<VhxViewSet>(),
        ) {
            if 0 == viewset.views.len() {
                return Ok(()); // Nothing to do without views..
            }

            let current_view = viewset.views[0].lock().unwrap();
            let resources = viewset.resources[0].as_ref();

            if self.ready && resources.is_some() {
                let resources = resources.unwrap();
                let pipeline_cache = world.resource::<PipelineCache>();
                let command_encoder = render_context.command_encoder();
                let data_handler = &current_view.data_handler;

                if !current_view.data_ready {
                    // The first byte of metadata is used to monitor if the GPU has init data uploaded.
                    // Until state is set on host, just copy data to the readable buffer.
                    command_encoder.copy_buffer_to_buffer(
                        &resources.node_metadata_buffer,
                        0,
                        &resources.readable_used_bits_buffer,
                        0,
                        (std::mem::size_of_val(&data_handler.render_data.node_metadata[0])) as u64,
                    );
                } else {
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
                            (current_view.resolution[0] / 2) / WORKGROUP_SIZE,
                            (current_view.resolution[1] / 2) / WORKGROUP_SIZE,
                            1,
                        );
                    }

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
                        current_view.resolution[0] / WORKGROUP_SIZE,
                        current_view.resolution[1] / WORKGROUP_SIZE,
                        1,
                    );
                }

                command_encoder.copy_buffer_to_buffer(
                    &resources.used_bits_buffer,
                    0,
                    &resources.readable_used_bits_buffer,
                    0,
                    (std::mem::size_of_val(&data_handler.render_data.used_bits[0])
                        * data_handler.render_data.used_bits.len()) as u64,
                );

                debug_assert!(
                    !current_view.spyglass.node_requests.is_empty(),
                    "Expected node requests array to not be empty"
                );
                command_encoder.copy_buffer_to_buffer(
                    &resources.node_requests_buffer,
                    0,
                    &resources.readable_node_requests_buffer,
                    0,
                    (std::mem::size_of_val(&current_view.spyglass.node_requests[0])
                        * current_view.spyglass.node_requests.len()) as u64,
                );
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
    let render_data = &tree_view.data_handler.render_data;

    // Create the staging buffer helping in reading data from the GPU
    let readable_used_bits_buffer = render_device.create_buffer(&BufferDescriptor {
        mapped_at_creation: false,
        size: (render_data.used_bits.len() * 4) as u64,
        label: Some("BoxTree Node metadata staging Buffer"),
        usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
    });

    let (spyglass_bind_group, viewport_buffer, node_requests_buffer, readable_node_requests_buffer) =
        create_spyglass_bind_group(pipeline, &render_device, tree_view);

    let (render_stage_prepass_bind_group, render_stage_main_bind_group) =
        create_stage_bind_groups(&gpu_images, pipeline, &render_device, tree_view);

    let (
        tree_bind_group,
        boxtree_meta_buffer,
        used_bits_buffer,
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
        node_requests_buffer,
        readable_node_requests_buffer,
        tree_bind_group,
        boxtree_meta_buffer,
        used_bits_buffer,
        node_metadata_buffer,
        node_children_buffer,
        node_mips_buffer,
        node_ocbits_buffer,
        voxels_buffer,
        color_palette_buffer,
        readable_used_bits_buffer,
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
    pipeline: Option<ResMut<VhxRenderPipeline>>,
    viewset: Option<ResMut<VhxViewSet>>,
) {
    if let (Some(mut pipeline), Some(mut viewset)) = (pipeline, viewset) {
        if 0 == viewset.views.len() {
            return; // Nothing to do without views..
        }
        if !viewset.views[0].lock().unwrap().rebuild && !pipeline.update_tree {
            return;
        }

        // Rebuild view for texture updates
        let can_rebuild = {
            let view = viewset.views[0].lock().unwrap();
            view.rebuild
                && view.new_output_texture.is_some()
                && gpu_images
                    .get(view.new_output_texture.as_ref().unwrap())
                    .is_some()
                && view.spyglass.output_texture == *view.new_output_texture.as_ref().unwrap()
                && view.new_depth_texture.is_some()
                && gpu_images
                    .get(view.new_depth_texture.as_ref().unwrap())
                    .is_some()
                && view.spyglass.depth_texture == *view.new_depth_texture.as_ref().unwrap()
        };

        if can_rebuild {
            let (
                spyglass_bind_group,
                viewport_buffer,
                node_requests_buffer,
                readable_node_requests_buffer,
            ) = create_spyglass_bind_group(
                &mut pipeline,
                &render_device,
                &viewset.views[0].lock().unwrap(),
            );

            // Update View resources
            let (render_stage_prepass_bind_group, render_stage_main_bind_group) =
                create_stage_bind_groups(
                    &gpu_images,
                    &mut pipeline,
                    &render_device,
                    &viewset.views[0].lock().unwrap(),
                );

            let view_resources = viewset.resources[0].as_mut().unwrap();
            view_resources.render_stage_prepass_bind_group = render_stage_prepass_bind_group;
            view_resources.render_stage_main_bind_group = render_stage_main_bind_group;
            view_resources.spyglass_bind_group = spyglass_bind_group;
            view_resources.viewport_buffer = viewport_buffer;
            view_resources.node_requests_buffer = node_requests_buffer;
            view_resources.readable_node_requests_buffer = readable_node_requests_buffer;

            // Update view to clear temporary objects
            let mut view = viewset.views[0].lock().unwrap();
            view.new_output_texture = None;
            view.new_depth_texture = None;
            view.rebuild = false;
            return;
        }

        // build everything from the ground up
        if let Some(resources) = &viewset.resources[0] {
            let tree_view = &viewset.views[0].lock().unwrap();
            let render_data = &tree_view.data_handler.render_data;

            let mut buffer = UniformBuffer::new(Vec::<u8>::new());
            buffer.write(&render_data.boxtree_meta).unwrap();
            pipeline.render_queue.write_buffer(
                &resources.boxtree_meta_buffer,
                0,
                &buffer.into_inner(),
            );

            let mut buffer = StorageBuffer::new(Vec::<u8>::new());
            buffer.write(&render_data.used_bits).unwrap();
            pipeline.render_queue.write_buffer(
                &resources.used_bits_buffer,
                0,
                &buffer.into_inner(),
            );

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
            pipeline.render_queue.write_buffer(
                &resources.node_mips_buffer,
                0,
                &buffer.into_inner(),
            );

            let mut buffer = StorageBuffer::new(Vec::<u8>::new());
            buffer.write(&render_data.node_ocbits).unwrap();
            pipeline.render_queue.write_buffer(
                &resources.node_ocbits_buffer,
                0,
                &buffer.into_inner(),
            );

            let mut buffer = StorageBuffer::new(Vec::<u8>::new());
            buffer.write(&render_data.color_palette).unwrap();
            pipeline.render_queue.write_buffer(
                &resources.color_palette_buffer,
                0,
                &buffer.into_inner(),
            )
        } else {
            let view_resources = create_view_resources(
                &mut pipeline,
                render_device,
                gpu_images,
                &viewset.views[0].lock().unwrap(),
            );
            viewset.resources[0] = Some(view_resources);
        }

        pipeline.update_tree = false;
    }
}
