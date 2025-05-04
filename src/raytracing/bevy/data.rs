use crate::{
    boxtree::{
        types::{BrickData, NodeContent, PaletteIndexValues},
        BoxTree, V3c, VoxelData, BOX_NODE_CHILDREN_COUNT, OOB_SECTANT,
    },
    object_pool::empty_marker,
    raytracing::bevy::{
        create_depth_texture, create_output_texture,
        types::{
            BoxTreeGPUDataHandler, BoxTreeGPUHost, BoxTreeGPUView, BoxTreeMetaData,
            BoxTreeRenderData, BoxTreeSpyGlass, BrickUpdate, VhxRenderPipeline, VhxViewSet,
            VictimPointer, Viewport,
        },
    },
};
use bendy::{decoding::FromBencode, encoding::ToBencode};
use bevy::{
    ecs::system::{Res, ResMut},
    math::Vec4,
    prelude::{Assets, Image},
    render::{
        render_resource::{
            encase::{internal::WriteInto, StorageBuffer, UniformBuffer},
            Buffer, ShaderSize,
        },
        renderer::{RenderDevice, RenderQueue},
    },
};
use bimap::BiHashMap;
use std::{
    collections::{HashMap, HashSet},
    hash::Hash,
    ops::Range,
    sync::{Arc, Mutex},
};

fn boxtree_properties<
    #[cfg(all(feature = "bytecode", feature = "serialization"))] T: FromBencode
        + ToBencode
        + Serialize
        + DeserializeOwned
        + Default
        + Eq
        + Clone
        + Hash
        + VoxelData,
    #[cfg(all(feature = "bytecode", not(feature = "serialization")))] T: Default + Eq + Clone + Hash + VoxelData,
    #[cfg(all(not(feature = "bytecode"), feature = "serialization"))] T: Serialize + DeserializeOwned + Default + Eq + Clone + Hash + VoxelData,
    #[cfg(all(not(feature = "bytecode"), not(feature = "serialization")))] T: Default + Eq + Clone + Hash + VoxelData,
