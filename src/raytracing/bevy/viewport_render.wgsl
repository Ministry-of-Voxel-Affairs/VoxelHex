// The time since startup data is in the globals binding which is part of the mesh_view_bindings import
#import bevy_pbr::{
    mesh_view_bindings::globals,
    forward_io::VertexOutput
}

struct Line {
    origin: vec3f,
    direction: vec3f,
}

struct Cube {
    min_position: vec3f,
    size: f32,
}

#define BOX_NODE_DIMENSION = 4u
#define BOX_NODE_DIMENSION_SQUARED = 16u
#define BOX_NODE_SIZE_MULTIPLIER 1. / BOX_NODE_DIMENSION
#define HALF_BOX_NODE_MULTIPLIER BOX_NODE_SIZE_MULTIPLIER / 2.
#define BOX_NODE_CHILDREN_COUNT = 64u
#define VOXEL_EPSILON = 0.00001
#define COLOR_FOR_NODE_REQUEST_SENT = vec3f(0.5,0.3,0.0)
#define COLOR_FOR_NODE_REQUEST_FAIL = vec3f(0.7,0.2,0.0)
#define COLOR_FOR_BRICK_REQUEST_SENT = vec3f(0.3,0.1,0.0)
#define COLOR_FOR_BRICK_REQUEST_FAIL = vec3f(0.6,0.0,0.0)
#define VHX_PREPASS_STAGE_ID = 1u
#define VHX_RENDER_STAGE_ID = 2u

//crate::spatial::math::hash_region
fn hash_region(offset: vec3f, size: f32) -> u32 {
    let index = vec3u(clamp(
        vec3i(floor(offset * f32(BOX_NODE_DIMENSION) / size)),
        vec3i(0),
        vec3i(BOX_NODE_DIMENSION - 1)
    ));
    return (
        index.x
        + (index.y * BOX_NODE_DIMENSION)
        + (index.z * BOX_NODE_DIMENSION_SQUARED)
    );
}

// Unique to this implementation, not adapted from rust code
// used to be crate::spatial::math::sectant_offset, but was replaced by LUT
fn sectant_offset(sectant_index: u32) -> vec3f {
    return (
        vec3f(
            f32(sectant_index % BOX_NODE_DIMENSION),
            f32((sectant_index % BOX_NODE_DIMENSION_SQUARED) / BOX_NODE_DIMENSION),
            f32(sectant_index / BOX_NODE_DIMENSION_SQUARED)
        )
        * BOX_NODE_SIZE_MULTIPLIER
    );
}

struct CubeRayIntersection {
    hit: bool,
    impact_hit: bool,
    impact_distance: f32,
    exit_distance: f32,
}

//crate::spatial::raytracing::Cube::intersect_ray
fn cube_intersect_ray(cube: Cube, ray_origin: vec3f, ray_inv_dir: vec3f) -> CubeRayIntersection {
    // Calculate intersection parameters for all axes at once
    let t_min = (cube.min_position - ray_origin) * ray_inv_dir;
    let t_max = ((cube.min_position + vec3f(cube.size)) - ray_origin) * ray_inv_dir;

    // Handle negative ray directions
    let t_near = min(t_min, t_max);
    let t_far = max(t_min, t_max);

    // Find intersection interval
    let tmin = max(t_near.x, max(t_near.y, t_near.z));
    let tmax = min(t_far.x, min(t_far.y, t_far.z));
    // Branchless intersection test
    let has_intersection = (tmax >= 0.0) && (tmin <= tmax);
    let ray_starts_outside = tmin >= 0.0;

    return CubeRayIntersection(
        has_intersection,
        has_intersection && ray_starts_outside,
        select(0.0, tmin, ray_starts_outside),
        tmax
    );
}

//crate::raytracing::NodeStack
const NODE_STACK_SIZE: u32 = 4;
const EMPTY_MARKER: u32 = 0xFFFFFFFFu;

//crate::raytracing::NodeStack::push
fn node_stack_push(
    node_stack: ptr<function,array<u32, NODE_STACK_SIZE>>,
    node_stack_meta: ptr<function, u32>,
    data: u32,
){
    *node_stack_meta = (
        // count
        ( min(NODE_STACK_SIZE, ((*node_stack_meta & 0x000000FFu) + 1)) & 0x000000FFu)
        // head_index
        | ( ((
            ( ((*node_stack_meta & 0x0000FF00u) >> 8u) + 1 ) % NODE_STACK_SIZE
        ) << 8u) & 0x0000FF00u )
    );
    (*node_stack)[(*node_stack_meta & 0x0000FF00u) >> 8u] = data;
}


