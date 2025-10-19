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

const EMPTY_MARKER: u32 = 0xFFFFFFFFu;
const BOX_NODE_DIMENSION = 4u;
const BOX_NODE_DIMENSION_SQUARED = 16u;
const BOX_NODE_DIMENSION_INV = 0.25;
const BOX_NODE_CHILDREN_COUNT = 64u;
const VOXEL_EPSILON = 0.00001;
const COLOR_FOR_NODE_REQUEST_SENT = vec3f(0.5,0.3,0.0);
const COLOR_FOR_NODE_REQUEST_FAIL = vec3f(0.7,0.2,0.0);
const COLOR_FOR_BRICK_REQUEST_SENT = vec3f(0.3,0.1,0.0);
const COLOR_FOR_BRICK_REQUEST_FAIL = vec3f(0.6,0.0,0.0);
const VHX_PREPASS_STAGE_ID = 1u;
const VHX_RENDER_STAGE_ID = 2u;
const MAX_RENDER_DISTANCE = 3.40282346638528859812e+38f;

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
        / f32(BOX_NODE_DIMENSION)
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


/// Node stack
/// The node-stack stores 3 elements, forgetting the oldest element when a new one is pushed in.
///
/// Structure(MSB first):
///  _===============================================================_
/// | Byte 0-4                                                       |
/// |----------------------------------------------------------------|
/// | bit 0-9   | MSB of data at index 0                             |
/// | bit 10-19 | MSB of data at index 1                             |
/// | bit 20-29 | MSB of data at index 2                             |
/// | bit 30-31 | stored items count within stack                    |
/// |================================================================|
/// | Byte 5-8                                                       |
/// |----------------------------------------------------------------|
/// | bit 0-9   | LSB of data at index 0                             |
/// | bit 10-19 | LSB of data at index 1                             |
/// | bit 20-29 | LSB of data at index 2                             |
/// | bit 30-31 | index of the currently used head slot              |
/// `================================================================`

//crate::raytracing::NodeStack
alias NodeStackData = array<u32, 2>;

//crate::raytracing::NodeStack::push
/// inserts the given node index into the stack
fn node_stack_push(node_stack: ptr<function, NodeStackData>, data: u32) {
    // Update head index value
    var new_head_index = ((*node_stack)[1] & 0x3u);
    new_head_index = select(new_head_index + 1u, 0u, new_head_index >= 2u);

    let head_bit_pos = (new_head_index * 10u + 2u);
    (*node_stack)[1] = (
        ((*node_stack)[1] & ~((0x3FFu << head_bit_pos) | 0x3u)) // erase previous data
        | ((data & 0x3FFu) << head_bit_pos) // add current data
        | new_head_index // add new head index
    );

    // Increase stored items count and update data on the new head
    (*node_stack)[0] = (
        ((*node_stack)[0] & ~((0x3FFu << head_bit_pos) | 0x3u )) // erase previous data
        | (((data & 0x000FFC00) >> 10u) << head_bit_pos) // add current data
        | min((((*node_stack)[0] & 0x3u) + 1), 3u) // add new stored items count
    );
}

//crate::raytracing::NodeStack::last/last_mut
/// returns with node index stored at the top of the stack, or empty marker
fn node_stack_last(node_stack: ptr<function, NodeStackData>) -> u32 {
    if 0 == ((*node_stack)[0] & 0x3u) {
        return EMPTY_MARKER;
    }
    let head_index = (*node_stack)[1] & 0x3u;
    let head_bit_pos = (head_index * 10u + 2u);
    let data_bitmask = 0x3FFu << head_bit_pos;
    return (
        (( ((*node_stack)[0] & data_bitmask) >> head_bit_pos ) << 10u)
        | (((*node_stack)[1] & data_bitmask) >> head_bit_pos )
    );
}