>(
    tree: &BoxTree<T>,
) -> u32 {
    (tree.brick_dim & 0x0000FFFF) | ((tree.mip_map_strategy.is_enabled() as u32) << 16)
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
    > BoxTreeGPUHost<T>
{
    //##############################################################################
    //     ███████      █████████  ███████████ ███████████   ██████████ ██████████
    //   ███░░░░░███   ███░░░░░███░█░░░███░░░█░░███░░░░░███ ░░███░░░░░█░░███░░░░░█
    //  ███     ░░███ ███     ░░░ ░   ░███  ░  ░███    ░███  ░███  █ ░  ░███  █ ░
    // ░███      ░███░███             ░███     ░██████████   ░██████    ░██████
    // ░███      ░███░███             ░███     ░███░░░░░███  ░███░░█    ░███░░█
    // ░░███     ███ ░░███     ███    ░███     ░███    ░███  ░███ ░   █ ░███ ░   █
    //  ░░░███████░   ░░█████████     █████    █████   █████ ██████████ ██████████
    //    ░░░░░░░      ░░░░░░░░░     ░░░░░    ░░░░░   ░░░░░ ░░░░░░░░░░ ░░░░░░░░░░
    //    █████████  ███████████  █████  █████
    //   ███░░░░░███░░███░░░░░███░░███  ░░███
    //  ███     ░░░  ░███    ░███ ░███   ░███
    // ░███          ░██████████  ░███   ░███
    // ░███    █████ ░███░░░░░░   ░███   ░███
    // ░░███  ░░███  ░███         ░███   ░███
    //  ░░█████████  █████        ░░████████
    //   ░░░░░░░░░  ░░░░░          ░░░░░░░░
    //  █████   █████ █████ ██████████ █████   ███   █████
    // ░░███   ░░███ ░░███ ░░███░░░░░█░░███   ░███  ░░███
    //  ░███    ░███  ░███  ░███  █ ░  ░███   ░███   ░███
    //  ░███    ░███  ░███  ░██████    ░███   ░███   ░███
    //  ░░███   ███   ░███  ░███░░█    ░░███  █████  ███
    //   ░░░█████░    ░███  ░███ ░   █  ░░░█████░█████░
    //     ░░███      █████ ██████████    ░░███ ░░███
    //##############################################################################

    /// Creates GPU compatible data renderable on the GPU from an BoxTree
    pub fn create_new_view(
        &mut self,
        vhx_view_set: &mut VhxViewSet,
        nodes_in_view: usize,
        viewport: Viewport,
        resolution: [u32; 2],
        mut images: ResMut<Assets<Image>>,
    ) -> usize {
        let gpu_data_handler = BoxTreeGPUDataHandler {
            render_data: BoxTreeRenderData {
                mips_enabled: self.tree.mip_map_strategy.is_enabled(),
                boxtree_meta: BoxTreeMetaData {
                    boxtree_size: self.tree.boxtree_size,
                    tree_properties: boxtree_properties(&self.tree),
                    ambient_light_color: V3c::new(1., 1., 1.),
                    ambient_light_position: V3c::new(
                        self.tree.boxtree_size as f32,
                        self.tree.boxtree_size as f32,
                        self.tree.boxtree_size as f32,
                    ),
                },
                used_bits: vec![0; nodes_in_view],
                node_metadata: vec![0; (nodes_in_view as f32 / 8.).ceil() as usize],
                node_ocbits: vec![0; nodes_in_view * 2],
                node_children: vec![empty_marker(); nodes_in_view * BOX_NODE_CHILDREN_COUNT],
                node_mips: vec![empty_marker(); nodes_in_view],
                color_palette: vec![Vec4::ZERO; u16::MAX as usize],
            },
            victim_node: VictimPointer::new(nodes_in_view),
            victim_brick: 0,
            node_key_vs_meta_index: BiHashMap::new(),
            brick_ownership: BiHashMap::new(),
            uploaded_color_palette_size: 0,
        };
        let output_texture = create_output_texture(resolution, &mut images);
        vhx_view_set.views.push(Arc::new(Mutex::new(BoxTreeGPUView {
            resolution,
            reload: false,
            rebuild: false,
            init_data_sent: false,
            data_ready: false,
            new_resolution: None,
            new_output_texture: None,
            new_depth_texture: None,
            data_handler: gpu_data_handler,
            spyglass: BoxTreeSpyGlass {
                depth_texture: create_depth_texture(resolution, &mut images),
                output_texture,
                viewport_changed: true,
                node_requests: vec![empty_marker(); 4],
                viewport,
            },
        })));
        vhx_view_set.resources.push(None);

        debug_assert_eq!(vhx_view_set.resources.len(), vhx_view_set.views.len());
        vhx_view_set.views.len() - 1
    }
}

/// Handles data sync between Bevy main(CPU) world and rendering world
pub(crate) fn sync_with_main_world(// tree_view: Option<ResMut<BoxTreeGPUView>>,
    // mut world: ResMut<bevy::render::MainWorld>,
) {
    // This function is unused because ExtractResource plugin is handling the sync
    // However, it is only one way: MainWorld --> RenderWorld
    // Any modification here is overwritten by the plugin if it is active,
    // in order to enable data flow in the opposite direction, extractresource plugin
    // needs to be disabled, and the sync logic (both ways) needs to be implemented here
    // refer to: https://www.reddit.com/r/bevy/comments/1ay50ee/copy_from_render_world_to_main_world/
}

//##############################################################################
//    █████████  ███████████  █████  █████
//   ███░░░░░███░░███░░░░░███░░███  ░░███
//  ███     ░░░  ░███    ░███ ░███   ░███
// ░███          ░██████████  ░███   ░███
// ░███    █████ ░███░░░░░░   ░███   ░███
// ░░███  ░░███  ░███         ░███   ░███
//  ░░█████████  █████        ░░████████
//   ░░░░░░░░░  ░░░░░          ░░░░░░░░
//  ███████████   ██████████   █████████   ██████████
// ░░███░░░░░███ ░░███░░░░░█  ███░░░░░███ ░░███░░░░███
//  ░███    ░███  ░███  █ ░  ░███    ░███  ░███   ░░███
//  ░██████████   ░██████    ░███████████  ░███    ░███
//  ░███░░░░░███  ░███░░█    ░███░░░░░███  ░███    ░███
//  ░███    ░███  ░███ ░   █ ░███    ░███  ░███    ███
//  █████   █████ ██████████ █████   █████ ██████████
// ░░░░░   ░░░░░ ░░░░░░░░░░ ░░░░░   ░░░░░ ░░░░░░░░░░
//##############################################################################
fn read_buffer(
    render_device: &RenderDevice,
    buffer: &Buffer,
    index_range: std::ops::Range<usize>,
    target: &mut Vec<u32>,
) {
    let byte_start = (index_range.start * std::mem::size_of::<u32>()) as u64;
    let byte_end = (index_range.end * std::mem::size_of::<u32>()) as u64;
    let metadata_buffer_slice = buffer.slice(byte_start..byte_end);
    let (s, metadata_recv) = crossbeam::channel::unbounded::<()>();
    metadata_buffer_slice.map_async(
        bevy::render::render_resource::MapMode::Read,
        move |d| match d {
            Ok(_) => s.send(()).expect("Failed to send map update"),
            Err(err) => panic!("Couldn't map buffer!: {err}"),
        },
    );

    render_device
        .poll(bevy::render::render_resource::Maintain::wait())
        .panic_on_timeout();
    metadata_recv
        .recv()
        .expect("Failed to receive the map_async message");
    {
        let buffer_view = metadata_buffer_slice.get_mapped_range();
        *target = buffer_view
            .chunks(std::mem::size_of::<u32>())
            .map(|chunk| u32::from_ne_bytes(chunk.try_into().expect("should be a u32")))
            .collect::<Vec<u32>>();
    }
    buffer.unmap();
}

/// Handles data reads from GPU every loop, mainly data requests and usaage updates.
/// Based on https://docs.rs/bevy/latest/src/gpu_readback/gpu_readback.rs.html
pub(crate) fn handle_gpu_readback(
    render_device: Res<RenderDevice>,
    vhx_view_set: ResMut<VhxViewSet>,
    vhx_pipeline: Option<ResMut<VhxRenderPipeline>>,
) {
    if vhx_pipeline.is_some() {
        let mut view = vhx_view_set.views[0].lock().unwrap();
        let resources = vhx_view_set.resources[0].as_ref();

        if resources.is_some() {
            let resources = resources.unwrap();

            // init sequence: checking if data is written to the GPU yet
            if view.init_data_sent && !view.data_ready {
                let mut received_value = Vec::new();
                read_buffer(
                    &render_device,
                    &resources.readable_used_bits_buffer,
                    0..1,
                    &mut received_value,
                );
                if view.data_handler.render_data.used_bits[0] == received_value[0] {
                    view.data_ready = true;
                }
            }

            // Read node requests
            read_buffer(
                &render_device,
                &resources.readable_node_requests_buffer,
                0..view.spyglass.node_requests.len(),
                &mut view.spyglass.node_requests,
            );

            let any_nodes_requested = {
                let mut is_metadata_required_this_loop = false;
                for node_request in &view.spyglass.node_requests {
                    if *node_request != empty_marker::<u32>() {
                        is_metadata_required_this_loop = true;
                        break;
                    }
                }
                is_metadata_required_this_loop
            };

            if any_nodes_requested && view.data_ready {
                read_buffer(
                    &render_device,
                    &resources.readable_used_bits_buffer,
                    0..view.data_handler.render_data.used_bits.len(),
                    &mut view.data_handler.render_data.used_bits,
                );
            }
        }
    }
}

//##############################################################################
//    █████████  ███████████  █████  █████
//   ███░░░░░███░░███░░░░░███░░███  ░░███
//  ███     ░░░  ░███    ░███ ░███   ░███
// ░███          ░██████████  ░███   ░███
// ░███    █████ ░███░░░░░░   ░███   ░███
// ░░███  ░░███  ░███         ░███   ░███
//  ░░█████████  █████        ░░████████
//   ░░░░░░░░░  ░░░░░          ░░░░░░░░

//  █████   ███   █████ ███████████   █████ ███████████ ██████████
// ░░███   ░███  ░░███ ░░███░░░░░███ ░░███ ░█░░░███░░░█░░███░░░░░█
//  ░███   ░███   ░███  ░███    ░███  ░███ ░   ░███  ░  ░███  █ ░
//  ░███   ░███   ░███  ░██████████   ░███     ░███     ░██████
//  ░░███  █████  ███   ░███░░░░░███  ░███     ░███     ░███░░█
//   ░░░█████░█████░    ░███    ░███  ░███     ░███     ░███ ░   █
//     ░░███ ░░███      █████   █████ █████    █████    ██████████
//      ░░░   ░░░      ░░░░░   ░░░░░ ░░░░░    ░░░░░    ░░░░░░░░░░
//##############################################################################

/// Converts the given array to `&[u8]` on the given range,
/// and schedules it to be written to the given buffer in the GPU
fn write_range_to_buffer<U>(
    array: &[U],
    index_range: Range<usize>,
    buffer: &Buffer,
    render_queue: &RenderQueue,
) where
    U: Send + Sync + 'static + ShaderSize + WriteInto,
{
    if !index_range.is_empty() {
        let element_size = std::mem::size_of_val(&array[0]);
        let byte_offset = (index_range.start * element_size) as u64;
        let slice = array.get(index_range.clone()).unwrap_or_else(|| {
            panic!(
                "{}",
                format!(
                    "Expected range {:?} to be in bounds of {:?}",
                    index_range,
                    array.len(),
                )
                .to_owned()
            )
        });
        unsafe {
            render_queue.write_buffer(buffer, byte_offset, slice.align_to::<u8>().1);
        }
    }
}

/// Extend the given HashMap with a list of brick updates, except avoid overwriting available data with None value
fn extend_brick_updates<'a>(
    brick_updates: &mut HashMap<usize, Option<&'a [PaletteIndexValues]>>,
    addition: Vec<BrickUpdate<'a>>,
) {
    for brick_update in addition {
        brick_updates
            .entry(brick_update.brick_index)
            .and_modify(|current_brick_data| {
                match (*current_brick_data, brick_update.data) {
                    (None, None) | (Some(_), None) => {} // Keep the current brick data if there is already something
                    (Some(_), Some(_)) | (None, Some(_)) => {
                        // Overwrite the current brick request
                        *current_brick_data = brick_update.data
                    }
                }
            })
            .or_insert(brick_update.data);
    }
}