//crate::raytracing::NodeStack::pop
fn node_stack_pop(
    node_stack: ptr<function,array<u32, NODE_STACK_SIZE>>,
    node_stack_meta: ptr<function, u32>,
) -> u32 { // returns either with index or EMPTY_MARKER
    if 0 == (*node_stack_meta & 0x000000FFu) {
        return EMPTY_MARKER;
    }
    let result = (*node_stack)[(*node_stack_meta & 0x0000FF00u) >> 8u];
    *node_stack_meta = select(
        (
            // count
            ( ((*node_stack_meta & 0x000000FFu) - 1) )
            // head_index
            | ( ((
                ( ((*node_stack_meta & 0x0000FF00u) >> 8u) - 1 )
            ) << 8u) & 0x0000FF00u )
        ),
        (
            // count
            ( ((*node_stack_meta & 0x000000FFu) - 1) )
            // head_index
            | ((NODE_STACK_SIZE - 1) << 8u)
        ),
        0 == (*node_stack_meta & 0x0000FF00u) // head index is 0
    );
    return result;
}

//crate::raytracing::NodeStack::last/last_mut
fn node_stack_last(node_stack_meta: u32) -> u32 { // returns either with index or EMPTY_MARKER
    return select(
        (node_stack_meta & 0x0000FF00u) >> 8u,
        EMPTY_MARKER,
        0 == (node_stack_meta & 0x000000FFu)
    );
}

struct DdaResult {
    step_direction: vec3f,
    step_distance: f32,
}

//crate::raytracing::dda_step_to_next_sibling
fn dda_step_to_next_sibling(
    ray_direction: vec3f,
    ray_current_point: ptr<function,vec3f>,
    current_bounds: ptr<function, Cube>,
    ray_scale_factors: vec3f
) -> DdaResult {
    let ray_dir_sign = sign(ray_direction);
    let d = abs(
        (
            fma(
                vec3f((*current_bounds).size), max(ray_dir_sign, vec3f(0.)),
                -(ray_dir_sign * (*ray_current_point - (*current_bounds).min_position))
            )
        ) * ray_scale_factors
    );
    let min_step = min(d.x, min(d.y, d.z));
    var result = vec3f(0.);

    (*ray_current_point) = fma(ray_direction, vec3f(min_step), (*ray_current_point));
    result = select(result, ray_dir_sign, vec3f(min_step) == d);
    return DdaResult(result, min_step);
}

struct BrickHit{
    hit: bool,
    index: vec3u,
    flat_index: u32,
}

/// In preprocess, a small resolution depth texture is rendered.
/// After a certain distance in the ray, the result becomes ambigious,
/// because the pixel ( source of raycast ) might cover multiple voxels at the same time.
/// The estimate distance before the ambigiutiy is still adequate is calculated based on:
/// texture_resolution / voxels_count(distance) >= minimum_size_of_voxel_in_pixels
/// wherein:
/// voxels_count: the number of voxel estimated to take up the viewport at a given distance
/// minimum_size_of_voxel_in_pixels: based on the depth texture half the size of the output
/// --> the size of a voxel to be large enough to be always contained by
/// --> at least one pixel in the depth texture
/// No need to continue iteration if one voxel becomes too small to be covered by a pixel completely
/// In these cases, there were no hits so far, which is valuable information
/// even if no useful data can be collected moving forward.
fn max_distance_of_reliable_hit() -> f32 {
    return (
        viewport.fov
        * f32(stage_data.output_resolution.x * stage_data.output_resolution.y)
        / (viewport.frustum.x * viewport.frustum.y * 2.828427125/*sqrt(8.)*/)
    ) - viewport.fov;
}