//crate::raytracing::NodeStack::pop
/// returns with node index stored at the top of the stack(and removes it), or empty marker
fn node_stack_pop(node_stack: ptr<function, NodeStackData>) {
    let count = (*node_stack)[0] & 0x3u;
    if 0u == count {
        return;
    }

    // set new count
    (*node_stack)[0] = ( ((*node_stack)[0] & 0xFFFFFFFCu) | (count - 1u) );

    // set new head index
    let head_index = (*node_stack)[1] & 0x3u;
    (*node_stack)[1] = (
        ((*node_stack)[1] & 0xFFFFFFFCu)
        | select( head_index - 1u, 2u, 0u == head_index )
    );
}


//crate::raytracing::dda_step_to_next_sibling
fn dda_step_to_next_sibling(
    ray_direction: vec3f,
    ray_current_point: ptr<function,vec3f>,
    current_bounds: ptr<function, Cube>,
    ray_scale_factors: vec3f
) -> vec3f {
    let ray_dir_sign = sign(ray_direction);
    let d = abs(
        ( // step_until_next_axis * ray_scale_factors
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
    return result;
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
    brick_start_index: u32,
    brick_bounds: ptr<function, Cube>,
    ray_scale_factors: vec3f,
    max_distance: f32,
) -> BrickHit {
    let dimension = i32(boxtree_meta_data.tree_properties & 0x0000FFFF);
    let dimension_squared = dimension * dimension;
    let voxels_count = i32(arrayLength(&voxels));
    var current_index = clamp(
        vec3i(floor(
            vec3f(*ray_current_point - (*brick_bounds).min_position) * f32(dimension) / (*brick_bounds).size
        )),
        vec3i(0),
        vec3i(dimension - 1)
    );
    var current_flat_index = (
        i32(brick_start_index) * (dimension_squared * dimension)
        + ( //crate::spatial::math::flat_projection
            current_index.x
            + (current_index.y * dimension)
            + (current_index.z * dimension_squared)
        )
    );
    var voxel_size = (*brick_bounds).size / f32(dimension);
    var current_bounds = Cube(
        (*brick_bounds).min_position + (vec3f(current_index) * voxel_size) - vec3f(VOXEL_EPSILON),
        fma(2., VOXEL_EPSILON, voxel_size)
    );

    /*// +++ DEBUG +++
    var safety = 0u;
    */// --- DEBUG ---
    var step = vec3f(0.);
    loop{
        /*// +++ DEBUG +++
        safety += 1u;
        if(safety > u32(f32(dimension) * sqrt(30.))) {
            return BrickHit(false, vec3u(1, 1, 1), 0);
        }
        */// --- DEBUG ---
        if any(current_index < vec3i(0)) || any(current_index >= vec3i(dimension))
        {
            return BrickHit(false, vec3u(), 0);
        }

        // step delta calculated from crate::spatial::math::flat_projection
        // --> e.g. flat_delta_y = flat_projection(0, 1, 0, brick_dim);
        current_flat_index += (
            i32(step.x)
            + i32(step.y) * dimension
            + i32(step.z) * dimension_squared
        );

        if current_flat_index >= voxels_count
        {
            return BrickHit(false, vec3u(current_index), u32(current_flat_index));
        }
        if !is_empty(voxels[current_flat_index])
        {
            return BrickHit(true, vec3u(current_index), u32(current_flat_index));
        }
        if stage_data.stage == VHX_PREPASS_STAGE_ID
            && dot(*ray_current_point - (*ray).origin, *ray_current_point - (*ray).origin) >= max_distance
        {
            return BrickHit(false, vec3u(current_index), u32(current_flat_index));
        }

        step = round(dda_step_to_next_sibling((*ray).direction, ray_current_point, &current_bounds, ray_scale_factors));
        current_bounds.min_position = fma(step, vec3f(current_bounds.size), current_bounds.min_position);
        current_index += vec3i(step);
    }

    // Technically this line is unreachable
    return BrickHit(false, vec3u(0), 0);
}

struct OctreeRayIntersection {
    hit: bool,
    voxel_index: u32,
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
                brick_descriptor, // Albedo is in color_palette, it's not a brick index in this case
                                  // which is signaled in the first (MSB) bit
                *ray_current_point,
                vec3f(0.,1.,0.) // see issue #11
            );
        } else { // brick is parted
            let leaf_brick_hit = traverse_brick(
                ray, ray_current_point,
                brick_descriptor,
                brick_bounds, ray_scale_factors,
                max_distance
            );

            if stage_data.stage == VHX_PREPASS_STAGE_ID {
                if leaf_brick_hit.hit == false && leaf_brick_hit.flat_index != 0 {
                    return OctreeRayIntersection(true, 0u, *ray_current_point, vec3f(0., 0., 1.));
                }
            }

            if leaf_brick_hit.hit == true {
                return OctreeRayIntersection(
                    true, leaf_brick_hit.flat_index, *ray_current_point,
                    vec3f(0.,1.,0.) // see issue #11
                );
            }
        }
    }
    return OctreeRayIntersection(false, 0u, *ray_current_point, vec3f(0., 0., 1.));
}

