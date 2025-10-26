use crate::{
    boxtree::types::PaletteIndexValues,
    raytracing::bevy::types::{
        BoxTreeGPUView, BoxTreeMetaData, RenderStageData, VhxRenderPipeline, Viewport,
        VHX_PREPASS_STAGE_ID, VHX_RENDER_STAGE_ID,
    },
};
use bevy::{
    ecs::system::Res,
    math::{UVec2, Vec4},
    render::{
        render_asset::RenderAssets,
        render_resource::{
            encase::{StorageBuffer, UniformBuffer},
            BindGroup, BindGroupEntry, BindGroupLayout, BindGroupLayoutEntry, BindingResource,
            BindingType, Buffer, BufferBindingType, BufferDescriptor, BufferInitDescriptor,
            BufferUsages, ShaderSize, ShaderStages, ShaderType, StorageTextureAccess,
            TextureFormat, TextureViewDimension,
        },
        renderer::RenderDevice,
        texture::GpuImage,
    },
};

//##############################################################################
// ███████████  █████ ██████   █████ ██████████
// ░░███░░░░░███░░███ ░░██████ ░░███ ░░███░░░░███
//  ░███    ░███ ░███  ░███░███ ░███  ░███   ░░███
//  ░██████████  ░███  ░███░░███░███  ░███    ░███
//  ░███░░░░░███ ░███  ░███ ░░██████  ░███    ░███
//  ░███    ░███ ░███  ░███  ░░█████  ░███    ███
//  ███████████  █████ █████  ░░█████ ██████████
// ░░░░░░░░░░░  ░░░░░ ░░░░░    ░░░░░ ░░░░░░░░░░
//    █████████  ███████████      ███████    █████  █████ ███████████
//   ███░░░░░███░░███░░░░░███   ███░░░░░███ ░░███  ░░███ ░░███░░░░░███
//  ███     ░░░  ░███    ░███  ███     ░░███ ░███   ░███  ░███    ░███
// ░███          ░██████████  ░███      ░███ ░███   ░███  ░██████████
// ░███    █████ ░███░░░░░███ ░███      ░███ ░███   ░███  ░███░░░░░░
// ░░███  ░░███  ░███    ░███ ░░███     ███  ░███   ░███  ░███
//  ░░█████████  █████   █████ ░░░███████░   ░░████████   █████
//   ░░░░░░░░░  ░░░░░   ░░░░░    ░░░░░░░      ░░░░░░░░   ░░░░░
//  █████         █████████   █████ █████    ███████    █████  █████ ███████████
// ░░███         ███░░░░░███ ░░███ ░░███   ███░░░░░███ ░░███  ░░███ ░█░░░███░░░█
//  ░███        ░███    ░███  ░░███ ███   ███     ░░███ ░███   ░███ ░   ░███  ░
//  ░███        ░███████████   ░░█████   ░███      ░███ ░███   ░███     ░███
//  ░███        ░███░░░░░███    ░░███    ░███      ░███ ░███   ░███     ░███
//  ░███      █ ░███    ░███     ░███    ░░███     ███  ░███   ░███     ░███
//  ███████████ █████   █████    █████    ░░░███████░   ░░████████      █████
// ░░░░░░░░░░░ ░░░░░   ░░░░░    ░░░░░       ░░░░░░░      ░░░░░░░░      ░░░░░
//##############################################################################
pub(crate) fn create_bind_group_layouts(
    render_device: &RenderDevice,
) -> (BindGroupLayout, BindGroupLayout, BindGroupLayout) {
    let render_stage_bind_group_layout = render_device.create_bind_group_layout(
        "RenderStageBindGroup",
        &[
            BindGroupLayoutEntry {
                binding: 0u32,
                visibility: ShaderStages::COMPUTE,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: Some(<RenderStageData as ShaderType>::min_size()),
                },
                count: None,
            },
            BindGroupLayoutEntry {
                binding: 1u32,
                visibility: ShaderStages::COMPUTE,
                ty: BindingType::StorageTexture {
                    access: StorageTextureAccess::ReadWrite,
                    format: TextureFormat::Rgba8Unorm,
                    view_dimension: TextureViewDimension::D2,
                },
                count: None,
            },
            BindGroupLayoutEntry {
                binding: 2u32,
                visibility: ShaderStages::COMPUTE,
                ty: BindingType::StorageTexture {
                    access: StorageTextureAccess::ReadWrite,
                    format: TextureFormat::R32Float,
                    view_dimension: TextureViewDimension::D2,
                },
                count: None,
            },
        ],
    );
    let spyglass_bind_group_layout = render_device.create_bind_group_layout(
        "BoxTreeSpyGlass",
        &[BindGroupLayoutEntry {
            binding: 0u32,
            visibility: ShaderStages::COMPUTE,
            ty: BindingType::Buffer {
                ty: BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: Some(<Viewport as ShaderType>::min_size()),
            },
            count: None,
        }],
    );
    let render_data_bind_group_layout = render_device.create_bind_group_layout(
        "BoxTreeRenderData",
        &[
            BindGroupLayoutEntry {
                binding: 0u32,
                visibility: ShaderStages::COMPUTE,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: Some(<BoxTreeMetaData as ShaderType>::min_size()),
                },
                count: None,
            },
            BindGroupLayoutEntry {
                binding: 1u32,
                visibility: ShaderStages::COMPUTE,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: Some(<Vec<u32> as ShaderType>::min_size()),
                },
                count: None,
            },
            BindGroupLayoutEntry {
                binding: 2u32,
                visibility: ShaderStages::COMPUTE,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: Some(<Vec<u32> as ShaderType>::min_size()),
                },
                count: None,
            },
            BindGroupLayoutEntry {
                binding: 3u32,
                visibility: ShaderStages::COMPUTE,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: Some(<Vec<u32> as ShaderType>::min_size()),
                },
                count: None,
            },
            BindGroupLayoutEntry {
                binding: 4u32,
                visibility: ShaderStages::COMPUTE,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: Some(<Vec<u32> as ShaderType>::min_size()),
                },
                count: None,
            },
            BindGroupLayoutEntry {
                binding: 5u32,
                visibility: ShaderStages::COMPUTE,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: Some(<Vec<PaletteIndexValues> as ShaderType>::min_size()),
                },
                count: None,
            },
            BindGroupLayoutEntry {
                binding: 6u32,
                visibility: ShaderStages::COMPUTE,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: Some(<Vec<Vec4> as ShaderType>::min_size()),
                },
                count: None,
            },
            BindGroupLayoutEntry {
                binding: 7u32,
                visibility: ShaderStages::COMPUTE,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Storage { read_only: false },
                    has_dynamic_offset: false,
                    min_binding_size: Some(<Vec<u32> as ShaderType>::min_size()),
                },
                count: None,
            },
        ],
    );
    (
        render_stage_bind_group_layout,
        spyglass_bind_group_layout,
        render_data_bind_group_layout,
    )
}