fn traverse_brick(
    ray: ptr<function, Line>,
    ray_current_point: ptr<function,vec3f>,
    distance_traveled: ptr<function, f32>,
    brick_start_index: u32,
    brick_bounds: ptr<function, Cube>,
    ray_scale_factors: vec3f,
    max_distance: f32,
) -> BrickHit {
    let dimension = i32(boxtree_meta_data.tree_properties & 0x0000FFFF);
    let inv_dimension = 1.0 / f32(dimension);
    let inv_brick_size = 1.0 / (*brick_bounds).size;
    let voxels_count = i32(arrayLength(&voxels));

    var current_index = clamp(
        vec3i(floor(
            vec3f(*ray_current_point - (*brick_bounds).min_position) * (f32(dimension) * inv_brick_size)
        )),
        vec3i(0),
        vec3i(dimension - 1)
    );
    var current_flat_index = (
        i32(brick_start_index) * (dimension * dimension * dimension)
        + (
            current_index.x
            + (current_index.y * dimension)
            + (current_index.z * dimension * dimension)
        )
    );
    var voxel_size = (*brick_bounds).size * inv_dimension;
    var current_bounds = Cube(
        (*brick_bounds).min_position + (vec3f(current_index) * voxel_size) - vec3f(VOXEL_EPSILON),
        fma(2., VOXEL_EPSILON, voxel_size)
    );

    var step = vec3f(0.);
    loop{
        if any(current_index < vec3i(0)) || any(current_index >= vec3i(dimension)) {
            return BrickHit(false, vec3u(), 0);
        }

        current_flat_index += (
            i32(step.x)
            + i32(step.y) * dimension
            + i32(step.z) * dimension * dimension
        );

        if current_flat_index >= voxels_count {
            return BrickHit(false, vec3u(current_index), u32(current_flat_index));
        }
        if !is_empty(voxels[current_flat_index]) {
            return BrickHit(true, vec3u(current_index), u32(current_flat_index));
        }
        if stage_data.stage == VHX_PREPASS_STAGE_ID && *distance_traveled >= max_distance {
            return BrickHit(false, vec3u(current_index), u32(current_flat_index));
        }

        let dda_result = dda_step_to_next_sibling((*ray).direction, ray_current_point, &current_bounds, ray_scale_factors);
        *distance_traveled += dda_result.step_distance;
        step = round(dda_result.step_direction);

        current_bounds.min_position = fma(step, vec3f(current_bounds.size), current_bounds.min_position);
        current_index += vec3i(step);
    }

    return BrickHit(false, vec3u(0), 0);
}

struct OctreeRayIntersection {
    hit: bool,
    albedo : vec4<f32>,
    impact_point: vec3f,
    impact_normal: vec3f,
}

fn probe_brick(
    ray: ptr<function, Line>,
    ray_current_point: ptr<function,vec3f>,
    leaf_node_key: u32,
    brick_sectant: u32,
    brick_bounds: ptr<function, Cube>,
    ray_scale_factors: vec3f,
    max_distance: f32,
) -> OctreeRayIntersection {
    let brick_descriptor = node_children[
        ((leaf_node_key * BOX_NODE_CHILDREN_COUNT) + brick_sectant)
    ];
    if(EMPTY_MARKER != brick_descriptor){
        if(0 != (0x80000000 & brick_descriptor)) { // brick is solid
            // Whole brick is solid, ray hits it at first connection
            return OctreeRayIntersection(
                !is_empty(brick_descriptor),
                color_palette[brick_descriptor & 0x0000FFFF], // Albedo is in color_palette, it's not a brick index in this case
                *ray_current_point,
                vec3f(0.,1.,0.) // see issue #11
            );
        } else { // brick is parted
            let leaf_brick_hit = traverse_brick(
                ray, ray_current_point,
                brick_descriptor & 0x0000FFFF,
                brick_bounds, ray_scale_factors,
                max_distance
            );

            if stage_data.stage == VHX_PREPASS_STAGE_ID {
                if leaf_brick_hit.hit == false && leaf_brick_hit.flat_index != 0 {
                    return OctreeRayIntersection(true, vec4f(0.), *ray_current_point, vec3f(0., 0., 1.));
                }
            }

            if leaf_brick_hit.hit == true {
                return OctreeRayIntersection(
                    true,
                    color_palette[voxels[leaf_brick_hit.flat_index] & 0x0000FFFF],
                    *ray_current_point,
                    vec3f(0.,1.,0.) // see issue #11
                );
            }
        }
    }
    return OctreeRayIntersection(false, vec4f(0.), *ray_current_point, vec3f(0., 0., 1.));
}

