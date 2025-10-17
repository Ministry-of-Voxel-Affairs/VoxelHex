use crate::{
    boxtree::{types::BoxTreeUpdatedSignalParams, BoxTree, V3cf32, VoxelData},
    raytracing::bevy::streaming::types::BoxTreeGPUDataHandler,
    spatial::Cube,
};
use bevy::{
    asset::Handle,
    ecs::resource::Resource,
    math::{UVec2, Vec4},
    prelude::Image,
    reflect::TypePath,
    render::{
        extract_resource::ExtractResource,
        render_graph::RenderLabel,
        render_resource::{
            BindGroup, BindGroupLayout, Buffer, CachedComputePipelineId, ShaderType,
        },
        renderer::RenderQueue,
    },
};
use std::{
    collections::VecDeque,
    hash::Hash,
    sync::{Arc, RwLock},
};

/// Properties of the tree stored on the GPU
#[derive(Debug, Clone, ShaderType)]
pub struct BoxTreeMetaData {
    /// Color of the ambient light in the render
    pub sun_color: V3cf32,

    /// Direction of the ambient light in the render
    pub sun_direction: V3cf32,

    /// Size of the boxtree to display
    pub(crate) boxtree_size: u32,

    /// Contains the properties of the BoxTree
    ///  _===================================================================_
    /// | Byte 0-1 | Voxel Brick Dimension                                    |
    /// |=====================================================================|
    /// | Byte 2   | Features                                                 |
    /// |---------------------------------------------------------------------|
    /// |  bit 0   | 1 if MIP maps are enabled                                |
    /// |  bit 1   | unused                                                   |
    /// |  bit 2   | unused                                                   |
    /// |  bit 3   | unused                                                   |
    /// |  bit 4   | unused                                                   |
    /// |  bit 5   | unused                                                   |
    /// |  bit 6   | unused                                                   |
    /// |  bit 7   | unused                                                   |
    /// |=====================================================================|
    /// | Byte 3   | unused                                                   |
    /// `=====================================================================`
    pub(crate) tree_properties: u32,
}

/// The position and viewing distance of the viewer BoxTree rendering is based on
#[derive(Debug, Clone, Copy, ShaderType)]
pub struct Viewport {
    /// The origin of the viewport, think of it as the position the eye
    pub(crate) origin: V3cf32,

    /// Delta position in case the viewport origin is displaced
    pub(crate) origin_delta: V3cf32,

    /// The direction the raycasts are based upon, think of it as wherever the eye looks
    pub direction: V3cf32,

    /// The volume the viewport reaches to
    /// * `x` - looking glass width
    /// * `y` - looking glass height
    /// * `z` - the max depth of the viewport
    pub frustum: V3cf32,

    /// Field of View: how scattered will the rays in the viewport are
    pub fov: f32,

    /// Pre-computed view matrix (camera transform)
    pub view_matrix: bevy::math::Mat4,

    /// Pre-computed projection matrix
    pub projection_matrix: bevy::math::Mat4,

    /// Pre-computed inverse view projection matrix
    pub inverse_view_projection_matrix: bevy::math::Mat4,
}

/// Represents a BoxTree hosted within the library as it is streamed to the GPU
#[derive(Resource, Clone, TypePath, ExtractResource)]
#[type_path = "shocovox::gpu::BoxTreeGPUHost"]
pub struct BoxTreeGPUHost<T = u32>
where
    T: Default + Clone + Eq + VoxelData + Send + Sync + Hash + 'static,
{
    /// The BoxTree hosted within the bevy library
    pub tree: Arc<RwLock<BoxTree<T>>>,

    /// Updates made to the tree are collected in this buffer
    /// Changes made to nodes within the tree will automatically include them into
    pub(crate) changes_buffer: Arc<RwLock<VecDeque<BoxTreeUpdatedSignalParams>>>,
}

/// Container for all the views rendered by the library instance
#[derive(Debug, Resource, Clone, TypePath)]
#[type_path = "shocovox::gpu::VhxViewSet"]
pub struct VhxViewSet {
    /// dirty bit which is being set for large changes
    /// e.g. new output textures need to be generated
    pub(crate) changed: bool,

    /// Thread safe container for the contained views currently rendered
    pub(crate) views: Vec<Arc<RwLock<BoxTreeGPUView>>>,
}

/// The Camera responsible for storing frustum and view related data
#[derive(Debug, Clone)]
pub struct BoxTreeSpyGlass {
    // The texture used to store depth information in the scene
    pub(crate) depth_texture: Handle<Image>,

    /// The currently used output texture
    pub(crate) output_texture: Handle<Image>,

    // Set to true, if the viewport changed
    pub(crate) viewport_changed: bool,

    // The viewport containing display information
    pub(crate) viewport: Viewport,
}

/// A View of an BoxTree
#[derive(Debug, Resource)]
pub struct BoxTreeGPUView {
    /// Buffers, layouts and bind groups for the view
    pub(crate) resources: Option<BoxTreeRenderDataResources>,

    /// The data handler responsible for uploading data to the GPU
    pub data_handler: BoxTreeGPUDataHandler,

    /// The plane for the basis of the raycasts
    pub spyglass: BoxTreeSpyGlass,

    /// Set to true if the view needs to be reloaded
    pub(crate) reload: bool,

    /// Set to true if the buffers in the view need to be resized
    pub(crate) resize: bool,

    /// Set to true if the view needs to be refreshed, e.g. by a resolution change
    pub(crate) rebuild: bool,

    /// Sets to true if new pipeline textures are ready
    pub(crate) new_images_ready: bool,

    /// The currently used resolution the raycasting dimensions are based for the base ray
    pub(crate) resolution: [u32; 2],

    /// The new resolution to be set if any
    pub(crate) new_resolution: Option<[u32; 2]>,