//##############################################################################
//   █████████  ███████████   █████████     █████████  ██████████
//  ███░░░░░███░█░░░███░░░█  ███░░░░░███   ███░░░░░███░░███░░░░░█
// ░███    ░░░ ░   ░███  ░  ░███    ░███  ███     ░░░  ░███  █ ░
// ░░█████████     ░███     ░███████████ ░███          ░██████
//  ░░░░░░░░███    ░███     ░███░░░░░███ ░███    █████ ░███░░█
//  ███    ░███    ░███     ░███    ░███ ░░███  ░░███  ░███ ░   █
// ░░█████████     █████    █████   █████ ░░█████████  ██████████
//  ░░░░░░░░░     ░░░░░    ░░░░░   ░░░░░   ░░░░░░░░░  ░░░░░░░░░░

//    █████████  ███████████      ███████    █████  █████ ███████████   █████████
//   ███░░░░░███░░███░░░░░███   ███░░░░░███ ░░███  ░░███ ░░███░░░░░███ ███░░░░░███
//  ███     ░░░  ░███    ░███  ███     ░░███ ░███   ░███  ░███    ░███░███    ░░░
// ░███          ░██████████  ░███      ░███ ░███   ░███  ░██████████ ░░█████████
// ░███    █████ ░███░░░░░███ ░███      ░███ ░███   ░███  ░███░░░░░░   ░░░░░░░░███
// ░░███  ░░███  ░███    ░███ ░░███     ███  ░███   ░███  ░███         ███    ░███
//  ░░█████████  █████   █████ ░░░███████░   ░░████████   █████       ░░█████████
//   ░░░░░░░░░  ░░░░░   ░░░░░    ░░░░░░░      ░░░░░░░░   ░░░░░         ░░░░░░░░░
//##############################################################################
pub(crate) fn create_stage_bind_groups(
    gpu_images: &Res<RenderAssets<GpuImage>>,
    pipeline: &mut VhxRenderPipeline,
    render_device: &Res<RenderDevice>,
    tree_view: &BoxTreeGPUView,
) -> (BindGroup, BindGroup) {
    let mut buffer = StorageBuffer::new(Vec::<u8>::new());
    buffer
        .write(&RenderStageData {
            stage: VHX_PREPASS_STAGE_ID,
            output_resolution: UVec2::new(tree_view.resolution[0] / 2, tree_view.resolution[1] / 2),
        })
        .unwrap();
    let prepass_data_buffer = render_device.create_buffer_with_data(&BufferInitDescriptor {
        label: Some("Vhx Prepass stage Buffer"),
        contents: &buffer.into_inner(),
        usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
    });

    let mut buffer = StorageBuffer::new(Vec::<u8>::new());
    buffer
        .write(&RenderStageData {
            stage: VHX_RENDER_STAGE_ID,
            output_resolution: UVec2::new(tree_view.resolution[0], tree_view.resolution[1]),
        })
        .unwrap();
    let render_stage_data_buffer = render_device.create_buffer_with_data(&BufferInitDescriptor {
        label: Some("Vhx Main Render stage Buffer"),
        contents: &buffer.into_inner(),
        usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
    });

    (
        render_device.create_bind_group(
            "Vhx Prepass stage bind group",
            &pipeline.render_stage_bind_group_layout,
            &[
                bevy::render::render_resource::BindGroupEntry {
                    binding: 0,
                    resource: prepass_data_buffer.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: BindingResource::TextureView(
                        &gpu_images
                            .get(&tree_view.spyglass.output_texture)
                            .unwrap()
                            .texture_view,
                    ),
                },
                BindGroupEntry {
                    binding: 2,
                    resource: BindingResource::TextureView(
                        &gpu_images
                            .get(&tree_view.spyglass.depth_texture)
                            .unwrap()
                            .texture_view,
                    ),
                },
            ],
        ),
        render_device.create_bind_group(
            "Vhx Main Render stage main bind group",
            &pipeline.render_stage_bind_group_layout,
            &[
                bevy::render::render_resource::BindGroupEntry {
                    binding: 0,
                    resource: render_stage_data_buffer.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: BindingResource::TextureView(
                        &gpu_images
                            .get(&tree_view.spyglass.output_texture)
                            .unwrap()
                            .texture_view,
                    ),
                },
                BindGroupEntry {
                    binding: 2,
                    resource: BindingResource::TextureView(
                        &gpu_images
                            .get(&tree_view.spyglass.depth_texture)
                            .unwrap()
                            .texture_view,
                    ),
                },
            ],
        ),
    )
}