fn probe_MIP(
    ray: ptr<function, Line>,
    ray_current_point: vec3f,
    node_key: u32,
    node_bounds: ptr<function, Cube>,
    ray_scale_factors: vec3f,
    max_distance: f32
) -> OctreeRayIntersection {
    if(node_mips[node_key] != EMPTY_MARKER) { // there is a valid mip present
        if(0 != (node_mips[node_key] & 0x80000000)) { // MIP brick is solid
            // Whole brick is solid, ray hits it at first connection
            return OctreeRayIntersection(
                !is_empty(node_mips[node_key]),
                color_palette[node_mips[node_key] & 0x0000FFFF], // Albedo is in color_palette, it's not a brick index in this case
                ray_current_point,
                vec3f(0.,1.,0.) // see issue #11
            );
        } else { // brick is parted
            var brick_point = ray_current_point;
            let leaf_brick_hit = traverse_brick(
                ray, &brick_point,
                node_mips[node_key] & 0x0000FFFF,
                node_bounds, ray_scale_factors,
                max_distance
            );
            if leaf_brick_hit.hit == true {
                return OctreeRayIntersection(
                    true,
                    color_palette[voxels[leaf_brick_hit.flat_index] & 0x0000FFFF],
                    brick_point,
                    vec3f(0.,1.,0.) // see issue #11
                );
            }
        }
    }
    return OctreeRayIntersection(false, vec4f(0.), ray_current_point, vec3f(0., 0., 1.));
}