fn probe_MIP(
    ray: ptr<function, Line>,
    ray_current_point: vec3f,
    node_key: u32,
    node_bounds: ptr<function, Cube>,
    ray_scale_factors: vec3f,
) -> OctreeRayIntersection {
    if(node_mips[node_key] != EMPTY_MARKER) { // there is a valid mip present
        if(0 != (node_mips[node_key] & 0x80000000)) { // MIP brick is solid
            // Whole brick is solid, ray hits it at first connection
            return OctreeRayIntersection(
                !is_empty(node_mips[node_key]),
                node_mips[node_key], // Albedo is in color_palette, it's not a brick index in this case
                                     // which is signaled in the first (MSB) bit
                ray_current_point,
                vec3f(0.,1.,0.) // see issue #11
            );
        } else { // brick is parted
            var brick_point = ray_current_point;
            let leaf_brick_hit = traverse_brick(
                ray, &brick_point,
                node_mips[node_key] & 0x0000FFFF,
                node_bounds, ray_scale_factors,
                MAX_RENDER_DISTANCE
            );
            if leaf_brick_hit.hit == true {
                return OctreeRayIntersection(
                    true, leaf_brick_hit.flat_index, brick_point,
                    vec3f(0.,1.,0.) // see issue #11
                );
            }
        }
    }
    return OctreeRayIntersection(false, 0u, ray_current_point, vec3f(0., 0., 1.));
}

var<workgroup> root_intersect: array<CubeRayIntersection, 64>; // 8 * 8 for the given workgroup sizes