/// Handles Data Streaming to the GPU based on incoming requests from the view(s)
pub(crate) fn write_to_gpu<
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
    #[cfg(all(not(feature = "bytecode"), feature = "serialization"))] T: Serialize + DeserializeOwned + Default + Eq + Clone + Hash + VoxelData + Send + Sync + 'static,
    #[cfg(all(not(feature = "bytecode"), not(feature = "serialization")))] T: Default + Eq + Clone + Hash + VoxelData + Send + Sync + 'static,
>(
    tree_gpu_host: Option<ResMut<BoxTreeGPUHost<T>>>,
    vhx_pipeline: Option<ResMut<VhxRenderPipeline>>,
    vhx_view_set: ResMut<VhxViewSet>,
) {
    if let (Some(pipeline), Some(tree_host)) = (vhx_pipeline, tree_gpu_host) {
        let mut view = vhx_view_set.views[0].lock().unwrap();

        // Initial BoxTree data upload
        if !view.init_data_sent || view.reload {
            if let Some(resources) = &vhx_view_set.resources[0] {
                // write data for root node
                view.data_handler
                    .add_node(&tree_host.tree, BoxTree::<T>::ROOT_NODE_KEY as usize);

                // Set some well recognizable init value
                view.data_handler.render_data.used_bits[0] = 0xBEEF;

                // Write out the initial data package
                write_range_to_buffer(
                    &view.data_handler.render_data.used_bits,
                    0..1,
                    &resources.used_bits_buffer,
                    &pipeline.render_queue,
                );
                write_range_to_buffer(
                    &view.data_handler.render_data.node_metadata,
                    0..1,
                    &resources.node_metadata_buffer,
                    &pipeline.render_queue,
                );
                write_range_to_buffer(
                    &view.data_handler.render_data.node_children,
                    0..BOX_NODE_CHILDREN_COUNT,
                    &resources.node_children_buffer,
                    &pipeline.render_queue,
                );
                write_range_to_buffer(
                    &view.data_handler.render_data.node_mips,
                    0..1,
                    &resources.node_mips_buffer,
                    &pipeline.render_queue,
                );
                write_range_to_buffer(
                    &view.data_handler.render_data.node_ocbits,
                    0..2,
                    &resources.node_ocbits_buffer,
                    &pipeline.render_queue,
                );
                view.init_data_sent = true;
                view.reload = false;
            }
        }
        let resources = if let Some(resources) = &vhx_view_set.resources[0] {
            resources
        } else {
            // No resources available yet, can't write to them
            return;
        };
        let render_queue = &pipeline.render_queue;

        // Data updates for spyglass viewport
        if view.spyglass.viewport_changed {
            let mut buffer = UniformBuffer::new(Vec::<u8>::new());
            buffer.write(&view.spyglass.viewport).unwrap();
            render_queue.write_buffer(&resources.viewport_buffer, 0, &buffer.into_inner());
            view.spyglass.viewport_changed = false;
        }

        // Data updates for BoxTree MIP map feature
        let tree = &tree_host.tree;
        if view.data_handler.render_data.mips_enabled != tree.mip_map_strategy.is_enabled() {
            // Regenerate feature bits
            view.data_handler.render_data.boxtree_meta.tree_properties = boxtree_properties(tree);

            // Write to GPU
            let mut buffer = UniformBuffer::new(Vec::<u8>::new());
            buffer
                .write(&view.data_handler.render_data.boxtree_meta)
                .unwrap();
            pipeline.render_queue.write_buffer(
                &resources.node_metadata_buffer,
                0,
                &buffer.into_inner(),
            );
            view.data_handler.render_data.mips_enabled = tree.mip_map_strategy.is_enabled()
        }

        // Handle node requests, update cache
        let mut node_meta_updated = Range {
            start: view.data_handler.render_data.node_metadata.len(),
            end: 0,
        };
        let mut used_bits_updated = Range {
            start: view.data_handler.render_data.used_bits.len(),
            end: 0,
        };
        let mut ocbits_updated = Range {
            start: view.data_handler.render_data.node_ocbits.len(),
            end: 0,
        };
        let mut node_children_updated = Range {
            start: view.data_handler.render_data.node_children.len(),
            end: 0,
        };
        let mut node_mips_updated = Range {
            start: view.data_handler.render_data.node_mips.len(),
            end: 0,
        };
        let mut node_requests = view.spyglass.node_requests.clone();
        let mut modified_nodes = HashSet::<usize>::new();
        let mut modified_bricks = HashMap::<usize, Option<&[PaletteIndexValues]>>::new();
        let victim_node_loop_count = view.data_handler.victim_node.get_loop_count();
        for node_request in &mut node_requests {
            if *node_request == empty_marker::<u32>() {
                continue;
            }
            let requested_parent_meta_index = (*node_request & 0x00FFFFFF) as usize;
            let requested_child_sectant = ((*node_request & 0xFF000000) >> 24) as u8;

            debug_assert!(view
                .data_handler
                .node_key_vs_meta_index
                .contains_right(&requested_parent_meta_index));

            let requested_parent_node_key = *view
                .data_handler
                .node_key_vs_meta_index
                .get_by_right(&requested_parent_meta_index)
                .unwrap();

            debug_assert!(
                tree.nodes.key_is_valid(requested_parent_node_key),
                "Expected parent node({:?}) to be valid in GPU request.",
                requested_parent_node_key
            );

            if modified_nodes.contains(&requested_parent_meta_index)
                || !view
                    .data_handler
                    .node_key_vs_meta_index
                    .contains_left(&requested_parent_node_key)
            {
                // Do not accept a request if the requester meta is already overwritten or deleted
                continue;
            }

            // In case MIP is requested, not node child
            if OOB_SECTANT == requested_child_sectant {
                // Upload MIP to bricks
                let (child_descriptor, cache_update) =
                    view.data_handler
                        .add_brick(tree, requested_parent_node_key, OOB_SECTANT);

                used_bits_updated.start = used_bits_updated
                    .start
                    .min(cache_update.modified_usage_range.start);
                used_bits_updated.end = used_bits_updated
                    .end
                    .max(cache_update.modified_usage_range.end);

                modified_nodes.extend(cache_update.modified_nodes);
                extend_brick_updates(&mut modified_bricks, cache_update.brick_updates);

                // Update mip index
                view.data_handler.render_data.node_mips[requested_parent_meta_index] =
                    child_descriptor as u32;

                continue;
            }

            // In case node child is requested, not a MIP
            modified_nodes.insert(requested_parent_meta_index);
            ocbits_updated.start = ocbits_updated.start.min(requested_parent_meta_index * 2);
            ocbits_updated.end = ocbits_updated.end.max(requested_parent_meta_index * 2 + 2);
            let requester_first_child_index = requested_parent_meta_index * BOX_NODE_CHILDREN_COUNT;
            let requester_child_offset =
                requester_first_child_index + requested_child_sectant as usize;
            match tree.nodes.get(requested_parent_node_key) {
                NodeContent::Nothing => {} // parent is empty, nothing to do
                NodeContent::Internal(_) => {
                    let requested_child_node_key = tree_host.tree.node_children
                        [requested_parent_node_key]
                        .child(requested_child_sectant);
                    debug_assert!(
                            tree.nodes.key_is_valid(requested_child_node_key),
                            "Expected key({:?}, child of node[{:?}][{:?}] in meta[{:?}]) to be valid in GPU request.",
                            requested_child_node_key, requested_parent_node_key, requested_child_sectant, requested_parent_meta_index
                        );
                    let child_index = if !view
                        .data_handler
                        .node_key_vs_meta_index
                        .contains_left(&requested_child_node_key)
                    {
                        let (child_index, cache_update) =
                            view.data_handler.add_node(tree, requested_child_node_key);

                        used_bits_updated.start = used_bits_updated
                            .start
                            .min(cache_update.modified_usage_range.start);
                        used_bits_updated.end = used_bits_updated
                            .end
                            .max(cache_update.modified_usage_range.end);
                        modified_nodes.extend(cache_update.modified_nodes);
                        extend_brick_updates(&mut modified_bricks, cache_update.brick_updates);

                        child_index
                    } else {
                        *view
                            .data_handler
                            .node_key_vs_meta_index
                            .get_by_left(&requested_child_node_key)
                            .unwrap()
                    };

                    // Update connection to parent
                    view.data_handler.render_data.node_children[requester_child_offset] =
                        child_index as u32;

                    debug_assert!(
                        view.data_handler
                            .node_key_vs_meta_index
                            .contains_right(&requested_parent_meta_index),
                        "Requester parent erased while adding its child node to meta"
                    );

                    ocbits_updated.start = ocbits_updated.start.min(child_index * 2);
                    ocbits_updated.end = ocbits_updated.end.max(child_index * 2 + 2);
                }
                NodeContent::UniformLeaf(brick) => {
                    // Only upload brick if it's a parted, not already available brick
                    if matches!(brick, BrickData::Parted(_))
                        && view.data_handler.render_data.node_children[requester_first_child_index]
                            == empty_marker::<u32>()
                    {
                        let (child_descriptor, cache_update) =
                            view.data_handler
                                .add_brick(tree, requested_parent_node_key, 0);

                        view.data_handler.render_data.node_children[requester_first_child_index] =
                            child_descriptor as u32;

                        used_bits_updated.start = used_bits_updated
                            .start
                            .min(cache_update.modified_usage_range.start);
                        used_bits_updated.end = used_bits_updated
                            .end
                            .max(cache_update.modified_usage_range.end);
                        modified_nodes.extend(cache_update.modified_nodes);
                        extend_brick_updates(&mut modified_bricks, cache_update.brick_updates);
                    }
                }
                NodeContent::Leaf(bricks) => {
                    // Only upload brick if it's a parted, not already available brick
                    if matches!(
                        bricks[requested_child_sectant as usize],
                        BrickData::Parted(_)
                    ) && view.data_handler.render_data.node_children[requester_child_offset]
                        == empty_marker::<u32>()
                    {
                        let (child_descriptor, cache_update) = view.data_handler.add_brick(
                            tree,
                            requested_parent_node_key,
                            requested_child_sectant,
                        );

                        view.data_handler.render_data.node_children[requester_child_offset] =
                            child_descriptor as u32;

                        used_bits_updated.start = used_bits_updated
                            .start
                            .min(cache_update.modified_usage_range.start);
                        used_bits_updated.end = used_bits_updated
                            .end
                            .max(cache_update.modified_usage_range.end);
                        modified_nodes.extend(cache_update.modified_nodes);
                        extend_brick_updates(&mut modified_bricks, cache_update.brick_updates);
                    }
                }
            }

            if victim_node_loop_count != view.data_handler.victim_node.get_loop_count() {
                break;
            }
        }

        debug_assert!(
            // Either all node requests are empty
            node_requests
                    .iter()
                    .filter(|&v| *v == empty_marker::<u32>())
                    .count()
                    == node_requests.len()
                    // Or some nodes were updated this loop
                    || !modified_nodes.is_empty()
                    // Or the distance traveled by the victim pointer this loop is small enough
                    || (view.data_handler.victim_node.len() as f32 * 0.5) as usize
                        > (victim_node_loop_count - view.data_handler.victim_node.get_loop_count()),
            "Couldn't process a single request because size of the buffer is too small."
        );

        for node_request in &mut node_requests {
            *node_request = empty_marker();
        }

        // Set updated buffers range based on modified nodes and bricks
        for modified_node_index in &modified_nodes {
            node_meta_updated.start = node_meta_updated.start.min(*modified_node_index / 8);
            node_meta_updated.end = node_meta_updated.end.max(modified_node_index / 8 + 1);
            node_children_updated.start = node_children_updated
                .start
                .min(modified_node_index * BOX_NODE_CHILDREN_COUNT);
            node_children_updated.end = node_children_updated
                .end
                .max(modified_node_index * BOX_NODE_CHILDREN_COUNT + BOX_NODE_CHILDREN_COUNT);
            node_mips_updated.start = node_mips_updated.start.min(*modified_node_index);
            node_mips_updated.end = node_mips_updated.end.max(modified_node_index + 1);
        }

        // write back updated data
        let host_color_count = tree.map_to_color_index_in_palette.keys().len();
        let color_palette_size_diff =
            host_color_count - view.data_handler.uploaded_color_palette_size;
        let resources = &vhx_view_set.resources[0].as_ref().unwrap();

        debug_assert!(
            host_color_count >= view.data_handler.uploaded_color_palette_size,
            "Expected host color palette({:?}), to be larger, than colors stored on the GPU({:?})",
            host_color_count,
            view.data_handler.uploaded_color_palette_size
        );

        // Color palette
        if 0 < color_palette_size_diff {
            for i in view.data_handler.uploaded_color_palette_size..host_color_count {
                view.data_handler.render_data.color_palette[i] = tree.voxel_color_palette[i].into();
            }

            // Upload color palette delta to GPU
            write_range_to_buffer(
                &view.data_handler.render_data.color_palette,
                (host_color_count - color_palette_size_diff)..(host_color_count),
                &resources.color_palette_buffer,
                render_queue,
            );
        }
        view.data_handler.uploaded_color_palette_size =
            tree.map_to_color_index_in_palette.keys().len();

        // Render data
        write_range_to_buffer(
            &view.data_handler.render_data.used_bits,
            used_bits_updated,
            &resources.used_bits_buffer,
            render_queue,
        );
        write_range_to_buffer(
            &view.data_handler.render_data.node_metadata,
            node_meta_updated,
            &resources.node_metadata_buffer,
            render_queue,
        );
        write_range_to_buffer(
            &view.data_handler.render_data.node_children,
            node_children_updated,
            &resources.node_children_buffer,
            render_queue,
        );
        write_range_to_buffer(
            &view.data_handler.render_data.node_ocbits,
            ocbits_updated,
            &resources.node_ocbits_buffer,
            render_queue,
        );
        write_range_to_buffer(
            &view.data_handler.render_data.node_mips,
            node_mips_updated,
            &resources.node_mips_buffer,
            render_queue,
        );

        // Upload Voxel data
        for modified_brick_data in &modified_bricks {
            if let Some(new_brick_data) = modified_brick_data.1 {
                let brick_start_index = *modified_brick_data.0 * new_brick_data.len();
                debug_assert_eq!(
                    new_brick_data.len(),
                    tree.brick_dim.pow(3) as usize,
                    "Expected Brick slice to align to tree brick dimension"
                );
                unsafe {
                    render_queue.write_buffer(
                        &resources.voxels_buffer,
                        (brick_start_index * std::mem::size_of_val(&new_brick_data[0])) as u64,
                        new_brick_data.align_to::<u8>().1,
                    );
                }
            }
        }

        // Node requests
        let mut buffer = StorageBuffer::new(Vec::<u8>::new());
        buffer.write(&node_requests).unwrap();
        render_queue.write_buffer(&resources.node_requests_buffer, 0, &buffer.into_inner());
    }
}