fn get_by_ray(ray: ptr<function, Line>, start_distance: f32) -> OctreeRayIntersection {
    var ray_inv_dir = 1. / (*ray).direction;
    var ray_scale_factors = abs(ray_inv_dir);
    var tmp_vec = vec3f(1.) + normalize((*ray).direction);
    let boxtree_size = f32(boxtree_meta_data.boxtree_size);
    let inv_boxtree_size = 1.0 / boxtree_size;

    let linear_max_distance = select(
        boxtree_size * 1.73205, // Use sqrt(3.0) for a tighter bound than 2.0
        max_distance_of_reliable_hit(),
        stage_data.stage == VHX_PREPASS_STAGE_ID
    );

    var node_stack: array<u32, NODE_STACK_SIZE>;
    var node_stack_meta: u32 = 0;

    var distance_traveled = start_distance;
    var ray_current_point = fma((*ray).direction, vec3f(start_distance), (*ray).origin);

    var current_bounds = Cube(vec3f(0.), boxtree_size);
    var current_node_metadata = 0u;
    var current_node_key = BOXTREE_ROOT_NODE_KEY;
    var target_bounds = current_bounds;
    var target_sectant = BOX_NODE_CHILDREN_COUNT;
    var target_sectant_center = vec3f(0.);
    var target_child_descriptor = 0u;
    let root_intersect = cube_intersect_ray(current_bounds, (*ray).origin, ray_inv_dir);

    ray_current_point = select(
        ray_current_point,
        fma((*ray).direction, vec3f(root_intersect.impact_distance), (*ray).origin),
        ( 0. == start_distance && root_intersect.impact_hit == true )
    );
    target_sectant = select(
        BOX_NODE_CHILDREN_COUNT,
        hash_region(ray_current_point, inv_boxtree_size),
        root_intersect.hit
    );

    while(
        target_sectant < BOX_NODE_CHILDREN_COUNT
        && distance_traveled < linear_max_distance
    ) {
        current_node_key = BOXTREE_ROOT_NODE_KEY;
        let metadata_index_root = current_node_key >> 4u;
        let bit_shift_root = (current_node_key & 15u) << 1u;
        current_node_metadata = (node_metadata[metadata_index_root] >> bit_shift_root) & 0x3;

        current_bounds.size = boxtree_size;
        current_bounds.min_position = vec3(0.);
        target_bounds.size = round(current_bounds.size * BOX_NODE_SIZE_MULTIPLIER);
        target_bounds.min_position = (sectant_offset(target_sectant) * current_bounds.size);
        target_sectant_center = fma(
            sectant_offset(target_sectant), vec3f(current_bounds.size),
            vec3f(target_bounds.size / 2.)
        );

        node_stack_push(&node_stack, &node_stack_meta, BOXTREE_ROOT_NODE_KEY);

        while(
            0 != (node_stack_meta & 0x000000FFu) // is_empty
            && distance_traveled < linear_max_distance
        ) {
            if(
                stage_data.stage == VHX_PREPASS_STAGE_ID
                && distance_traveled >= linear_max_distance
            ) {
                return OctreeRayIntersection( false, vec4f(0.), ray_current_point, vec3f(0., 0., 1.) );
            }

            target_child_descriptor = node_children[(current_node_key * BOX_NODE_CHILDREN_COUNT) + target_sectant];
            if(
                (0 != (boxtree_meta_data.tree_properties & 0x00010000))
                && target_sectant < BOX_NODE_CHILDREN_COUNT
                && target_child_descriptor == EMPTY_MARKER
                && 0u != select(
                    node_occupied_bits[current_node_key * 2 + 1] & (0x01u << (target_sectant - 32)),
                    node_occupied_bits[current_node_key * 2] & (0x01u << target_sectant),
                    target_sectant < 32
                )
            ){
                var mip_hit = probe_MIP(
                    ray, ray_current_point, &distance_traveled, current_node_key, &current_bounds,
                    ray_scale_factors, linear_max_distance
                );
                if true == mip_hit.hit {
                    return mip_hit;
                }
            }
            if(
                target_sectant < BOX_NODE_CHILDREN_COUNT
                && target_child_descriptor != EMPTY_MARKER
                && (0 != (current_node_metadata & 0x01u))
            ){
                target_bounds.min_position = select(
                    target_bounds.min_position, current_bounds.min_position,
                    0 != (current_node_metadata & 0x02u)
                );
                target_bounds.size = select(
                    target_bounds.size, current_bounds.size,
                    0 != (current_node_metadata & 0x02u)
                );
                var hit = probe_brick(
                    ray, &ray_current_point, &distance_traveled,
                    current_node_key,
                    select(target_sectant, 0u, 0 != (current_node_metadata & 0x02u)),
                    &target_bounds,
                    ray_scale_factors,
                    linear_max_distance
                );
                if hit.hit == true {
                    return hit;
                }
            }
            if( target_sectant >= BOX_NODE_CHILDREN_COUNT
                || (0 != (current_node_metadata & 0x02u))
            ) {
                node_stack_pop(&node_stack, &node_stack_meta);
                target_bounds = current_bounds;
                current_bounds.size *= f32(BOX_NODE_DIMENSION);
                current_bounds.min_position -= current_bounds.min_position % current_bounds.size;
                let ray_point_before_pop = ray_current_point;

                let dda_result_pop = dda_step_to_next_sibling((*ray).direction, &ray_current_point, &target_bounds, ray_scale_factors);
                distance_traveled += dda_result_pop.step_distance;
                tmp_vec = round(dda_result_pop.step_direction);

                if(
                    stage_data.stage == VHX_PREPASS_STAGE_ID
                    && distance_traveled >= linear_max_distance
                ) {
                    return OctreeRayIntersection( false, vec4f(0.), ray_point_before_pop, vec3f(0., 0., 1.) );
                }
                target_sectant_center = fma(
                    tmp_vec, vec3f(target_bounds.size),
                    target_bounds.min_position + vec3f(target_bounds.size / 2.)
                );
                let inv_curr_bounds_size_pop = 1.0 / current_bounds.size;
                target_sectant = select(
                    hash_region(
                        (target_sectant_center - current_bounds.min_position),
                        inv_curr_bounds_size_pop
                    ),
                    BOX_NODE_CHILDREN_COUNT,
                    ( any(target_sectant_center < current_bounds.min_position)
                        || any(
                            target_sectant_center >= (
                                current_bounds.min_position
                                + vec3f(current_bounds.size)
                            )
                        )
                    )
                );
                target_bounds.min_position += tmp_vec * target_bounds.size;
                current_node_key = select(
                    current_node_key,
                    node_stack[node_stack_last(node_stack_meta)],
                    EMPTY_MARKER != node_stack_last(node_stack_meta),
                );
                let metadata_index_pop = current_node_key >> 4u;
                let bit_shift_pop = (current_node_key & 15u) << 1u;
                current_node_metadata = (node_metadata[metadata_index_pop] >> bit_shift_pop) & 0x3;
                continue;
            }
            if (
                (target_child_descriptor != EMPTY_MARKER)
                &&(0 == (current_node_metadata & 0x01u))
            ) {
                // PUSH
                current_node_key = target_child_descriptor;
                let metadata_index_push = current_node_key >> 4u;
                let bit_shift_push = (current_node_key & 15u) << 1u;
                current_node_metadata = (node_metadata[metadata_index_push] >> bit_shift_push) & 0x3;

                current_bounds = target_bounds;
                let inv_target_bounds_size = 1.0 / target_bounds.size;

                target_sectant = hash_region(
                    (ray_current_point - target_bounds.min_position),
                    inv_target_bounds_size
                );
                target_bounds.size = round(current_bounds.size * BOX_NODE_SIZE_MULTIPLIER);
                target_bounds.min_position = fma(
                    sectant_offset(target_sectant), vec3f(current_bounds.size),
                    current_bounds.min_position
                );
                target_sectant_center = target_bounds.min_position + vec3f(target_bounds.size / 2.);
                node_stack_push(&node_stack, &node_stack_meta, target_child_descriptor);
            } else {
                // ADVANCE
                let inv_curr_bounds_size_adv = 1.0 / current_bounds.size;
                loop {
                    let dda_result_adv = dda_step_to_next_sibling((*ray).direction, &ray_current_point, &target_bounds, ray_scale_factors);
                    distance_traveled += dda_result_adv.step_distance;
                    tmp_vec = round(dda_result_adv.step_direction);

                    target_sectant_center = fma(tmp_vec, vec3f(target_bounds.size), target_sectant_center);
                    target_sectant = select(
                        hash_region(
                            (target_sectant_center - current_bounds.min_position),
                            inv_curr_bounds_size_adv
                        ),
                        BOX_NODE_CHILDREN_COUNT,
                        ( any(target_sectant_center < current_bounds.min_position)
                            || any(
                                target_sectant_center >= (
                                    current_bounds.min_position
                                    + vec3f(current_bounds.size)
                                )
                            )
                        )
                    );
                    target_bounds.min_position = fma(tmp_vec, vec3f(target_bounds.size), target_bounds.min_position);
                    target_child_descriptor = select(
                        EMPTY_MARKER,
                        node_children[(current_node_key * BOX_NODE_CHILDREN_COUNT) + target_sectant],
                        target_sectant < BOX_NODE_CHILDREN_COUNT
                    );

                    if (
                        target_sectant >= BOX_NODE_CHILDREN_COUNT
                        || 0u != select(
                            node_occupied_bits[current_node_key * 2 + 1] & (0x01u << (target_sectant - 32)),
                            node_occupied_bits[current_node_key * 2] & (0x01u << target_sectant),
                            target_sectant < 32
                        )
                        || distance_traveled >= linear_max_distance
                    ) {
                        break;
                    }
                }
            }
        }

        let dda_step = dda_step_to_next_sibling((*ray).direction, &ray_current_point, &target_bounds, ray_scale_factors);
        distance_traveled += dda_step.step_distance;
        
        target_sectant = select(
            BOX_NODE_CHILDREN_COUNT,
            hash_region(ray_current_point, inv_boxtree_size),
            distance_traveled < linear_max_distance
            && all(ray_current_point < vec3f(boxtree_size))
            && all(ray_current_point > vec3f(0.))
        );
    }
    return OctreeRayIntersection(false, vec4f(0.), ray_current_point, vec3f(0., 0., 1.));
}