    /// The new depth texture to be used, if any
    pub(crate) new_depth_texture: Option<Handle<Image>>,

    /// The new output texture to be used, if any
    pub(crate) new_output_texture: Option<Handle<Image>>,

    pub(crate) brick_slot: Cube,
}

#[derive(Debug, Clone)]
pub(crate) struct BoxTreeRenderDataResources {
    pub(crate) render_stage_prepass_bind_group: BindGroup,
    pub(crate) render_stage_main_bind_group: BindGroup,

    // Spyglass group
    // --{
    pub(crate) spyglass_bind_group: BindGroup,
    pub(crate) viewport_buffer: Buffer,
    // }--

    // BoxTree render data group
    // --{
    pub(crate) tree_bind_group: BindGroup,
    pub(crate) boxtree_meta_buffer: Buffer,
    pub(crate) node_metadata_buffer: Buffer,
    pub(crate) node_children_buffer: Buffer,
    pub(crate) node_mips_buffer: Buffer,

    /// Buffer of Node occupancy bitmaps. Each node has a 64 bit bitmap,
    /// which is stored in 2 * u32 values. only available in GPU, to eliminate needles redundancy
    pub(crate) node_ocbits_buffer: Buffer,

    /// Buffer of Voxel Bricks. Each brick contains voxel_brick_dim^3 elements.
    /// Each Brick has a corresponding 64 bit occupancy bitmap in the @voxel_maps buffer.
    /// Only available in GPU, to eliminate needles redundancy
    pub(crate) voxels_buffer: Buffer,
    pub(crate) color_palette_buffer: Buffer,
    // }--
}

#[derive(Debug, Clone, TypePath)]
#[type_path = "shocovox::gpu::ShocoVoxRenderData"]
pub(crate) struct BoxTreeRenderData {
    /// CPU only field, helper variable to detect when the sun color or position have changed
    pub(crate) sun_changed: bool,

    /// CPU only field, contains stored MIP feature enabled state
    pub(crate) mips_enabled: bool,

    /// Contains the properties of the BoxTree
    pub(crate) boxtree_meta: BoxTreeMetaData,

    /// Node Property descriptors, 16x2 bits for each Node
    ///  _===============================================================_
    /// | 16*2 bits for 16 nodes                                         |
    /// |================================================================|
    /// | bit 0  | 1 if brick is leaf, 0 if isn't                        |
    /// |----------------------------------------------------------------|
    /// | bit 1  | 1 if brick is uniform, 0 if isn't                     |
    /// `================================================================`
    pub(crate) node_metadata: Vec<u32>,

    /// Composite field: Children information (18 bits per child)
    /// In case of Internal Nodes
    /// -----------------------------------------
    /// Index values for Nodes, 64 value per @SizedNode entry.
    /// Each value points to one of 64 children of the node,
    /// either pointing to a node in metadata, or marked empty
    /// when there are no children in the given sectant
    ///
    /// In case of Leaf Nodes:
    /// -----------------------------------------
    /// Contains 64 bricks pointing to the child of the node for the relevant sectant
    /// according to @node_metadata ( Uniform/Non-uniform ) a node may have 1
    /// or 64 children, in that case only the first index is used.
    /// Structure is as follows(MSB first):
    ///  _===============================================================_
    /// | bit 0-30 | index of where the voxel brick starts               |
    /// |          | inside the @voxels_buffer(when parted)              |
    /// |          | or inside the @color_palette(when solid)            |
    /// |----------------------------------------------------------------|
    /// |   bit 31 | 0 if brick is parted, 1 if solid                    |
    /// `================================================================`
    pub(crate) node_children: Vec<u32>,

    /// Index values for node MIPs stored inside the bricks, each node has one MIP index, or marked empty
    /// Structure is the same as one child in @node_children
    pub(crate) node_mips: Vec<u32>,

    /// Buffer of Node occupancy bitmaps. Each node has a 64 bit bitmap,
    /// which is stored in 2 * u32 values
    pub(crate) node_ocbits: Vec<u32>,

    /// Stores each unique color, it is references in @voxels
    /// and in @children_buffer as well( in case of solid bricks )
    pub(crate) color_palette: Vec<Vec4>,
}

pub struct RenderBevyPlugin<T = u32>
where
    T: Default + Clone + Eq + VoxelData + Send + Sync + 'static,
{
    pub(crate) dummy: std::marker::PhantomData<T>,
}

pub(crate) const VHX_PREPASS_STAGE_ID: u32 = 0x01;
pub(crate) const VHX_RENDER_STAGE_ID: u32 = 0x02;

#[derive(Debug, Clone, Copy, ShaderType)]
pub(crate) struct RenderStageData {
    pub(crate) stage: u32,
    pub(crate) output_resolution: UVec2,
}

#[derive(Resource)]
pub(crate) struct VhxRenderPipeline {
    pub(crate) render_queue: RenderQueue,
    pub(crate) update_pipeline: CachedComputePipelineId,
    pub(crate) render_stage_bind_group_layout: BindGroupLayout,
    pub(crate) spyglass_bind_group_layout: BindGroupLayout,
    pub(crate) render_data_bind_group_layout: BindGroupLayout,
}

#[derive(Debug, Hash, PartialEq, Eq, Clone, RenderLabel)]
pub(crate) struct VhxLabel;

pub(crate) struct VhxRenderNode {
    pub(crate) ready: bool,
}

#[cfg(test)]
mod types_wgpu_byte_compatibility_tests {
    use super::{BoxTreeMetaData, Viewport};
    use bevy::render::render_resource::encase::ShaderType;

    #[test]
    fn test_wgpu_compatibility() {
        Viewport::assert_uniform_compat();
        BoxTreeMetaData::assert_uniform_compat();
    }
}