//##############################################################################
//   █████████  ███████████  █████ █████
//  ███░░░░░███░░███░░░░░███░░███ ░░███
// ░███    ░░░  ░███    ░███ ░░███ ███
// ░░█████████  ░██████████   ░░█████
//  ░░░░░░░░███ ░███░░░░░░     ░░███
//  ███    ░███ ░███            ░███
// ░░█████████  █████           █████
//  ░░░░░░░░░  ░░░░░           ░░░░░
//    █████████  █████         █████████    █████████   █████████
//   ███░░░░░███░░███         ███░░░░░███  ███░░░░░███ ███░░░░░███
//  ███     ░░░  ░███        ░███    ░███ ░███    ░░░ ░███    ░░░
// ░███          ░███        ░███████████ ░░█████████ ░░█████████
// ░███    █████ ░███        ░███░░░░░███  ░░░░░░░░███ ░░░░░░░░███
// ░░███  ░░███  ░███      █ ░███    ░███  ███    ░███ ███    ░███
//  ░░█████████  ███████████ █████   █████░░█████████ ░░█████████
//   ░░░░░░░░░  ░░░░░░░░░░░ ░░░░░   ░░░░░  ░░░░░░░░░   ░░░░░░░░░
//    █████████  ███████████      ███████    █████  █████ ███████████
//   ███░░░░░███░░███░░░░░███   ███░░░░░███ ░░███  ░░███ ░░███░░░░░███
//  ███     ░░░  ░███    ░███  ███     ░░███ ░███   ░███  ░███    ░███
// ░███          ░██████████  ░███      ░███ ░███   ░███  ░██████████
// ░███    █████ ░███░░░░░███ ░███      ░███ ░███   ░███  ░███░░░░░░
// ░░███  ░░███  ░███    ░███ ░░███     ███  ░███   ░███  ░███
//  ░░█████████  █████   █████ ░░░███████░   ░░████████   █████
//##############################################################################
/// Creates the bind groups for render stages
/// returns with a pair: (prepass_bind_group, main_stage_bind_group)
pub(crate) fn create_spyglass_bind_group(
    pipeline: &mut VhxRenderPipeline,
    render_device: &Res<RenderDevice>,
    tree_view: &BoxTreeGPUView,
) -> (BindGroup, Buffer) {
    let mut buffer = UniformBuffer::new([0u8; Viewport::SHADER_SIZE.get() as usize]);
    buffer.write(&tree_view.spyglass.viewport).unwrap();
    let viewport_buffer = render_device.create_buffer_with_data(&BufferInitDescriptor {
        label: Some("BoxTree Viewport Buffer"),
        contents: &buffer.into_inner(),
        usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
    });

    (
        render_device.create_bind_group(
            "OctreeSpyGlass",
            &pipeline.spyglass_bind_group_layout,
            &[BindGroupEntry {
                binding: 0,
                resource: viewport_buffer.as_entire_binding(),
            }],
        ),
        viewport_buffer,
    )
}