alias PaletteIndexValues = u32;

fn is_empty(e: PaletteIndexValues) -> bool {
    return (
        (0x0000FFFF == (0x0000FFFF & e))
        ||(
            0. == color_palette[e & 0x0000FFFF].a
            && 0. == color_palette[e & 0x0000FFFF].r
            && 0. == color_palette[e & 0x0000FFFF].g
            && 0. == color_palette[e & 0x0000FFFF].b
        )
    );
}

// why not only check (alpha) channel if engine can guarantee that all empty ones have pallete.a == 0

const BOXTREE_ROOT_NODE_KEY = 0u;
struct BoxtreeMetaData {
    ambient_light_color: vec3f,
    ambient_light_position: vec3f,
    boxtree_size: u32,
    tree_properties: u32,
}

struct Viewport {
    origin: vec3f,
    origin_delta: vec3f,
    direction: vec3f,
    frustum: vec3f,
    fov: f32,
    view_matrix: mat4x4<f32>,
    projection_matrix: mat4x4<f32>,
    inverse_view_projection_matrix: mat4x4<f32>,
}

struct RenderStageData {
    stage: u32,
    output_resolution: vec2u,
}

@group(0) @binding(0)
var<uniform> stage_data: RenderStageData;

@group(0) @binding(1)
var output_texture: texture_storage_2d<rgba8unorm, read_write>;

@group(0) @binding(2)
var depth_texture: texture_storage_2d<r32float, read_write>;