fn get_by_ray(ray: ptr<function, Line>, start_distance: f32, local_index: u32) -> OctreeRayIntersection {
    var ray_inv_dir = 1. / (*ray).direction; // Should be const, but then it can't be passed as ptr
    var ray_scale_factors = abs(ray_inv_dir); //crate::boxtree:raytracing::get_dda_scale_factors
    var tmp_vec = vec3f(1.) + normalize((*ray).direction); // using local variable as temporary storage
    // I shall answer for my crimes later
    //crate::spatial::math::hash_direction
    let boxtree_size = f32(boxtree_meta_data.boxtree_size);
    var max_distance = pow(
        select( // In the main stage the upper limit to ray travel is set by data bounds
            boxtree_size * 2., // a multiply of 2. is used instead of sqrt(3.) as an upper bounds estimation
            max_distance_of_reliable_hit(),
            stage_data.stage == VHX_PREPASS_STAGE_ID
        ),
        2.
    );

    ///  _===============================================================_
    /// | current_target - Bit structure(MSB first)                      |
    /// |----------------------------------------------------------------|
    /// | bit 0-11  | unused                                             |
    /// | bit 12-13 | node metadata                                      |
    /// | bit 14-31 | current node key                                   |
    /// `================================================================`
    var current_target = 0u;
    var target_sectant = BOX_NODE_CHILDREN_COUNT;
    var target_child_descriptor = 0u;
    var node_stack = NodeStackData();
    var ray_current_point = fma((*ray).direction, vec3f(start_distance), (*ray).origin);
    var current_bounds = Cube(vec3f(0.), boxtree_size);
    var target_bounds = current_bounds;
    var target_sectant_center = vec3f(0.);

    root_intersect[local_index] = cube_intersect_ray(current_bounds, (*ray).origin, ray_inv_dir);
    ray_current_point = select(
        ray_current_point,
        fma((*ray).direction, vec3f(root_intersect[local_index].impact_distance), ray_current_point),
        ( 0. == start_distance && root_intersect[local_index].impact_hit == true )
    );
    target_sectant = select(
        BOX_NODE_CHILDREN_COUNT,
        hash_region(ray_current_point, current_bounds.size),
        root_intersect[local_index].hit
    );

    /*// +++ DEBUG +++
    var outer_safety = 0;
    */// --- DEBUG ---
    while(
        target_sectant < BOX_NODE_CHILDREN_COUNT
        && dot(ray_current_point - (*ray).origin, ray_current_point - (*ray).origin) < max_distance
    ) {
        /*// +++ DEBUG +++
        outer_safety += 1;
        if(f32(outer_safety) > boxtree_size * sqrt(3.)) {
            return OctreeRayIntersection(true, 0u, vec3f(0.), vec3f(0., 0., 1.));
        }
        */// --- DEBUG ---
        // Init with root node: index 0 with meta
        current_target &= ~0x0FFFFFu;
        current_target |= (node_metadata[0] & 0x3) << 18;
        current_bounds.size = boxtree_size;
        current_bounds.min_position = vec3(0.);
        target_bounds.size = round(current_bounds.size * BOX_NODE_DIMENSION_INV);
        target_bounds.min_position = (sectant_offset(target_sectant) * current_bounds.size);
        target_sectant_center = fma(
            sectant_offset(target_sectant), vec3f(current_bounds.size),
            vec3f(target_bounds.size * 0.5)
        );

        // Push the root node key ( which is 0 ) and the node meta to bits 18-19
        node_stack_push(&node_stack, current_target & 0x3FFu);
        /*// +++ DEBUG +++
        var safety = 0;
        */// --- DEBUG ---
        while(
            0 != (node_stack[0] & 0x3u) //!crate::raytracing::NodeStack::is_empty
            && dot(ray_current_point - (*ray).origin, ray_current_point - (*ray).origin) < max_distance
        ) {
            /*// +++ DEBUG +++
            safety += 1;
            if(f32(safety) > boxtree_size * sqrt(30.)) {
                return OctreeRayIntersection(true, 0u, vec3f(0.), vec3f(0., 0., 1.));
            }
            */// --- DEBUG ---
            tmp_vec = ray_current_point - (*ray).origin;
            if(stage_data.stage == VHX_PREPASS_STAGE_ID && dot(tmp_vec, tmp_vec) >= max_distance) {
                return OctreeRayIntersection(false, 0u, ray_current_point, vec3f(0., 0., 1.));
            }

            target_child_descriptor = node_children[
                ((current_target & 0x3FFFFu) * BOX_NODE_CHILDREN_COUNT) + target_sectant
            ];
            if(
                (0 != (boxtree_meta_data.tree_properties & 0x00010000)) // MIPs enabled
                && target_sectant < BOX_NODE_CHILDREN_COUNT // node has a target pointing inwards
                && target_child_descriptor == EMPTY_MARKER // node doesn't have target child uploaded
                && 0u != select( // node is occupied at target sectant
                    node_occupied_bits[(current_target & 0x3FFFFu) * 2 + 1] & (0x01u << (target_sectant - 32)),
                    node_occupied_bits[(current_target & 0x3FFFFu) * 2] & (0x01u << target_sectant),
                    target_sectant < 32
                )
            ){
                var mip_hit = probe_MIP(
                    ray, ray_current_point, (current_target & 0x3FFFFu), &current_bounds,
                    ray_scale_factors
                );
                if true == mip_hit.hit {
                    return mip_hit;
                }
            }
            if( // node target points inside and is available
                target_sectant < BOX_NODE_CHILDREN_COUNT
                && target_child_descriptor != EMPTY_MARKER
                && (0 != (current_target & 0x40000u)) // node is a leaf
            ){
                // In case current node is uniform, should brick probe fail, iteration will move one level up
                // so target bounds will not be used in this evaluation loop
                target_bounds.min_position = select(
                    target_bounds.min_position, current_bounds.min_position,
                    0 != (current_target & 0x80000u) // node is uniform
                );
                target_bounds.size = select(
                    target_bounds.size, current_bounds.size,
                    0 != (current_target & 0x80000u) // node is uniform
                );
                var hit = probe_brick(
                    ray, &ray_current_point,
                    (current_target & 0x3FFFFu),
                    select(
                        target_sectant, 0u,
                        0 != (current_target & 0x80000u) // node is uniform
                    ),
                    &target_bounds,
                    ray_scale_factors,
                    max_distance
                );
                if hit.hit == true {
                    return hit;
                }
            }
            if( target_sectant >= BOX_NODE_CHILDREN_COUNT
                || (0 != (current_target & 0x80000u)) // node is uniform
            ) {
                // POP
                node_stack_pop(&node_stack);
                target_bounds = current_bounds;
                current_bounds.size *= f32(BOX_NODE_DIMENSION);
                current_bounds.min_position -= current_bounds.min_position % current_bounds.size;
                let ray_point_before_pop = ray_current_point;

                tmp_vec = round(dda_step_to_next_sibling((*ray).direction, &ray_current_point, &target_bounds, ray_scale_factors));
                if(
                    stage_data.stage == VHX_PREPASS_STAGE_ID
                    && dot(ray_current_point - (*ray).origin, ray_current_point - (*ray).origin) >= max_distance
                ) {
                    return OctreeRayIntersection(false, 0u, ray_point_before_pop, vec3f(0., 0., 1.));
                }
                target_sectant_center = fma(
                    tmp_vec, vec3f(target_bounds.size),
                    target_bounds.min_position + vec3f(target_bounds.size * 0.5)
                );
                target_sectant = select(
                    hash_region(
                        (target_sectant_center - current_bounds.min_position),
                        current_bounds.size
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
                // node stack contains both the node id(18 bits) and the node meta(2 bits)
                current_target &= ~0x0FFFFFu;
                current_target |= node_stack_last(&node_stack) & 0x0FFFFFu;
                continue;
            }
            if (
                (target_child_descriptor != EMPTY_MARKER) // target is available
                &&(0 == (current_target & 0x40000u)) // node is not leaf
            ) {
                // PUSH
                current_target &= ~0x0FFFFFu;
                current_target |= (
                    ((node_metadata[target_child_descriptor / 16] >> (2 * (target_child_descriptor % 16))) & 0x3) << 18
                    | (target_child_descriptor & 0x3FFFFu)
                );
                current_bounds = target_bounds;
                target_sectant = hash_region( // new target sectant
                    (ray_current_point - target_bounds.min_position),
                    target_bounds.size
                );
                target_bounds.size = round(current_bounds.size * BOX_NODE_DIMENSION_INV);
                target_bounds.min_position = fma(
                    sectant_offset(target_sectant), vec3f(current_bounds.size),
                    current_bounds.min_position
                );
                target_sectant_center = target_bounds.min_position + vec3f(target_bounds.size * 0.5);
                target_child_descriptor = ( // cache the node metadata together with the child node id
                    (target_child_descriptor & 0x3FFFF) | (current_target & 0xC0000u)
                );
                node_stack_push(&node_stack, target_child_descriptor);
            } else {
                // ADVANCE
                /*// +++ DEBUG +++
                var advance_safety = 0;
                */// --- DEBUG ---
                loop {
                    /*// +++ DEBUG +++
                    advance_safety += 1;
                    if(advance_safety > 16) {
                        return OctreeRayIntersection(true, 0u, vec3f(0.), vec3f(0., 0., 1.));
                    }
                    */// --- DEBUG ---
                    tmp_vec = round(dda_step_to_next_sibling((*ray).direction, &ray_current_point, &target_bounds, ray_scale_factors));
                    target_sectant_center = fma(tmp_vec, vec3f(target_bounds.size), target_sectant_center);
                    target_sectant = select(
                        hash_region(
                            (target_sectant_center - current_bounds.min_position),
                            current_bounds.size
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
                        node_children[((current_target & 0x3FFFFu) * BOX_NODE_CHILDREN_COUNT) + target_sectant],
                        target_sectant < BOX_NODE_CHILDREN_COUNT
                    );

                    tmp_vec = ray_current_point - (*ray).origin;
                    if (
                        target_sectant >= BOX_NODE_CHILDREN_COUNT // target is out of bounds
                        || 0u != select( // node is occupied at target sectant(can't use target child as it may not be uploaded)
                            node_occupied_bits[(current_target & 0x3FFFFu) * 2 + 1] & (0x01u << (target_sectant - 32)),
                            node_occupied_bits[(current_target & 0x3FFFFu) * 2] & (0x01u << target_sectant),
                            target_sectant < 32
                        )
                        || dot(tmp_vec, tmp_vec) >= max_distance
                    ) {
                        break;
                    }
                } // advance loop
            }
        } // while (node_stack not empty)

        // Push ray current distance a little bit forward to avoid iterating the same paths all over again
        tmp_vec = ray_current_point - (*ray).origin;
        ray_current_point = fma((*ray).direction, vec3f(10.0 * VOXEL_EPSILON), ray_current_point);
        target_sectant = select(
            BOX_NODE_CHILDREN_COUNT,
            hash_region(ray_current_point, boxtree_size),
            dot(tmp_vec, tmp_vec) < max_distance
            && all(ray_current_point < vec3f(boxtree_size))
            && all(ray_current_point > vec3f(0.))
        );
    } // while (ray inside root bounds)
    return OctreeRayIntersection(false, 0u, ray_current_point, vec3f(0., 0., 1.));
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

const BOXTREE_ROOT_NODE_KEY = 0u;
struct BoxtreeMetaData {
    sun_color: vec3f,
    sun_direction: vec3f,
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
fn update(
    @builtin(global_invocation_id) invocation_id: vec3<u32>,
    @builtin(local_invocation_id) local_id: vec3<u32>
) {
    // Calculate NDC (Normalized Device Coordinates) from pixel coordinates
    let ndc_x = (f32(invocation_id.x) + 0.5) / f32(stage_data.output_resolution.x) * 2.0 - 1.0;
    let ndc_y = -((f32(invocation_id.y) + 0.5) / f32(stage_data.output_resolution.y) * 2.0 - 1.0);

    // Transform NDC coordinates to world space
    let world_near = viewport.inverse_view_projection_matrix * vec4f(ndc_x, ndc_y, -1.0, 1.0); // near plane in NDC
    let world_far = viewport.inverse_view_projection_matrix * vec4f(ndc_x, ndc_y, 1.0, 1.0); // far plane in NDC
    
    let ray_local_index = 8u * local_id.y + local_id.x;
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
            vec4f(length(get_by_ray(&ray, 0., ray_local_index).impact_point - ray.origin))
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

        /*// +++ DEBUG +++
        var root_bounds = Cube(vec3(0.,0.,0.), f32(boxtree_meta_data.boxtree_size));
        let root_intersect = cube_intersect_ray(root_bounds, &ray, 1. / ray.direction);
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
        var primary_raycast = get_by_ray(&ray, start_distance, ray_local_index);

        if(primary_raycast.hit){
            // shadow ray: cast ray from impact point to lightsource
            var shadow_ray = Line(primary_raycast.impact_point, boxtree_meta_data.sun_direction);

            // start shadow raycast from inside the voxel to avoid self-intersection
            var shadow_raycast = get_by_ray(&shadow_ray, 0.1, ray_local_index);
            var shadow_modifier = clamp(( // the farther away the hit is, the less the hard shadow is going to count
                0.3 * length(shadow_raycast.impact_point - primary_raycast.impact_point)
                / ( // hard shadows get softer when sunlight block is far away
                    f32(boxtree_meta_data.tree_properties & 0x0000FFFF) * 4.
                )
            ), 0.3, 1.);

            // If the shadow ray hits something, the point is in shadow
            rgb_result = select(
                vec3f(0.),
                color_palette[voxels[primary_raycast.voxel_index] & 0x0000FFFF].rgb * shadow_modifier,
                primary_raycast.hit
            );
        }

        textureStore(output_texture, vec2u(invocation_id.xy), vec4f(rgb_result, 1.));
    }
}