//##############################################################################
//  ███████████ ███████████   ██████████ ██████████
// ░█░░░███░░░█░░███░░░░░███ ░░███░░░░░█░░███░░░░░█
// ░   ░███  ░  ░███    ░███  ░███  █ ░  ░███  █ ░
//     ░███     ░██████████   ░██████    ░██████
//     ░███     ░███░░░░░███  ░███░░█    ░███░░█
//     ░███     ░███    ░███  ░███ ░   █ ░███ ░   █
//     █████    █████   █████ ██████████ ██████████
//    ░░░░░    ░░░░░   ░░░░░ ░░░░░░░░░░ ░░░░░░░░░░
//    █████████  ███████████      ███████    █████  █████ ███████████
//   ███░░░░░███░░███░░░░░███   ███░░░░░███ ░░███  ░░███ ░░███░░░░░███
//  ███     ░░░  ░███    ░███  ███     ░░███ ░███   ░███  ░███    ░███
// ░███          ░██████████  ░███      ░███ ░███   ░███  ░██████████
// ░███    █████ ░███░░░░░███ ░███      ░███ ░███   ░███  ░███░░░░░░
// ░░███  ░░███  ░███    ░███ ░░███     ███  ░███   ░███  ░███
//  ░░█████████  █████   █████ ░░░███████░   ░░████████   █████
//   ░░░░░░░░░  ░░░░░   ░░░░░    ░░░░░░░      ░░░░░░░░   ░░░░░
//##############################################################################
pub(crate) fn create_tree_bind_group(
    pipeline: &mut VhxRenderPipeline,
    render_device: Res<RenderDevice>,
    tree_view: &BoxTreeGPUView,
) -> (
    BindGroup,
    Buffer,
    Buffer,
    Buffer,
    Buffer,
    Buffer,
    Buffer,
    Buffer,
    Buffer,
) {
    let render_data = &tree_view.data_handler.render_data;

    let mut buffer = UniformBuffer::new(Vec::<u8>::new());
    buffer.write(&render_data.boxtree_meta).unwrap();
    let boxtree_meta_buffer = render_device.create_buffer_with_data(&BufferInitDescriptor {
        label: Some("BoxTree Tree Metadata Buffer"),
        contents: &buffer.into_inner(),
        usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
    });

    let mut buffer = StorageBuffer::new(Vec::<u8>::new());
    buffer.write(&render_data.node_metadata).unwrap();
    let node_metadata_buffer = render_device.create_buffer_with_data(&BufferInitDescriptor {
        label: Some("BoxTree Node Metadata Buffer"),
        contents: &buffer.into_inner(),
        usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC | BufferUsages::COPY_DST,
    });

    let mut buffer = StorageBuffer::new(Vec::<u8>::new());
    buffer.write(&render_data.node_children).unwrap();
    let node_children_buffer = render_device.create_buffer_with_data(&BufferInitDescriptor {
        label: Some("BoxTree Node Children Buffer"),
        contents: &buffer.into_inner(),
        usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC | BufferUsages::COPY_DST,
    });

    let mut buffer = StorageBuffer::new(Vec::<u8>::new());
    buffer.write(&render_data.node_mips).unwrap();
    let node_mips_buffer = render_device.create_buffer_with_data(&BufferInitDescriptor {
        label: Some("BoxTree Node MIPs Buffer"),
        contents: &buffer.into_inner(),
        usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC | BufferUsages::COPY_DST,
    });

    let mut buffer = StorageBuffer::new(Vec::<u8>::new());
    buffer.write(&render_data.node_ocbits).unwrap();
    let node_ocbits_buffer = render_device.create_buffer_with_data(&BufferInitDescriptor {
        label: Some("BoxTree Node Occupied Bits Buffer"),
        contents: &buffer.into_inner(),
        usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC | BufferUsages::COPY_DST,
    });

    let brick_size = (render_data.boxtree_meta.tree_properties & 0x0000FFFF).pow(3) as u64;
    let one_voxel_byte_size = std::mem::size_of::<PaletteIndexValues>() as u64;
    let voxel_count_in_buffer = brick_size * tree_view.data_handler.bricks_in_view as u64;
    let voxels_buffer = render_device.create_buffer(&BufferDescriptor {
        mapped_at_creation: false,
        size: one_voxel_byte_size * voxel_count_in_buffer,
        label: Some("BoxTree Voxels Buffer"),
        usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC | BufferUsages::COPY_DST,
    });

    let mut buffer = StorageBuffer::new(Vec::<u8>::new());
    buffer.write(&render_data.color_palette).unwrap();
    let color_palette_buffer = render_device.create_buffer_with_data(&BufferInitDescriptor {
        label: Some("BoxTree Color Palette Buffer"),
        contents: &buffer.into_inner(),
        usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC | BufferUsages::COPY_DST,
    });

    let voxel_cache_buffer = render_device.create_buffer(&BufferDescriptor {
        mapped_at_creation: false,
        size: voxel_count_in_buffer as u64, // 1 byte per voxel
        label: Some("BoxTree Voxel Cache Buffer"),
        usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC | BufferUsages::COPY_DST,
    });

    (
        render_device.create_bind_group(
            "BoxTreeRenderData",
            &pipeline.render_data_bind_group_layout,
            &[
                bevy::render::render_resource::BindGroupEntry {
                    binding: 0,
                    resource: boxtree_meta_buffer.as_entire_binding(),
                },
                bevy::render::render_resource::BindGroupEntry {
                    binding: 1,
                    resource: node_metadata_buffer.as_entire_binding(),
                },
                bevy::render::render_resource::BindGroupEntry {
                    binding: 2,
                    resource: node_children_buffer.as_entire_binding(),
                },
                bevy::render::render_resource::BindGroupEntry {
                    binding: 3,
                    resource: node_mips_buffer.as_entire_binding(),
                },
                bevy::render::render_resource::BindGroupEntry {
                    binding: 4,
                    resource: node_ocbits_buffer.as_entire_binding(),
                },
                bevy::render::render_resource::BindGroupEntry {
                    binding: 5,
                    resource: voxels_buffer.as_entire_binding(),
                },
                bevy::render::render_resource::BindGroupEntry {
                    binding: 6,
                    resource: color_palette_buffer.as_entire_binding(),
                },
                bevy::render::render_resource::BindGroupEntry {
                    binding: 7,
                    resource: voxel_cache_buffer.as_entire_binding(),
                },
            ],
        ),
        boxtree_meta_buffer,
        node_metadata_buffer,
        node_children_buffer,
        node_mips_buffer,
        node_ocbits_buffer,
        voxels_buffer,
        color_palette_buffer,
        voxel_cache_buffer,
    )
}