@group(1) @binding(0)
var<uniform> viewport: Viewport;

@group(2) @binding(0)
var<uniform> boxtree_meta_data: BoxtreeMetaData;

@group(2) @binding(1)
var<storage, read> node_metadata: array<u32>;

@group(2) @binding(2)
var<storage, read> node_children: array<u32>;

@group(2) @binding(3)
var<storage, read> node_mips: array<u32>;

@group(2) @binding(4)
var<storage, read> node_occupied_bits: array<u32>;

@group(2) @binding(5)
var<storage, read> voxels: array<PaletteIndexValues>;

@group(2) @binding(6)
var<storage, read> color_palette: array<vec4f>;


@compute @workgroup_size(8, 8, 1)
fn update(@builtin(global_invocation_id) invocation_id: vec3<u32>) {
    // Calculate NDC (Normalized Device Coordinates) from pixel coordinates
    let ndc_x = (f32(invocation_id.x) + 0.5) / f32(stage_data.output_resolution.x) * 2.0 - 1.0;
    let ndc_y = -((f32(invocation_id.y) + 0.5) / f32(stage_data.output_resolution.y) * 2.0 - 1.0);

    // Transform NDC coordinates to world space
    let world_near = viewport.inverse_view_projection_matrix * vec4f(ndc_x, ndc_y, -1.0, 1.0); // near plane in NDC
    let world_far = viewport.inverse_view_projection_matrix * vec4f(ndc_x, ndc_y, 1.0, 1.0); // far plane in NDC
    
    var ray = Line(
        viewport.origin,
        normalize((world_far.xyz / world_far.w) - (world_near.xyz / world_near.w))
    );
    if stage_data.stage == VHX_PREPASS_STAGE_ID {
        // In preprocess, for every pixel in the depth texture, traverse the model until
        // either there's a hit or the voxels are too far away to determine 
        // exactly which pixel belongs to which voxel
        textureStore(
            depth_texture, vec2u(invocation_id.xy),
            vec4f(length(get_by_ray(&ray, 0.).impact_point - ray.origin))
        );
    } else
    if stage_data.stage == VHX_RENDER_STAGE_ID {
        var rgb_result = vec3f(0.5,1.0,1.0);

        // get relevant pixels in depth
        let start_distance = min(
            textureLoad(depth_texture, vec2u(invocation_id.xy / 2)),
            min(
                textureLoad(depth_texture, vec2u(invocation_id.xy / 2) + vec2u(0,1)),
                min(
                    textureLoad(depth_texture, vec2u(invocation_id.xy / 2) + vec2u(1,0)),
                    textureLoad(depth_texture, vec2u(invocation_id.xy / 2) + vec2u(1,1))
                )
            )
        ).r;

        var ray_result = get_by_ray(&ray, start_distance);
        /*// +++ DEBUG +++
        var root_bounds = Cube(vec3(0.,0.,0.), f32(boxtree_meta_data.boxtree_size));
        let root_intersect = cube_intersect_ray(root_bounds, &ray, rcp(ray.direction));
        if root_intersect.hit == true {
            // Display the xyz axes
            if root_intersect. impact_hit == true {
                let axes_length = f32(boxtree_meta_data.boxtree_size) / 2.;
                let axes_width = f32(boxtree_meta_data.boxtree_size) / 50.;
                let entry_point = (ray.origin + ray.direction * root_intersect.impact_distance);
                if entry_point.x < axes_length && entry_point.y < axes_width && entry_point.z < axes_width {
                    rgb_result.r = 1.;
                }
                if entry_point.x < axes_width && entry_point.y < axes_length && entry_point.z < axes_width {
                    rgb_result.g = 1.;
                }
                if entry_point.x < axes_width && entry_point.y < axes_width && entry_point.z < axes_length {
                    rgb_result.b = 1.;
                }
            }
            rgb_result.b += 0.1; // Also color in the area of the boxtree
        }
        */// --- DEBUG ---
        rgb_result = select(
            (rgb_result + ray_result.albedo.rgb) / 2.,
            (ray_result.albedo.rgb * (dot(ray_result.impact_normal, vec3f(-0.5,0.5,-0.5)) / 2. + 0.5)).rgb,
            ray_result.hit
        );

        textureStore(output_texture, vec2u(invocation_id.xy), vec4f(rgb_result, 1.));
    }
}
