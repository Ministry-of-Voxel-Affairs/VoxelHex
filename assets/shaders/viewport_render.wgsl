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

const OOB_SECTANT = 64u;
const BOX_NODE_DIMENSION = 4u;
const BOX_NODE_CHILDREN_COUNT = 64u;
const FLOAT_ERROR_TOLERANCE = 0.00001;
const COLOR_FOR_NODE_REQUEST_SENT = vec3f(0.5,0.3,0.0);
const COLOR_FOR_NODE_REQUEST_FAIL = vec3f(0.7,0.2,0.0);
const COLOR_FOR_BRICK_REQUEST_SENT = vec3f(0.3,0.1,0.0);
const COLOR_FOR_BRICK_REQUEST_FAIL = vec3f(0.6,0.0,0.0);
const VHX_PREPASS_STAGE_ID = 1u;
const VHX_RENDER_STAGE_ID = 2u;

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
        + (index.z * BOX_NODE_DIMENSION * BOX_NODE_DIMENSION)
    );
}

struct CubeRayIntersection {
    hit: bool,
    impact_hit: bool,
    impact_distance: f32,
    exit_distance: f32,
}

//crate::spatial::raytracing::Cube::intersect_ray
fn cube_intersect_ray(cube: Cube, ray: ptr<function, Line>,) -> CubeRayIntersection{
    let tmin = max(
        max(
            min(
                (cube.min_position.x - (*ray).origin.x) / (*ray).direction.x,
                (cube.min_position.x + cube.size - (*ray).origin.x) / (*ray).direction.x
            ),
            min(
                (cube.min_position.y - (*ray).origin.y) / (*ray).direction.y,
                (cube.min_position.y + cube.size - (*ray).origin.y) / (*ray).direction.y
            )
        ),
        min(
            (cube.min_position.z - (*ray).origin.z) / (*ray).direction.z,
            (cube.min_position.z + cube.size - (*ray).origin.z) / (*ray).direction.z
        )
    );
    let tmax = min(
        min(
            max(
                (cube.min_position.x - (*ray).origin.x) / (*ray).direction.x,
                (cube.min_position.x + cube.size - (*ray).origin.x) / (*ray).direction.x
            ),
            max(
                (cube.min_position.y - (*ray).origin.y) / (*ray).direction.y,
                (cube.min_position.y + cube.size - (*ray).origin.y) / (*ray).direction.y
            )
        ),
        max(
            (cube.min_position.z - (*ray).origin.z) / (*ray).direction.z,
            (cube.min_position.z + cube.size - (*ray).origin.z) / (*ray).direction.z
        )
    );

    if tmax < 0. || tmin > tmax{
        return CubeRayIntersection(false, false, 0., 0.);
    }

    if tmin < 0.0 {
        return CubeRayIntersection(true, false, 0., tmax);
    }

    return CubeRayIntersection(true, true, tmin, tmax);
}

fn cube_impact_normal(cube: Cube, impact_point: vec3f) -> vec3f{
    var impact_normal = vec3f(0.,0.,0.);
    let mid_to_impact = cube.min_position + vec3f(cube.size / 2.) - impact_point;
    let max_component = max(
        abs(mid_to_impact).x,
        max(abs(mid_to_impact).y, abs(mid_to_impact).z)
    );
    if max_component - abs(mid_to_impact).x < FLOAT_ERROR_TOLERANCE {
        impact_normal.x = -mid_to_impact.x;
    }
    if max_component - abs(mid_to_impact).y < FLOAT_ERROR_TOLERANCE {
        impact_normal.y = -mid_to_impact.y;
    }
    if max_component - abs(mid_to_impact).z < FLOAT_ERROR_TOLERANCE {
        impact_normal.z = -mid_to_impact.z;
    }
    return normalize(impact_normal);
}


//crate::raytracing::NodeStack
const NODE_STACK_SIZE: u32 = 4;
const EMPTY_MARKER: u32 = 0xFFFFFFFFu;

//crate::raytracing::NodeStack::is_empty
fn node_stack_is_empty(node_stack_meta: u32) -> bool {
    return 0 == (node_stack_meta & 0x000000FFu);
}

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
    if 0 == (*node_stack_meta & 0x0000FF00u) { // head index is 0
        *node_stack_meta = (
            // count
            ( ((*node_stack_meta & 0x000000FFu) - 1) )
            // head_index
            | ((NODE_STACK_SIZE - 1) << 8u)
        );
    } else {
        *node_stack_meta = (
            // count
            ( ((*node_stack_meta & 0x000000FFu) - 1) )
            // head_index
            | ( ((
                ( ((*node_stack_meta & 0x0000FF00u) >> 8u) - 1 )
            ) << 8u) & 0x0000FF00u )
        );
    }
    return result;
}

//crate::raytracing::NodeStack::last/last_mut
fn node_stack_last(node_stack_meta: u32) -> u32 { // returns either with index or EMPTY_MARKER
    if 0 == (node_stack_meta & 0x000000FFu) {
        return EMPTY_MARKER;
    }
    return (node_stack_meta & 0x0000FF00u) >> 8u;
}

//crate::boxtree:raytracing::get_dda_scale_factors
fn get_dda_scale_factors(ray: ptr<function, Line>) -> vec3f {
    return vec3f(
        sqrt(
            1.
            + pow((*ray).direction.z / (*ray).direction.x, 2.)
            + pow((*ray).direction.y / (*ray).direction.x, 2.)
        ),
        sqrt(
            pow((*ray).direction.x / (*ray).direction.y, 2.)
            + 1.
            + pow((*ray).direction.z / (*ray).direction.y, 2.)
        ),
        sqrt(
            pow((*ray).direction.x / (*ray).direction.z, 2.)
            + pow((*ray).direction.y / (*ray).direction.z, 2.)
            + 1.
        ),
    );
}

//crate::raytracing::dda_step_to_next_sibling
fn dda_step_to_next_sibling(
    ray: ptr<function, Line>,
    ray_current_point: ptr<function,vec3f>,
    current_bounds: ptr<function, Cube>,
    ray_scale_factors: ptr<function, vec3f>
) -> vec3f {
    let ray_dir_sign = sign((*ray).direction);
    let d = abs(
        ( // step_until_next_axis * ray_scale_factors
            ((*current_bounds).size * max(ray_dir_sign, vec3f(0.)))
            - (ray_dir_sign * (*ray_current_point - (*current_bounds).min_position))
        ) * *ray_scale_factors
    );
    let min_step = min(d.x, min(d.y, d.z));
    var result = vec3f(0., 0., 0.);

    (*ray_current_point) += (*ray).direction * min_step;
    if min_step == d.x {
        result.x = ray_dir_sign.x;
    }
    if min_step == d.y {
        result.y = ray_dir_sign.y;
    }
    if min_step == d.z {
        result.z = ray_dir_sign.z;
    }
    return result;
}

// Unique to this implementation, not adapted from rust code
/// Sets the used bit true for the given node
fn set_node_used(node_key: u32) {
    if 0 != (used_bits[node_key] & 0x01u) {
        // no need to set if already true
        return;
    }

    loop{
        let exchange_result = atomicCompareExchangeWeak(
            &used_bits[node_key], used_bits[node_key], used_bits[node_key] | 0x01u
        );
        if(exchange_result.exchanged || 0 < (exchange_result.old_value & 0x01u)){
            break;
        }
    }
}

// Unique to this implementation, not adapted from rust code
/// Sets the used bit true for the given brick
fn set_brick_used(brick_index: u32) {
    if 0 != ( used_bits[brick_index / 31] & (0x01u << (1u + (brick_index % 31u))) ) {
        // no need to set if already true
        return;
    }

    loop{
        let exchange_result = atomicCompareExchangeWeak(
            &used_bits[brick_index / 31],
            used_bits[brick_index / 31],
            used_bits[brick_index / 31] | (0x01u << (1u + (brick_index % 31u)))
        );
        if(
            exchange_result.exchanged
            || 0 != ( exchange_result.old_value & (0x01u << (1u + (brick_index % 31u))) )
        ){
            break;
        }
    }
}

// Unique to this implementation, not adapted from rust code
/// Requests the child of the given node to be uploaded
fn request_node(node_meta_index: u32, child_sectant: u32) -> bool {
    if 0xFFFFFFFF != node_requests[arrayLength(&node_requests) - 1] {
        // Node requests already full
        return false;
    }
    var request_index = 0u;
    loop{
        let exchange_result = atomicCompareExchangeWeak(
            &node_requests[request_index], EMPTY_MARKER,
            (node_meta_index & 0x00FFFFFFu)|((child_sectant & 0x000000FF) << 24)
        );
        if(
            exchange_result.exchanged 
            ||(
                exchange_result.old_value
                == ((node_meta_index & 0x00FFFFFFu)|((child_sectant & 0x000000FF) << 24))
            )
        ) {
            break;
        }
        request_index += 1u;
        if(request_index >= arrayLength(&node_requests)) {
            return false;
        }
    }
    return true;
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
    return(
        viewport.fov
        * f32(stage_data.output_resolution.x * stage_data.output_resolution.y)
        / (viewport.frustum.x * viewport.frustum.y * sqrt(8.))
    ) - viewport.fov;
}

fn traverse_brick(
    ray: ptr<function, Line>,
    ray_current_point: ptr<function,vec3f>,
    brick_start_index: u32,
    brick_bounds: ptr<function, Cube>,
    ray_scale_factors: ptr<function, vec3f>,
    direction_lut_index: u32,
    max_distance: f32,
) -> BrickHit {
    let dimension = i32(boxtree_meta_data.tree_properties & 0x0000FFFF);
    let voxels_count = i32(arrayLength(&voxels));
    var current_index = clamp(
        vec3i(vec3f(*ray_current_point - (*brick_bounds).min_position) // entry position in brick
        * f32(dimension) / (*brick_bounds).size),
        vec3i(0),
        vec3i(dimension - 1)
    );
    var current_flat_index = (
        i32(brick_start_index) * (dimension * dimension * dimension)
        + ( //crate::spatial::math::flat_projection
            current_index.x
            + (current_index.y * dimension)
            + (current_index.z * dimension * dimension)
        )
    );
    var current_bounds = Cube(
        (
            (*brick_bounds).min_position 
            + vec3f(current_index) * round((*brick_bounds).size / f32(dimension))
        ),
        round((*brick_bounds).size / f32(dimension))
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
        if current_index.x < 0
            || current_index.x >= dimension
            || current_index.y < 0
            || current_index.y >= dimension
            || current_index.z < 0
            || current_index.z >= dimension
        {
            return BrickHit(false, vec3u(), 0);
        }

        // step delta calculated from crate::spatial::math::flat_projection
        // --> e.g. flat_delta_y = flat_projection(0, 1, 0, brick_dim);
        current_flat_index += (
            i32(step.x)
            + i32(step.y) * dimension
            + i32(step.z) * dimension * dimension
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
            && length(*ray_current_point - (*ray).origin) >= max_distance
        {
            return BrickHit(false, vec3u(current_index), u32(current_flat_index));
        }

        step = round(dda_step_to_next_sibling(
            ray, ray_current_point, &current_bounds, ray_scale_factors
        ));
        current_bounds.min_position += step * current_bounds.size;
        current_index += vec3i(step);
    }

    // Technically this line is unreachable
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
    ray_scale_factors: ptr<function, vec3f>,
    direction_lut_index: u32,
    max_distance: f32,
) -> OctreeRayIntersection {
    if(( // node is occupied at target child_sectant, meaning: brick is not empty
        (brick_sectant < 32)
        && (0u != (node_occupied_bits[leaf_node_key * 2] & (0x01u << brick_sectant) ))
    )||(
        (brick_sectant >= 32)
        && (0u != (node_occupied_bits[leaf_node_key * 2 + 1] & (0x01u << (brick_sectant - 32)) ))
    )){
        let brick_descriptor = node_children[
            ((leaf_node_key * BOX_NODE_CHILDREN_COUNT) + brick_sectant)
        ];
        if(0 != (0x80000000 & brick_descriptor)) { // brick is solid
            // Whole brick is solid, ray hits it at first connection
            return OctreeRayIntersection(
                true,
                color_palette[brick_descriptor & 0x0000FFFF], // Albedo is in color_palette, it's not a brick index in this case
                *ray_current_point,
                cube_impact_normal(*brick_bounds, *ray_current_point)
            );
        } else { // brick is parted
            set_brick_used(brick_descriptor & 0x0000FFFF);
            let leaf_brick_hit = traverse_brick(
                ray, ray_current_point,
                brick_descriptor & 0x0000FFFF,
                brick_bounds, ray_scale_factors, direction_lut_index,
                max_distance
            );

            if stage_data.stage == VHX_PREPASS_STAGE_ID {
                if leaf_brick_hit.hit == false && leaf_brick_hit.flat_index != 0 {
                    return OctreeRayIntersection(true, vec4f(0.), *ray_current_point, vec3f(0., 0., 1.));
                }
            }

            if leaf_brick_hit.hit == true {
                let unit_voxel_size = round(
                    (*brick_bounds).size
                    / f32(boxtree_meta_data.tree_properties & 0x0000FFFF)
                );
                return OctreeRayIntersection(
                    true,
                    color_palette[voxels[leaf_brick_hit.flat_index] & 0x0000FFFF],
                    *ray_current_point,
                    cube_impact_normal(
                        Cube(
                            ((*brick_bounds).min_position + (vec3f(leaf_brick_hit.index) * unit_voxel_size)),
                            unit_voxel_size,
                        ),
                        *ray_current_point
                    )
                );
            }
        }
    }
    return OctreeRayIntersection(false, vec4f(0.), *ray_current_point, vec3f(0., 0., 1.));
}

fn probe_MIP(
    ray: ptr<function, Line>,
    ray_current_point: ptr<function,vec3f>,
    node_key: u32,
    node_bounds: ptr<function, Cube>,
    ray_scale_factors: ptr<function, vec3f>,
    direction_lut_index: u32,
    max_distance: f32
) -> OctreeRayIntersection {
    let brick_descriptor = node_mips[node_key];
    if( // there is a valid mip present
        0 != (node_metadata[node_key / 8] & (16 + (node_key % 8))) // node has MIP
        && brick_descriptor != EMPTY_MARKER // which is uploaded
    ) {
        if(0 != (brick_descriptor & 0x80000000)) { // MIP brick is solid
            // Whole brick is solid, ray hits it at first connection
            return OctreeRayIntersection(
                true,
                color_palette[brick_descriptor & 0x0000FFFF], // Albedo is in color_palette, it's not a brick index in this case
                *ray_current_point,
                cube_impact_normal((*node_bounds), *ray_current_point)
            );
        } else { // brick is parted
            set_brick_used(brick_descriptor & 0x0000FFFF);
            var brick_point = *ray_current_point;
            let leaf_brick_hit = traverse_brick(
                ray, &brick_point,
                brick_descriptor & 0x0000FFFF,
                node_bounds, ray_scale_factors, direction_lut_index,
                max_distance
            );
            if leaf_brick_hit.hit == true {
                let unit_voxel_size = round((*node_bounds).size / f32(boxtree_meta_data.tree_properties & 0x0000FFFF));
                return OctreeRayIntersection(
                    true,
                    color_palette[voxels[leaf_brick_hit.flat_index] & 0x0000FFFF],
                    brick_point,
                    cube_impact_normal(
                        Cube(
                            ((*node_bounds).min_position + (vec3f(leaf_brick_hit.index) * unit_voxel_size)),
                            unit_voxel_size,
                        ),
                        brick_point
                    )
                );
            }
        }
    }
    return OctreeRayIntersection(false, vec4f(0.), *ray_current_point, vec3f(0., 0., 1.));
}

// Unique to this implementation, not adapted from rust code
/// Traverses the node to provide information about how the occupied bits of the node
/// and the given ray collides. The higher the number, the closer the hit is.
fn traverse_node_for_ocbits(
    ray: ptr<function, Line>,
    ray_current_point: ptr<function,vec3f>,
    node_key: u32,
    node_bounds: ptr<function, Cube>,
    ray_scale_factors: ptr<function, vec3f>,
) -> f32 {
    var position = vec3f(*ray_current_point - (*node_bounds).min_position);
    var current_index = vec3i(vec3f(
        clamp( (position.x * 4. / (*node_bounds).size), 0.01, 3.99),
        clamp( (position.y * 4. / (*node_bounds).size), 0.01, 3.99),
        clamp( (position.z * 4. / (*node_bounds).size), 0.01, 3.99),
    ));
    var current_bounds = Cube(
        (
            (*node_bounds).min_position
            + vec3f(current_index) * ((*node_bounds).size / 4.)
        ),
        round((*node_bounds).size / 4.)
    );

    var steps_taken = 0u;
    var result = 0.;
    loop {
        if steps_taken > 10 || current_index.x < 0 || current_index.x >= 4
            || current_index.y < 0 || current_index.y >= 4
            || current_index.z < 0 || current_index.z >= 4
        {
            break;
        }

        let bitmap_index = (
            u32(current_index.x)
            + (u32(current_index.y) * BOX_NODE_DIMENSION)
            + (u32(current_index.z) * BOX_NODE_DIMENSION * BOX_NODE_DIMENSION)
        );

        if (
            (
                (bitmap_index < 32)
                && (0u != (node_occupied_bits[node_key * 2]
                            & (0x01u << bitmap_index) ))
            )||(
                (bitmap_index >= 32)
                && (0u != (node_occupied_bits[node_key * 2 + 1]
                            & (0x01u << (bitmap_index - 32)) ))
            )
        ){
            result = 1. - (f32(steps_taken) * 0.25);
            break;
        }

        let step = round(dda_step_to_next_sibling(
            ray, &position, &current_bounds,ray_scale_factors
        ));
        current_bounds.min_position += step * current_bounds.size;
        current_index += vec3i(step);
        steps_taken += 1u;
    }
    return result;
}

fn get_by_ray(ray: ptr<function, Line>, start_distance: f32) -> OctreeRayIntersection {
    var ray_scale_factors = get_dda_scale_factors(ray); // Should be const, but then it can't be passed as ptr
    var tmp_vec = vec3f(1.) + normalize((*ray).direction); // using local variable as temporary storage
    // I shall answer for my crimes later
    let direction_lut_index = ( //crate::spatial::math::hash_direction
        u32(tmp_vec.x >= 1.)
        + u32(tmp_vec.z >= 1.) * 2u
        + u32(tmp_vec.y >= 1.) * 4u
    );
    var max_distance = 2. * f32(boxtree_meta_data.boxtree_size);
    if stage_data.stage == VHX_PREPASS_STAGE_ID {
        max_distance = max_distance_of_reliable_hit();
    }

    var node_stack: array<u32, NODE_STACK_SIZE>;
    var node_stack_meta: u32 = 0;
    var ray_current_point = (*ray).origin + (*ray).direction * start_distance;
    var current_bounds = Cube(vec3(0.), f32(boxtree_meta_data.boxtree_size));
    var target_bounds = current_bounds;
    var current_node_key = BOXTREE_ROOT_NODE_KEY;
    var target_sectant = OOB_SECTANT;
    var missing_data_color = vec3f(0.);
    var mip_level = log2( // log4 isn't available in WGSL
        f32(boxtree_meta_data.boxtree_size / (boxtree_meta_data.tree_properties & 0x0000FFFF))
    ) / 2.;

    let root_intersect = cube_intersect_ray(current_bounds, ray);
    if(root_intersect.hit){
        if( 0. == start_distance && root_intersect.impact_hit == true ) {
            ray_current_point += (*ray).direction * root_intersect.impact_distance;
        }
        target_sectant = hash_region(ray_current_point, current_bounds.size);
    }

    /*// +++ DEBUG +++
    var outer_safety = 0;
    */// --- DEBUG ---
    while target_sectant != OOB_SECTANT && length(ray_current_point - (*ray).origin) < max_distance{
        /*// +++ DEBUG +++
        outer_safety += 1;
        if(f32(outer_safety) > f32(boxtree_meta_data.boxtree_size) * sqrt(3.)) {
            return OctreeRayIntersection(
                true, vec4f(1.,0.,0.,1.), vec3f(0.), vec3f(0., 0., 1.)
            );
        }
        */// --- DEBUG ---
        current_node_key = BOXTREE_ROOT_NODE_KEY;
        current_bounds.size = f32(boxtree_meta_data.boxtree_size);
        current_bounds.min_position = vec3(0.);
        target_bounds.size = round(current_bounds.size / f32(BOX_NODE_DIMENSION));
        target_bounds.min_position = (
            current_bounds.min_position 
            + (SECTANT_OFFSET_REGION_LUT[target_sectant] * current_bounds.size)
        );
        node_stack_push(&node_stack, &node_stack_meta, BOXTREE_ROOT_NODE_KEY);
        /*// +++ DEBUG +++
        var safety = 0;
        */// --- DEBUG ---
        while(
            !node_stack_is_empty(node_stack_meta)
            && length(ray_current_point - (*ray).origin) < max_distance
        ) {
            /*// +++ DEBUG +++
            safety += 1;
            if(f32(safety) > f32(boxtree_meta_data.boxtree_size) * sqrt(30.)) {
                return OctreeRayIntersection(
                    true, vec4f(0.,0.,1.,1.), vec3f(0.), vec3f(0., 0., 1.)
                );
            }
            */// --- DEBUG ---
            if(
                stage_data.stage == VHX_PREPASS_STAGE_ID
                && length(ray_current_point - (*ray).origin) >= max_distance
            ) {
                return OctreeRayIntersection( false, vec4f(0.), ray_current_point, vec3f(0., 0., 1.) );
            }

            if( // In case MIPs are enabled
                (0 != (boxtree_meta_data.tree_properties & 0x00010000))
                &&( // In case current node MIP level is smaller, than the required MIP level
                    mip_level <
                    ( // Note: Aligning to bound borders deemed undesriable artefacts
                        length( // based on ray current travel distance
                            viewport.origin - ( // aligned to nearest cube edges(based on current MIP level)
                                round(ray_current_point / (mip_level * 2.)) * (mip_level * 2.)
                            )
                        )
                        / f32(viewport.frustum.z)
                    )
                )
            ){
                if( // node has MIP which is not uploaded
                    ( 0 != (node_metadata[current_node_key / 8] & (0x01u << (16 + (current_node_key % 8u)))) )
                    && node_mips[current_node_key] == EMPTY_MARKER
                ){
                    request_node(current_node_key, OOB_SECTANT);
                } else {
                    let mip_hit = probe_MIP(
                        ray, &ray_current_point,
                        current_node_key, &current_bounds,
                        &ray_scale_factors, direction_lut_index,
                        max_distance
                    );
                    if true == mip_hit.hit {
                        return mip_hit;
                    }
                }
            }
            var target_child_descriptor = node_children[(current_node_key * BOX_NODE_CHILDREN_COUNT) + target_sectant];
            if(
                // In case node doesn't yet have the target child node uploaded to GPU
                target_sectant != OOB_SECTANT
                && target_child_descriptor == EMPTY_MARKER
                && (( // node is occupied at target sectant
                    (target_sectant < 32)
                    && (0u != (node_occupied_bits[current_node_key * 2] & (0x01u << target_sectant) ))
                )||(
                    (target_sectant >= 32)
                    && (0u != (node_occupied_bits[current_node_key * 2 + 1] & (0x01u << (target_sectant - 32)) ))
                ))
                // Request node only once per ray iteration to prioritize nodes in sight for cache
                && 0 == (missing_data_color.r + missing_data_color.g + missing_data_color.b)
            ){
                // request the node, then display MIP if available; do not request MIP here
                if request_node(current_node_key, target_sectant) {
                    missing_data_color += (
                        COLOR_FOR_NODE_REQUEST_SENT
                        * vec3f(traverse_node_for_ocbits(
                            ray, &ray_current_point,
                            current_node_key, &current_bounds,
                            &ray_scale_factors
                        ))
                    );
                } else {
                    missing_data_color += (
                        COLOR_FOR_NODE_REQUEST_FAIL
                        * vec3f(traverse_node_for_ocbits(
                            ray, &ray_current_point,
                            current_node_key, &current_bounds,
                            &ray_scale_factors
                        ))
                    );
                }
                
                if ( // Probe MIP at data miss, if enabled
                    0 != (boxtree_meta_data.tree_properties & 0x00010000)
                    && stage_data.stage != VHX_PREPASS_STAGE_ID
                ){
                    var mip_hit = probe_MIP(
                        ray, &ray_current_point,
                        current_node_key, &current_bounds,
                        &ray_scale_factors, direction_lut_index,
                        max_distance
                    );
                    if true == mip_hit.hit {
                        mip_hit.albedo -= vec4f(missing_data_color, 0.);
                        return mip_hit;
                    }
                }
            } else if( // node is leaf, its target points inside and is available
                target_sectant != OOB_SECTANT
                && target_child_descriptor != EMPTY_MARKER
                &&( 0 != (node_metadata[current_node_key / 8] & (0x01u << (current_node_key % 8u))) )
            ){
                var hit: OctreeRayIntersection;
                if ( 0 != (node_metadata[current_node_key / 8] & (0x01u << (8 + (current_node_key % 8u)))) ) {
                    // node is uniform
                    hit = probe_brick(
                        ray, &ray_current_point,
                        current_node_key, 0u, &current_bounds,
                        &ray_scale_factors, direction_lut_index,
                        max_distance
                    );
                } else { // node is a non-uniform leaf
                    hit = probe_brick(
                        ray, &ray_current_point,
                        current_node_key, target_sectant, &target_bounds,
                        &ray_scale_factors, direction_lut_index,
                        max_distance
                    );
                }
                if hit.hit == true {
                    hit.albedo -= vec4f(missing_data_color, 0.);

                    /*// +++ DEBUG +++
                    let relative_c_point = hit.impact_point - current_bounds.min_position;
                    if (relative_c_point.x < 5. || relative_c_point.y < 5. || relative_c_point.z < 5.) {
                        hit.albedo.b = 1.;
                    }

                    let bound_size_ratio = f32(target_bounds.size) / f32(boxtree_meta_data.boxtree_size) * 5.;
                    if( // Display current bounds boundaries
                        (abs(ray_current_point.x - target_bounds.min_position.x) < bound_size_ratio)
                        ||(abs(ray_current_point.y - target_bounds.min_position.y) < bound_size_ratio)
                        ||(abs(ray_current_point.z - target_bounds.min_position.z) < bound_size_ratio)
                    ){
                        hit.albedo -= 0.5;
                    }

                    /*if( // Display current bounds center
                        (abs(ray_current_point.x - (current_bounds.min_position.x + (current_bounds.size / 2.))) < bound_size_ratio)
                        ||(abs(ray_current_point.y - (current_bounds.min_position.y + (current_bounds.size / 2.))) < bound_size_ratio)
                        ||(abs(ray_current_point.z - (current_bounds.min_position.z + (current_bounds.size / 2.))) < bound_size_ratio)
                    ){
                        hit.albedo += 0.5;
                    }*/
                    */// --- DEBUG ---
                    return hit;
                }
            }
            if( target_sectant == OOB_SECTANT
                || ( // node is uniform
                    0 != (
                        node_metadata[current_node_key / 8]
                        & (0x01u << (8 + (current_node_key % 8u)))
                    )
                )
                || ( // There is no overlap in node occupancy and ray potential hit area
                    0 == (
                        RAY_TO_NODE_OCCUPANCY_BITMASK_LUT[target_sectant][direction_lut_index * 2]
                        & node_occupied_bits[current_node_key * 2]
                    )
                    && 0 == (
                        RAY_TO_NODE_OCCUPANCY_BITMASK_LUT[target_sectant][direction_lut_index * 2 + 1]
                        & node_occupied_bits[current_node_key * 2 + 1]
                    )
                )
            ) {
                // POP
                mip_level += 1.;
                node_stack_pop(&node_stack, &node_stack_meta);
                target_bounds = current_bounds;
                current_bounds.size *= f32(BOX_NODE_DIMENSION);
                current_bounds.min_position -= current_bounds.min_position % current_bounds.size;
                let ray_point_before_pop = ray_current_point;
                tmp_vec = round(dda_step_to_next_sibling(
                    ray, &ray_current_point, &target_bounds,
                    &ray_scale_factors
                ));
                if(
                    stage_data.stage == VHX_PREPASS_STAGE_ID
                    && length(ray_current_point - (*ray).origin) >= max_distance
                ) {
                    return OctreeRayIntersection( false, vec4f(0.), ray_point_before_pop, vec3f(0., 0., 1.) );
                }
                target_sectant = SECTANT_STEP_RESULT_LUT[
                    hash_region(
                        (
                            target_bounds.min_position
                            + vec3f(target_bounds.size / 2.)
                            - current_bounds.min_position
                        ),
                        current_bounds.size
                    )
                ][u32(tmp_vec.x + 1)][u32(tmp_vec.y + 1)][u32(tmp_vec.z + 1)];
                target_bounds.min_position += tmp_vec * target_bounds.size;
                if(EMPTY_MARKER != node_stack_last(node_stack_meta)){
                    current_node_key = node_stack[node_stack_last(node_stack_meta)];
                }
                continue;
            }
            if ( // If node is not a leaf, occupied at target sectant and target is available
                (0 == (node_metadata[current_node_key / 8u] & (0x01u << (current_node_key % 8u))))
                &&(target_child_descriptor != EMPTY_MARKER)
                &&((
                    (target_sectant < 32)
                    && ( 0u != (node_occupied_bits[current_node_key * 2] & (0x01u << target_sectant)) )
                )||(
                    (target_sectant >= 32)
                    && ( 0u != (node_occupied_bits[current_node_key * 2 + 1] & (0x01u << (target_sectant - 32))) )
                ))
            ) {
                // PUSH
                set_node_used(target_child_descriptor); // Since current_node is internal, no need to filter for "parted" bit
                current_node_key = target_child_descriptor;
                current_bounds = target_bounds;
                target_sectant = hash_region( // child_target_sectant
                    (ray_current_point - target_bounds.min_position),
                    target_bounds.size
                );
                target_bounds.size = round(current_bounds.size / f32(BOX_NODE_DIMENSION));
                target_bounds.min_position = (
                    current_bounds.min_position
                    + (SECTANT_OFFSET_REGION_LUT[target_sectant] * current_bounds.size)
                );
                node_stack_push(&node_stack, &node_stack_meta, target_child_descriptor);
                mip_level -= 1.;
            } else {
                // ADVANCE
                /*// +++ DEBUG +++
                var advance_safety = 0;
                */// --- DEBUG ---
                loop {
                    /*// +++ DEBUG +++
                    advance_safety += 1;
                    if(advance_safety > 16) {
                        return OctreeRayIntersection(
                            true, vec4f(0.,1.,0.,1.), vec3f(0.), vec3f(0., 0., 1.)
                        );
                    }
                    */// --- DEBUG ---
                    tmp_vec = round(dda_step_to_next_sibling(
                        ray, &ray_current_point, &target_bounds,
                        &ray_scale_factors
                    ));
                    target_sectant = SECTANT_STEP_RESULT_LUT[target_sectant]
                                                            [u32(tmp_vec.x + 1)]
                                                            [u32(tmp_vec.y + 1)]
                                                            [u32(tmp_vec.z + 1)];
                    target_bounds.min_position += tmp_vec * target_bounds.size;
                    if OOB_SECTANT != target_sectant {
                        target_child_descriptor = node_children[
                            (current_node_key * BOX_NODE_CHILDREN_COUNT) + target_sectant
                        ];
                        if( // Also request current target if not available
                            target_child_descriptor == EMPTY_MARKER // target child key is invalid
                            && (( // node is occupied at target sectant
                                (target_sectant < 32)
                                && ( 0u != (node_occupied_bits[current_node_key * 2] & (0x01u << target_sectant)) )
                            )||(
                                (target_sectant >= 32)
                                && ( 0u != (node_occupied_bits[current_node_key * 2 + 1] & (0x01u << (target_sectant - 32u))) )
                            ))
                            // Request node only once per ray iteration to prioritize nodes in sight for cache
                            && 0 == (missing_data_color.r + missing_data_color.g + missing_data_color.b)
                        ){
                            if request_node(current_node_key, target_sectant) {
                                missing_data_color += (
                                    COLOR_FOR_NODE_REQUEST_SENT
                                    * vec3f(traverse_node_for_ocbits(
                                        ray, &ray_current_point,
                                        current_node_key, &current_bounds,
                                        &ray_scale_factors
                                    ))
                                );
                            } else {
                                missing_data_color += (
                                    COLOR_FOR_NODE_REQUEST_FAIL
                                    * vec3f(traverse_node_for_ocbits(
                                        ray, &ray_current_point,
                                        current_node_key, &current_bounds,
                                        &ray_scale_factors
                                    ))
                                );
                            }
                        }
                    }
                    if (
                        target_sectant == OOB_SECTANT // target is out of bounds
                        ||( // current node is available
                            target_child_descriptor != EMPTY_MARKER
                            &&(( // and current node is occupied at target sectant
                                (target_sectant < 32)
                                && ( 0u != (node_occupied_bits[current_node_key * 2] & (0x01u << target_sectant)) )
                            )||(
                                (target_sectant >= 32)
                                && ( 0u != (node_occupied_bits[current_node_key * 2 + 1] & (0x01u << (target_sectant - 32u))) )
                            ))
                        )
                        ||( length(ray_current_point - (*ray).origin) >= max_distance )
                    ) {
                        break;
                    }
                } // advance loop
            }
        } // while (node_stack not empty)

        // Push ray current distance a little bit forward to avoid iterating the same paths all over again
        ray_current_point += (*ray).direction * 0.1;
        if(
            length(ray_current_point - (*ray).origin) < max_distance
            && ray_current_point.x < f32(boxtree_meta_data.boxtree_size)
            && ray_current_point.y < f32(boxtree_meta_data.boxtree_size)
            && ray_current_point.z < f32(boxtree_meta_data.boxtree_size)
            && ray_current_point.x > 0.
            && ray_current_point.y > 0.
            && ray_current_point.z > 0.
        ) {
            target_sectant = hash_region(
                ray_current_point,
                f32(boxtree_meta_data.boxtree_size)
            );
        } else {
            target_sectant = OOB_SECTANT;
        }
    } // while (ray inside root bounds)
    return OctreeRayIntersection(false, vec4f(missing_data_color, 1.), ray_current_point, vec3f(0., 0., 1.));
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
    ambient_light_color: vec3f,
    ambient_light_position: vec3f,
    boxtree_size: u32,
    tree_properties: u32,
}

struct Viewport {
    origin: vec3f,
    direction: vec3f,
    frustum: vec3f,
    fov: f32,
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

@group(1) @binding(1)
var<storage, read_write> node_requests: array<atomic<u32>>;

@group(2) @binding(0)
var<uniform> boxtree_meta_data: BoxtreeMetaData;

@group(2) @binding(1)
var<storage, read_write> used_bits: array<atomic<u32>>;

@group(2) @binding(2)
var<storage, read> node_metadata: array<u32>;

@group(2) @binding(3)
var<storage, read> node_children: array<u32>;

@group(2) @binding(4)
var<storage, read> node_mips: array<u32>;

@group(2) @binding(5)
var<storage, read> node_occupied_bits: array<u32>;

@group(2) @binding(6)
var<storage, read> voxels: array<PaletteIndexValues>;

@group(2) @binding(7)
var<storage, read> color_palette: array<vec4f>;


@compute @workgroup_size(8, 8, 1)
fn update(
    @builtin(global_invocation_id) invocation_id: vec3<u32>,
    @builtin(num_workgroups) num_workgroups: vec3<u32>,
) {
    let ray_endpoint =
        (
            viewport.origin
            + (viewport.direction * viewport.fov)
            - (
                normalize(cross(vec3f(0., 1., 0.), viewport.direction))
                * (viewport.frustum.x / 2.)
            )
            - (vec3f(0., 1., 0.) * (viewport.frustum.y / 2.))
        ) // Viewport bottom left
        + (
            normalize(cross(vec3f(0., 1., 0.), viewport.direction))
            * viewport.frustum.x
            * (f32(invocation_id.x) / f32(stage_data.output_resolution.x))
        ) // Viewport right direction
        + (
            vec3f(0., 1., 0.) * viewport.frustum.y
            * (1. - (f32(invocation_id.y) / f32(stage_data.output_resolution.y)))
        ) // Viewport up direction
        ;
    var ray = Line(ray_endpoint, normalize(ray_endpoint - viewport.origin));
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
        let root_intersect = cube_intersect_ray(root_bounds, &ray);
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
        if ray_result.hit == true {
            rgb_result = (
                ray_result.albedo.rgb * (
                    dot(ray_result.impact_normal, vec3f(-0.5,0.5,-0.5)) / 2. + 0.5
                )
            ).rgb;
        } else {
            rgb_result = (rgb_result + ray_result.albedo.rgb) / 2.;
        }

        textureStore(output_texture, vec2u(invocation_id.xy), vec4f(rgb_result, 1.));
    }
}

const SECTANT_OFFSET_REGION_LUT: array<vec3f, 64> = array<vec3f, 64>(
    vec3f(0.0, 0.0, 0.0),vec3f(0.25, 0.0, 0.0),vec3f(0.5, 0.0, 0.0),vec3f(0.75, 0.0, 0.0),
    vec3f(0.0, 0.25, 0.0),vec3f(0.25, 0.25, 0.0),vec3f(0.5, 0.25, 0.0),vec3f(0.75, 0.25, 0.0),
    vec3f(0.0, 0.5, 0.0),vec3f(0.25, 0.5, 0.0),vec3f(0.5, 0.5, 0.0),vec3f(0.75, 0.5, 0.0),
    vec3f(0.0, 0.75, 0.0),vec3f(0.25, 0.75, 0.0),vec3f(0.5, 0.75, 0.0),vec3f(0.75, 0.75, 0.0),

    vec3f(0.0, 0.0, 0.25),vec3f(0.25, 0.0, 0.25),vec3f(0.5, 0.0, 0.25),vec3f(0.75, 0.0, 0.25),
    vec3f(0.0, 0.25, 0.25),vec3f(0.25, 0.25, 0.25),vec3f(0.5, 0.25, 0.25),vec3f(0.75, 0.25, 0.25),
    vec3f(0.0, 0.5, 0.25),vec3f(0.25, 0.5, 0.25),vec3f(0.5, 0.5, 0.25),vec3f(0.75, 0.5, 0.25),
    vec3f(0.0, 0.75, 0.25),vec3f(0.25, 0.75, 0.25),vec3f(0.5, 0.75, 0.25),vec3f(0.75, 0.75, 0.25),

    vec3f(0.0, 0.0, 0.5),vec3f(0.25, 0.0, 0.5),vec3f(0.5, 0.0, 0.5),vec3f(0.75, 0.0, 0.5),
    vec3f(0.0, 0.25, 0.5),vec3f(0.25, 0.25, 0.5),vec3f(0.5, 0.25, 0.5),vec3f(0.75, 0.25, 0.5),
    vec3f(0.0, 0.5, 0.5),vec3f(0.25, 0.5, 0.5),vec3f(0.5, 0.5, 0.5),vec3f(0.75, 0.5, 0.5),
    vec3f(0.0, 0.75, 0.5),vec3f(0.25, 0.75, 0.5),vec3f(0.5, 0.75, 0.5),vec3f(0.75, 0.75, 0.5),

    vec3f(0.0, 0.0, 0.75),vec3f(0.25, 0.0, 0.75),vec3f(0.5, 0.0, 0.75),vec3f(0.75, 0.0, 0.75),
    vec3f(0.0, 0.25, 0.75),vec3f(0.25, 0.25, 0.75),vec3f(0.5, 0.25, 0.75),vec3f(0.75, 0.25, 0.75),
    vec3f(0.0, 0.5, 0.75),vec3f(0.25, 0.5, 0.75),vec3f(0.5, 0.5, 0.75),vec3f(0.75, 0.5, 0.75),
    vec3f(0.0, 0.75, 0.75),vec3f(0.25, 0.75, 0.75),vec3f(0.5, 0.75, 0.75),vec3f(0.75, 0.75, 0.75),
);

const SECTANT_STEP_RESULT_LUT: array<array<array<array<u32, 3>, 3>, 3>,64> = array<array<array<array<u32, 3>, 3>, 3>,64>(
    array<array<array<u32, 3>, 3>, 3>(array<array<u32, 3>, 3>(array<u32, 3>(64,64,64),array<u32, 3>(64,64,64),array<u32, 3>(64,64,64)),array<array<u32, 3>, 3>(array<u32, 3>(64,64,64),array<u32, 3>(64,0,16),array<u32, 3>(64,4,20)),array<array<u32, 3>, 3>(array<u32, 3>(64,64,64),array<u32, 3>(64,1,17),array<u32, 3>(64,5,21))),
    array<array<array<u32, 3>, 3>, 3>(array<array<u32, 3>, 3>(array<u32, 3>(64,64,64),array<u32, 3>(64,0,16),array<u32, 3>(64,4,20)),array<array<u32, 3>, 3>(array<u32, 3>(64,64,64),array<u32, 3>(64,1,17),array<u32, 3>(64,5,21)),array<array<u32, 3>, 3>(array<u32, 3>(64,64,64),array<u32, 3>(64,2,18),array<u32, 3>(64,6,22))),
    array<array<array<u32, 3>, 3>, 3>(array<array<u32, 3>, 3>(array<u32, 3>(64,64,64),array<u32, 3>(64,1,17),array<u32, 3>(64,5,21)),array<array<u32, 3>, 3>(array<u32, 3>(64,64,64),array<u32, 3>(64,2,18),array<u32, 3>(64,6,22)),array<array<u32, 3>, 3>(array<u32, 3>(64,64,64),array<u32, 3>(64,3,19),array<u32, 3>(64,7,23))),
    array<array<array<u32, 3>, 3>, 3>(array<array<u32, 3>, 3>(array<u32, 3>(64,64,64),array<u32, 3>(64,2,18),array<u32, 3>(64,6,22)),array<array<u32, 3>, 3>(array<u32, 3>(64,64,64),array<u32, 3>(64,3,19),array<u32, 3>(64,7,23)),array<array<u32, 3>, 3>(array<u32, 3>(64,64,64),array<u32, 3>(64,64,64),array<u32, 3>(64,64,64))),
    array<array<array<u32, 3>, 3>, 3>(array<array<u32, 3>, 3>(array<u32, 3>(64,64,64),array<u32, 3>(64,64,64),array<u32, 3>(64,64,64)),array<array<u32, 3>, 3>(array<u32, 3>(64,0,16),array<u32, 3>(64,4,20),array<u32, 3>(64,8,24)),array<array<u32, 3>, 3>(array<u32, 3>(64,1,17),array<u32, 3>(64,5,21),array<u32, 3>(64,9,25))),
    array<array<array<u32, 3>, 3>, 3>(array<array<u32, 3>, 3>(array<u32, 3>(64,0,16),array<u32, 3>(64,4,20),array<u32, 3>(64,8,24)),array<array<u32, 3>, 3>(array<u32, 3>(64,1,17),array<u32, 3>(64,5,21),array<u32, 3>(64,9,25)),array<array<u32, 3>, 3>(array<u32, 3>(64,2,18),array<u32, 3>(64,6,22),array<u32, 3>(64,10,26))),
    array<array<array<u32, 3>, 3>, 3>(array<array<u32, 3>, 3>(array<u32, 3>(64,1,17),array<u32, 3>(64,5,21),array<u32, 3>(64,9,25)),array<array<u32, 3>, 3>(array<u32, 3>(64,2,18),array<u32, 3>(64,6,22),array<u32, 3>(64,10,26)),array<array<u32, 3>, 3>(array<u32, 3>(64,3,19),array<u32, 3>(64,7,23),array<u32, 3>(64,11,27))),
    array<array<array<u32, 3>, 3>, 3>(array<array<u32, 3>, 3>(array<u32, 3>(64,2,18),array<u32, 3>(64,6,22),array<u32, 3>(64,10,26)),array<array<u32, 3>, 3>(array<u32, 3>(64,3,19),array<u32, 3>(64,7,23),array<u32, 3>(64,11,27)),array<array<u32, 3>, 3>(array<u32, 3>(64,64,64),array<u32, 3>(64,64,64),array<u32, 3>(64,64,64))),
    array<array<array<u32, 3>, 3>, 3>(array<array<u32, 3>, 3>(array<u32, 3>(64,64,64),array<u32, 3>(64,64,64),array<u32, 3>(64,64,64)),array<array<u32, 3>, 3>(array<u32, 3>(64,4,20),array<u32, 3>(64,8,24),array<u32, 3>(64,12,28)),array<array<u32, 3>, 3>(array<u32, 3>(64,5,21),array<u32, 3>(64,9,25),array<u32, 3>(64,13,29))),
    array<array<array<u32, 3>, 3>, 3>(array<array<u32, 3>, 3>(array<u32, 3>(64,4,20),array<u32, 3>(64,8,24),array<u32, 3>(64,12,28)),array<array<u32, 3>, 3>(array<u32, 3>(64,5,21),array<u32, 3>(64,9,25),array<u32, 3>(64,13,29)),array<array<u32, 3>, 3>(array<u32, 3>(64,6,22),array<u32, 3>(64,10,26),array<u32, 3>(64,14,30))),
    array<array<array<u32, 3>, 3>, 3>(array<array<u32, 3>, 3>(array<u32, 3>(64,5,21),array<u32, 3>(64,9,25),array<u32, 3>(64,13,29)),array<array<u32, 3>, 3>(array<u32, 3>(64,6,22),array<u32, 3>(64,10,26),array<u32, 3>(64,14,30)),array<array<u32, 3>, 3>(array<u32, 3>(64,7,23),array<u32, 3>(64,11,27),array<u32, 3>(64,15,31))),
    array<array<array<u32, 3>, 3>, 3>(array<array<u32, 3>, 3>(array<u32, 3>(64,6,22),array<u32, 3>(64,10,26),array<u32, 3>(64,14,30)),array<array<u32, 3>, 3>(array<u32, 3>(64,7,23),array<u32, 3>(64,11,27),array<u32, 3>(64,15,31)),array<array<u32, 3>, 3>(array<u32, 3>(64,64,64),array<u32, 3>(64,64,64),array<u32, 3>(64,64,64))),
    array<array<array<u32, 3>, 3>, 3>(array<array<u32, 3>, 3>(array<u32, 3>(64,64,64),array<u32, 3>(64,64,64),array<u32, 3>(64,64,64)),array<array<u32, 3>, 3>(array<u32, 3>(64,8,24),array<u32, 3>(64,12,28),array<u32, 3>(64,64,64)),array<array<u32, 3>, 3>(array<u32, 3>(64,9,25),array<u32, 3>(64,13,29),array<u32, 3>(64,64,64))),
    array<array<array<u32, 3>, 3>, 3>(array<array<u32, 3>, 3>(array<u32, 3>(64,8,24),array<u32, 3>(64,12,28),array<u32, 3>(64,64,64)),array<array<u32, 3>, 3>(array<u32, 3>(64,9,25),array<u32, 3>(64,13,29),array<u32, 3>(64,64,64)),array<array<u32, 3>, 3>(array<u32, 3>(64,10,26),array<u32, 3>(64,14,30),array<u32, 3>(64,64,64))),
    array<array<array<u32, 3>, 3>, 3>(array<array<u32, 3>, 3>(array<u32, 3>(64,9,25),array<u32, 3>(64,13,29),array<u32, 3>(64,64,64)),array<array<u32, 3>, 3>(array<u32, 3>(64,10,26),array<u32, 3>(64,14,30),array<u32, 3>(64,64,64)),array<array<u32, 3>, 3>(array<u32, 3>(64,11,27),array<u32, 3>(64,15,31),array<u32, 3>(64,64,64))),
    array<array<array<u32, 3>, 3>, 3>(array<array<u32, 3>, 3>(array<u32, 3>(64,10,26),array<u32, 3>(64,14,30),array<u32, 3>(64,64,64)),array<array<u32, 3>, 3>(array<u32, 3>(64,11,27),array<u32, 3>(64,15,31),array<u32, 3>(64,64,64)),array<array<u32, 3>, 3>(array<u32, 3>(64,64,64),array<u32, 3>(64,64,64),array<u32, 3>(64,64,64))),
    array<array<array<u32, 3>, 3>, 3>(array<array<u32, 3>, 3>(array<u32, 3>(64,64,64),array<u32, 3>(64,64,64),array<u32, 3>(64,64,64)),array<array<u32, 3>, 3>(array<u32, 3>(64,64,64),array<u32, 3>(0,16,32),array<u32, 3>(4,20,36)),array<array<u32, 3>, 3>(array<u32, 3>(64,64,64),array<u32, 3>(1,17,33),array<u32, 3>(5,21,37))),
    array<array<array<u32, 3>, 3>, 3>(array<array<u32, 3>, 3>(array<u32, 3>(64,64,64),array<u32, 3>(0,16,32),array<u32, 3>(4,20,36)),array<array<u32, 3>, 3>(array<u32, 3>(64,64,64),array<u32, 3>(1,17,33),array<u32, 3>(5,21,37)),array<array<u32, 3>, 3>(array<u32, 3>(64,64,64),array<u32, 3>(2,18,34),array<u32, 3>(6,22,38))),
    array<array<array<u32, 3>, 3>, 3>(array<array<u32, 3>, 3>(array<u32, 3>(64,64,64),array<u32, 3>(1,17,33),array<u32, 3>(5,21,37)),array<array<u32, 3>, 3>(array<u32, 3>(64,64,64),array<u32, 3>(2,18,34),array<u32, 3>(6,22,38)),array<array<u32, 3>, 3>(array<u32, 3>(64,64,64),array<u32, 3>(3,19,35),array<u32, 3>(7,23,39))),
    array<array<array<u32, 3>, 3>, 3>(array<array<u32, 3>, 3>(array<u32, 3>(64,64,64),array<u32, 3>(2,18,34),array<u32, 3>(6,22,38)),array<array<u32, 3>, 3>(array<u32, 3>(64,64,64),array<u32, 3>(3,19,35),array<u32, 3>(7,23,39)),array<array<u32, 3>, 3>(array<u32, 3>(64,64,64),array<u32, 3>(64,64,64),array<u32, 3>(64,64,64))),
    array<array<array<u32, 3>, 3>, 3>(array<array<u32, 3>, 3>(array<u32, 3>(64,64,64),array<u32, 3>(64,64,64),array<u32, 3>(64,64,64)),array<array<u32, 3>, 3>(array<u32, 3>(0,16,32),array<u32, 3>(4,20,36),array<u32, 3>(8,24,40)),array<array<u32, 3>, 3>(array<u32, 3>(1,17,33),array<u32, 3>(5,21,37),array<u32, 3>(9,25,41))),
    array<array<array<u32, 3>, 3>, 3>(array<array<u32, 3>, 3>(array<u32, 3>(0,16,32),array<u32, 3>(4,20,36),array<u32, 3>(8,24,40)),array<array<u32, 3>, 3>(array<u32, 3>(1,17,33),array<u32, 3>(5,21,37),array<u32, 3>(9,25,41)),array<array<u32, 3>, 3>(array<u32, 3>(2,18,34),array<u32, 3>(6,22,38),array<u32, 3>(10,26,42))),
    array<array<array<u32, 3>, 3>, 3>(array<array<u32, 3>, 3>(array<u32, 3>(1,17,33),array<u32, 3>(5,21,37),array<u32, 3>(9,25,41)),array<array<u32, 3>, 3>(array<u32, 3>(2,18,34),array<u32, 3>(6,22,38),array<u32, 3>(10,26,42)),array<array<u32, 3>, 3>(array<u32, 3>(3,19,35),array<u32, 3>(7,23,39),array<u32, 3>(11,27,43))),
    array<array<array<u32, 3>, 3>, 3>(array<array<u32, 3>, 3>(array<u32, 3>(2,18,34),array<u32, 3>(6,22,38),array<u32, 3>(10,26,42)),array<array<u32, 3>, 3>(array<u32, 3>(3,19,35),array<u32, 3>(7,23,39),array<u32, 3>(11,27,43)),array<array<u32, 3>, 3>(array<u32, 3>(64,64,64),array<u32, 3>(64,64,64),array<u32, 3>(64,64,64))),
    array<array<array<u32, 3>, 3>, 3>(array<array<u32, 3>, 3>(array<u32, 3>(64,64,64),array<u32, 3>(64,64,64),array<u32, 3>(64,64,64)),array<array<u32, 3>, 3>(array<u32, 3>(4,20,36),array<u32, 3>(8,24,40),array<u32, 3>(12,28,44)),array<array<u32, 3>, 3>(array<u32, 3>(5,21,37),array<u32, 3>(9,25,41),array<u32, 3>(13,29,45))),
    array<array<array<u32, 3>, 3>, 3>(array<array<u32, 3>, 3>(array<u32, 3>(4,20,36),array<u32, 3>(8,24,40),array<u32, 3>(12,28,44)),array<array<u32, 3>, 3>(array<u32, 3>(5,21,37),array<u32, 3>(9,25,41),array<u32, 3>(13,29,45)),array<array<u32, 3>, 3>(array<u32, 3>(6,22,38),array<u32, 3>(10,26,42),array<u32, 3>(14,30,46))),
    array<array<array<u32, 3>, 3>, 3>(array<array<u32, 3>, 3>(array<u32, 3>(5,21,37),array<u32, 3>(9,25,41),array<u32, 3>(13,29,45)),array<array<u32, 3>, 3>(array<u32, 3>(6,22,38),array<u32, 3>(10,26,42),array<u32, 3>(14,30,46)),array<array<u32, 3>, 3>(array<u32, 3>(7,23,39),array<u32, 3>(11,27,43),array<u32, 3>(15,31,47))),
    array<array<array<u32, 3>, 3>, 3>(array<array<u32, 3>, 3>(array<u32, 3>(6,22,38),array<u32, 3>(10,26,42),array<u32, 3>(14,30,46)),array<array<u32, 3>, 3>(array<u32, 3>(7,23,39),array<u32, 3>(11,27,43),array<u32, 3>(15,31,47)),array<array<u32, 3>, 3>(array<u32, 3>(64,64,64),array<u32, 3>(64,64,64),array<u32, 3>(64,64,64))),
    array<array<array<u32, 3>, 3>, 3>(array<array<u32, 3>, 3>(array<u32, 3>(64,64,64),array<u32, 3>(64,64,64),array<u32, 3>(64,64,64)),array<array<u32, 3>, 3>(array<u32, 3>(8,24,40),array<u32, 3>(12,28,44),array<u32, 3>(64,64,64)),array<array<u32, 3>, 3>(array<u32, 3>(9,25,41),array<u32, 3>(13,29,45),array<u32, 3>(64,64,64))),
    array<array<array<u32, 3>, 3>, 3>(array<array<u32, 3>, 3>(array<u32, 3>(8,24,40),array<u32, 3>(12,28,44),array<u32, 3>(64,64,64)),array<array<u32, 3>, 3>(array<u32, 3>(9,25,41),array<u32, 3>(13,29,45),array<u32, 3>(64,64,64)),array<array<u32, 3>, 3>(array<u32, 3>(10,26,42),array<u32, 3>(14,30,46),array<u32, 3>(64,64,64))),
    array<array<array<u32, 3>, 3>, 3>(array<array<u32, 3>, 3>(array<u32, 3>(9,25,41),array<u32, 3>(13,29,45),array<u32, 3>(64,64,64)),array<array<u32, 3>, 3>(array<u32, 3>(10,26,42),array<u32, 3>(14,30,46),array<u32, 3>(64,64,64)),array<array<u32, 3>, 3>(array<u32, 3>(11,27,43),array<u32, 3>(15,31,47),array<u32, 3>(64,64,64))),
    array<array<array<u32, 3>, 3>, 3>(array<array<u32, 3>, 3>(array<u32, 3>(10,26,42),array<u32, 3>(14,30,46),array<u32, 3>(64,64,64)),array<array<u32, 3>, 3>(array<u32, 3>(11,27,43),array<u32, 3>(15,31,47),array<u32, 3>(64,64,64)),array<array<u32, 3>, 3>(array<u32, 3>(64,64,64),array<u32, 3>(64,64,64),array<u32, 3>(64,64,64))),
    array<array<array<u32, 3>, 3>, 3>(array<array<u32, 3>, 3>(array<u32, 3>(64,64,64),array<u32, 3>(64,64,64),array<u32, 3>(64,64,64)),array<array<u32, 3>, 3>(array<u32, 3>(64,64,64),array<u32, 3>(16,32,48),array<u32, 3>(20,36,52)),array<array<u32, 3>, 3>(array<u32, 3>(64,64,64),array<u32, 3>(17,33,49),array<u32, 3>(21,37,53))),
    array<array<array<u32, 3>, 3>, 3>(array<array<u32, 3>, 3>(array<u32, 3>(64,64,64),array<u32, 3>(16,32,48),array<u32, 3>(20,36,52)),array<array<u32, 3>, 3>(array<u32, 3>(64,64,64),array<u32, 3>(17,33,49),array<u32, 3>(21,37,53)),array<array<u32, 3>, 3>(array<u32, 3>(64,64,64),array<u32, 3>(18,34,50),array<u32, 3>(22,38,54))),
    array<array<array<u32, 3>, 3>, 3>(array<array<u32, 3>, 3>(array<u32, 3>(64,64,64),array<u32, 3>(17,33,49),array<u32, 3>(21,37,53)),array<array<u32, 3>, 3>(array<u32, 3>(64,64,64),array<u32, 3>(18,34,50),array<u32, 3>(22,38,54)),array<array<u32, 3>, 3>(array<u32, 3>(64,64,64),array<u32, 3>(19,35,51),array<u32, 3>(23,39,55))),
    array<array<array<u32, 3>, 3>, 3>(array<array<u32, 3>, 3>(array<u32, 3>(64,64,64),array<u32, 3>(18,34,50),array<u32, 3>(22,38,54)),array<array<u32, 3>, 3>(array<u32, 3>(64,64,64),array<u32, 3>(19,35,51),array<u32, 3>(23,39,55)),array<array<u32, 3>, 3>(array<u32, 3>(64,64,64),array<u32, 3>(64,64,64),array<u32, 3>(64,64,64))),
    array<array<array<u32, 3>, 3>, 3>(array<array<u32, 3>, 3>(array<u32, 3>(64,64,64),array<u32, 3>(64,64,64),array<u32, 3>(64,64,64)),array<array<u32, 3>, 3>(array<u32, 3>(16,32,48),array<u32, 3>(20,36,52),array<u32, 3>(24,40,56)),array<array<u32, 3>, 3>(array<u32, 3>(17,33,49),array<u32, 3>(21,37,53),array<u32, 3>(25,41,57))),
    array<array<array<u32, 3>, 3>, 3>(array<array<u32, 3>, 3>(array<u32, 3>(16,32,48),array<u32, 3>(20,36,52),array<u32, 3>(24,40,56)),array<array<u32, 3>, 3>(array<u32, 3>(17,33,49),array<u32, 3>(21,37,53),array<u32, 3>(25,41,57)),array<array<u32, 3>, 3>(array<u32, 3>(18,34,50),array<u32, 3>(22,38,54),array<u32, 3>(26,42,58))),
    array<array<array<u32, 3>, 3>, 3>(array<array<u32, 3>, 3>(array<u32, 3>(17,33,49),array<u32, 3>(21,37,53),array<u32, 3>(25,41,57)),array<array<u32, 3>, 3>(array<u32, 3>(18,34,50),array<u32, 3>(22,38,54),array<u32, 3>(26,42,58)),array<array<u32, 3>, 3>(array<u32, 3>(19,35,51),array<u32, 3>(23,39,55),array<u32, 3>(27,43,59))),
    array<array<array<u32, 3>, 3>, 3>(array<array<u32, 3>, 3>(array<u32, 3>(18,34,50),array<u32, 3>(22,38,54),array<u32, 3>(26,42,58)),array<array<u32, 3>, 3>(array<u32, 3>(19,35,51),array<u32, 3>(23,39,55),array<u32, 3>(27,43,59)),array<array<u32, 3>, 3>(array<u32, 3>(64,64,64),array<u32, 3>(64,64,64),array<u32, 3>(64,64,64))),
    array<array<array<u32, 3>, 3>, 3>(array<array<u32, 3>, 3>(array<u32, 3>(64,64,64),array<u32, 3>(64,64,64),array<u32, 3>(64,64,64)),array<array<u32, 3>, 3>(array<u32, 3>(20,36,52),array<u32, 3>(24,40,56),array<u32, 3>(28,44,60)),array<array<u32, 3>, 3>(array<u32, 3>(21,37,53),array<u32, 3>(25,41,57),array<u32, 3>(29,45,61))),
    array<array<array<u32, 3>, 3>, 3>(array<array<u32, 3>, 3>(array<u32, 3>(20,36,52),array<u32, 3>(24,40,56),array<u32, 3>(28,44,60)),array<array<u32, 3>, 3>(array<u32, 3>(21,37,53),array<u32, 3>(25,41,57),array<u32, 3>(29,45,61)),array<array<u32, 3>, 3>(array<u32, 3>(22,38,54),array<u32, 3>(26,42,58),array<u32, 3>(30,46,62))),
    array<array<array<u32, 3>, 3>, 3>(array<array<u32, 3>, 3>(array<u32, 3>(21,37,53),array<u32, 3>(25,41,57),array<u32, 3>(29,45,61)),array<array<u32, 3>, 3>(array<u32, 3>(22,38,54),array<u32, 3>(26,42,58),array<u32, 3>(30,46,62)),array<array<u32, 3>, 3>(array<u32, 3>(23,39,55),array<u32, 3>(27,43,59),array<u32, 3>(31,47,63))),
    array<array<array<u32, 3>, 3>, 3>(array<array<u32, 3>, 3>(array<u32, 3>(22,38,54),array<u32, 3>(26,42,58),array<u32, 3>(30,46,62)),array<array<u32, 3>, 3>(array<u32, 3>(23,39,55),array<u32, 3>(27,43,59),array<u32, 3>(31,47,63)),array<array<u32, 3>, 3>(array<u32, 3>(64,64,64),array<u32, 3>(64,64,64),array<u32, 3>(64,64,64))),
    array<array<array<u32, 3>, 3>, 3>(array<array<u32, 3>, 3>(array<u32, 3>(64,64,64),array<u32, 3>(64,64,64),array<u32, 3>(64,64,64)),array<array<u32, 3>, 3>(array<u32, 3>(24,40,56),array<u32, 3>(28,44,60),array<u32, 3>(64,64,64)),array<array<u32, 3>, 3>(array<u32, 3>(25,41,57),array<u32, 3>(29,45,61),array<u32, 3>(64,64,64))),
    array<array<array<u32, 3>, 3>, 3>(array<array<u32, 3>, 3>(array<u32, 3>(24,40,56),array<u32, 3>(28,44,60),array<u32, 3>(64,64,64)),array<array<u32, 3>, 3>(array<u32, 3>(25,41,57),array<u32, 3>(29,45,61),array<u32, 3>(64,64,64)),array<array<u32, 3>, 3>(array<u32, 3>(26,42,58),array<u32, 3>(30,46,62),array<u32, 3>(64,64,64))),
    array<array<array<u32, 3>, 3>, 3>(array<array<u32, 3>, 3>(array<u32, 3>(25,41,57),array<u32, 3>(29,45,61),array<u32, 3>(64,64,64)),array<array<u32, 3>, 3>(array<u32, 3>(26,42,58),array<u32, 3>(30,46,62),array<u32, 3>(64,64,64)),array<array<u32, 3>, 3>(array<u32, 3>(27,43,59),array<u32, 3>(31,47,63),array<u32, 3>(64,64,64))),
    array<array<array<u32, 3>, 3>, 3>(array<array<u32, 3>, 3>(array<u32, 3>(26,42,58),array<u32, 3>(30,46,62),array<u32, 3>(64,64,64)),array<array<u32, 3>, 3>(array<u32, 3>(27,43,59),array<u32, 3>(31,47,63),array<u32, 3>(64,64,64)),array<array<u32, 3>, 3>(array<u32, 3>(64,64,64),array<u32, 3>(64,64,64),array<u32, 3>(64,64,64))),
    array<array<array<u32, 3>, 3>, 3>(array<array<u32, 3>, 3>(array<u32, 3>(64,64,64),array<u32, 3>(64,64,64),array<u32, 3>(64,64,64)),array<array<u32, 3>, 3>(array<u32, 3>(64,64,64),array<u32, 3>(32,48,64),array<u32, 3>(36,52,64)),array<array<u32, 3>, 3>(array<u32, 3>(64,64,64),array<u32, 3>(33,49,64),array<u32, 3>(37,53,64))),
    array<array<array<u32, 3>, 3>, 3>(array<array<u32, 3>, 3>(array<u32, 3>(64,64,64),array<u32, 3>(32,48,64),array<u32, 3>(36,52,64)),array<array<u32, 3>, 3>(array<u32, 3>(64,64,64),array<u32, 3>(33,49,64),array<u32, 3>(37,53,64)),array<array<u32, 3>, 3>(array<u32, 3>(64,64,64),array<u32, 3>(34,50,64),array<u32, 3>(38,54,64))),
    array<array<array<u32, 3>, 3>, 3>(array<array<u32, 3>, 3>(array<u32, 3>(64,64,64),array<u32, 3>(33,49,64),array<u32, 3>(37,53,64)),array<array<u32, 3>, 3>(array<u32, 3>(64,64,64),array<u32, 3>(34,50,64),array<u32, 3>(38,54,64)),array<array<u32, 3>, 3>(array<u32, 3>(64,64,64),array<u32, 3>(35,51,64),array<u32, 3>(39,55,64))),
    array<array<array<u32, 3>, 3>, 3>(array<array<u32, 3>, 3>(array<u32, 3>(64,64,64),array<u32, 3>(34,50,64),array<u32, 3>(38,54,64)),array<array<u32, 3>, 3>(array<u32, 3>(64,64,64),array<u32, 3>(35,51,64),array<u32, 3>(39,55,64)),array<array<u32, 3>, 3>(array<u32, 3>(64,64,64),array<u32, 3>(64,64,64),array<u32, 3>(64,64,64))),
    array<array<array<u32, 3>, 3>, 3>(array<array<u32, 3>, 3>(array<u32, 3>(64,64,64),array<u32, 3>(64,64,64),array<u32, 3>(64,64,64)),array<array<u32, 3>, 3>(array<u32, 3>(32,48,64),array<u32, 3>(36,52,64),array<u32, 3>(40,56,64)),array<array<u32, 3>, 3>(array<u32, 3>(33,49,64),array<u32, 3>(37,53,64),array<u32, 3>(41,57,64))),
    array<array<array<u32, 3>, 3>, 3>(array<array<u32, 3>, 3>(array<u32, 3>(32,48,64),array<u32, 3>(36,52,64),array<u32, 3>(40,56,64)),array<array<u32, 3>, 3>(array<u32, 3>(33,49,64),array<u32, 3>(37,53,64),array<u32, 3>(41,57,64)),array<array<u32, 3>, 3>(array<u32, 3>(34,50,64),array<u32, 3>(38,54,64),array<u32, 3>(42,58,64))),
    array<array<array<u32, 3>, 3>, 3>(array<array<u32, 3>, 3>(array<u32, 3>(33,49,64),array<u32, 3>(37,53,64),array<u32, 3>(41,57,64)),array<array<u32, 3>, 3>(array<u32, 3>(34,50,64),array<u32, 3>(38,54,64),array<u32, 3>(42,58,64)),array<array<u32, 3>, 3>(array<u32, 3>(35,51,64),array<u32, 3>(39,55,64),array<u32, 3>(43,59,64))),
    array<array<array<u32, 3>, 3>, 3>(array<array<u32, 3>, 3>(array<u32, 3>(34,50,64),array<u32, 3>(38,54,64),array<u32, 3>(42,58,64)),array<array<u32, 3>, 3>(array<u32, 3>(35,51,64),array<u32, 3>(39,55,64),array<u32, 3>(43,59,64)),array<array<u32, 3>, 3>(array<u32, 3>(64,64,64),array<u32, 3>(64,64,64),array<u32, 3>(64,64,64))),
    array<array<array<u32, 3>, 3>, 3>(array<array<u32, 3>, 3>(array<u32, 3>(64,64,64),array<u32, 3>(64,64,64),array<u32, 3>(64,64,64)),array<array<u32, 3>, 3>(array<u32, 3>(36,52,64),array<u32, 3>(40,56,64),array<u32, 3>(44,60,64)),array<array<u32, 3>, 3>(array<u32, 3>(37,53,64),array<u32, 3>(41,57,64),array<u32, 3>(45,61,64))),
    array<array<array<u32, 3>, 3>, 3>(array<array<u32, 3>, 3>(array<u32, 3>(36,52,64),array<u32, 3>(40,56,64),array<u32, 3>(44,60,64)),array<array<u32, 3>, 3>(array<u32, 3>(37,53,64),array<u32, 3>(41,57,64),array<u32, 3>(45,61,64)),array<array<u32, 3>, 3>(array<u32, 3>(38,54,64),array<u32, 3>(42,58,64),array<u32, 3>(46,62,64))),
    array<array<array<u32, 3>, 3>, 3>(array<array<u32, 3>, 3>(array<u32, 3>(37,53,64),array<u32, 3>(41,57,64),array<u32, 3>(45,61,64)),array<array<u32, 3>, 3>(array<u32, 3>(38,54,64),array<u32, 3>(42,58,64),array<u32, 3>(46,62,64)),array<array<u32, 3>, 3>(array<u32, 3>(39,55,64),array<u32, 3>(43,59,64),array<u32, 3>(47,63,64))),
    array<array<array<u32, 3>, 3>, 3>(array<array<u32, 3>, 3>(array<u32, 3>(38,54,64),array<u32, 3>(42,58,64),array<u32, 3>(46,62,64)),array<array<u32, 3>, 3>(array<u32, 3>(39,55,64),array<u32, 3>(43,59,64),array<u32, 3>(47,63,64)),array<array<u32, 3>, 3>(array<u32, 3>(64,64,64),array<u32, 3>(64,64,64),array<u32, 3>(64,64,64))),
    array<array<array<u32, 3>, 3>, 3>(array<array<u32, 3>, 3>(array<u32, 3>(64,64,64),array<u32, 3>(64,64,64),array<u32, 3>(64,64,64)),array<array<u32, 3>, 3>(array<u32, 3>(40,56,64),array<u32, 3>(44,60,64),array<u32, 3>(64,64,64)),array<array<u32, 3>, 3>(array<u32, 3>(41,57,64),array<u32, 3>(45,61,64),array<u32, 3>(64,64,64))),
    array<array<array<u32, 3>, 3>, 3>(array<array<u32, 3>, 3>(array<u32, 3>(40,56,64),array<u32, 3>(44,60,64),array<u32, 3>(64,64,64)),array<array<u32, 3>, 3>(array<u32, 3>(41,57,64),array<u32, 3>(45,61,64),array<u32, 3>(64,64,64)),array<array<u32, 3>, 3>(array<u32, 3>(42,58,64),array<u32, 3>(46,62,64),array<u32, 3>(64,64,64))),
    array<array<array<u32, 3>, 3>, 3>(array<array<u32, 3>, 3>(array<u32, 3>(41,57,64),array<u32, 3>(45,61,64),array<u32, 3>(64,64,64)),array<array<u32, 3>, 3>(array<u32, 3>(42,58,64),array<u32, 3>(46,62,64),array<u32, 3>(64,64,64)),array<array<u32, 3>, 3>(array<u32, 3>(43,59,64),array<u32, 3>(47,63,64),array<u32, 3>(64,64,64))),
    array<array<array<u32, 3>, 3>, 3>(array<array<u32, 3>, 3>(array<u32, 3>(42,58,64),array<u32, 3>(46,62,64),array<u32, 3>(64,64,64)),array<array<u32, 3>, 3>(array<u32, 3>(43,59,64),array<u32, 3>(47,63,64),array<u32, 3>(64,64,64)),array<array<u32, 3>, 3>(array<u32, 3>(64,64,64),array<u32, 3>(64,64,64),array<u32, 3>(64,64,64)))
);

const RAY_TO_NODE_OCCUPANCY_BITMASK_LUT: array<array<u32, 16>, 64> = array<array<u32, 16>, 64>(
    array<u32, 16>(1,0,15,0,65537,65537,983055,983055,4369,0,65535,0,286331153,286331153,4294967295,4294967295,),
    array<u32, 16>(3,0,14,0,196611,196611,917518,917518,13107,0,61166,0,858993459,858993459,4008636142,4008636142,),
    array<u32, 16>(7,0,12,0,458759,458759,786444,786444,30583,0,52428,0,2004318071,2004318071,3435973836,3435973836,),
    array<u32, 16>(15,0,8,0,983055,983055,524296,524296,65535,0,34952,0,4294967295,4294967295,2290649224,2290649224,),
    array<u32, 16>(17,0,255,0,1114129,1114129,16711935,16711935,4368,0,65520,0,286265616,286265616,4293984240,4293984240,),
    array<u32, 16>(51,0,238,0,3342387,3342387,15597806,15597806,13104,0,61152,0,858796848,858796848,4007718624,4007718624,),
    array<u32, 16>(119,0,204,0,7798903,7798903,13369548,13369548,30576,0,52416,0,2003859312,2003859312,3435187392,3435187392,),
    array<u32, 16>(255,0,136,0,16711935,16711935,8913032,8913032,65520,0,34944,0,4293984240,4293984240,2290124928,2290124928,),
    array<u32, 16>(273,0,4095,0,17891601,17891601,268374015,268374015,4352,0,65280,0,285217024,285217024,4278255360,4278255360,),
    array<u32, 16>(819,0,3822,0,53674803,53674803,250482414,250482414,13056,0,60928,0,855651072,855651072,3993038336,3993038336,),
    array<u32, 16>(1911,0,3276,0,125241207,125241207,214699212,214699212,30464,0,52224,0,1996519168,1996519168,3422604288,3422604288,),
    array<u32, 16>(4095,0,2184,0,268374015,268374015,143132808,143132808,65280,0,34816,0,4278255360,4278255360,2281736192,2281736192,),
    array<u32, 16>(4369,0,65535,0,286331153,286331153,4294967295,4294967295,4096,0,61440,0,268439552,268439552,4026593280,4026593280,),
    array<u32, 16>(13107,0,61166,0,858993459,858993459,4008636142,4008636142,12288,0,57344,0,805318656,805318656,3758153728,3758153728,),
    array<u32, 16>(30583,0,52428,0,2004318071,2004318071,3435973836,3435973836,28672,0,49152,0,1879076864,1879076864,3221274624,3221274624,),
    array<u32, 16>(65535,0,34952,0,4294967295,4294967295,2290649224,2290649224,61440,0,32768,0,4026593280,4026593280,2147516416,2147516416,),
    array<u32, 16>(65537,0,983055,0,65536,65537,983040,983055,286331153,0,4294967295,0,286326784,286331153,4294901760,4294967295,),
    array<u32, 16>(196611,0,917518,0,196608,196611,917504,917518,858993459,0,4008636142,0,858980352,858993459,4008574976,4008636142,),
    array<u32, 16>(458759,0,786444,0,458752,458759,786432,786444,2004318071,0,3435973836,0,2004287488,2004318071,3435921408,3435973836,),
    array<u32, 16>(983055,0,524296,0,983040,983055,524288,524296,4294967295,0,2290649224,0,4294901760,4294967295,2290614272,2290649224,),
    array<u32, 16>(1114129,0,16711935,0,1114112,1114129,16711680,16711935,286265616,0,4293984240,0,286261248,286265616,4293918720,4293984240,),
    array<u32, 16>(3342387,0,15597806,0,3342336,3342387,15597568,15597806,858796848,0,4007718624,0,858783744,858796848,4007657472,4007718624,),
    array<u32, 16>(7798903,0,13369548,0,7798784,7798903,13369344,13369548,2003859312,0,3435187392,0,2003828736,2003859312,3435134976,3435187392,),
    array<u32, 16>(16711935,0,8913032,0,16711680,16711935,8912896,8913032,4293984240,0,2290124928,0,4293918720,4293984240,2290089984,2290124928,),
    array<u32, 16>(17891601,0,268374015,0,17891328,17891601,268369920,268374015,285217024,0,4278255360,0,285212672,285217024,4278190080,4278255360,),
    array<u32, 16>(53674803,0,250482414,0,53673984,53674803,250478592,250482414,855651072,0,3993038336,0,855638016,855651072,3992977408,3993038336,),
    array<u32, 16>(125241207,0,214699212,0,125239296,125241207,214695936,214699212,1996519168,0,3422604288,0,1996488704,1996519168,3422552064,3422604288,),
    array<u32, 16>(268374015,0,143132808,0,268369920,268374015,143130624,143132808,4278255360,0,2281736192,0,4278190080,4278255360,2281701376,2281736192,),
    array<u32, 16>(286331153,0,4294967295,0,286326784,286331153,4294901760,4294967295,268439552,0,4026593280,0,268435456,268439552,4026531840,4026593280,),
    array<u32, 16>(858993459,0,4008636142,0,858980352,858993459,4008574976,4008636142,805318656,0,3758153728,0,805306368,805318656,3758096384,3758153728,),
    array<u32, 16>(2004318071,0,3435973836,0,2004287488,2004318071,3435921408,3435973836,1879076864,0,3221274624,0,1879048192,1879076864,3221225472,3221274624,),
    array<u32, 16>(4294967295,0,2290649224,0,4294901760,4294967295,2290614272,2290649224,4026593280,0,2147516416,0,4026531840,4026593280,2147483648,2147516416,),
    array<u32, 16>(65537,1,983055,15,0,65537,0,983055,286331153,4369,4294967295,65535,0,286331153,0,4294967295,),
    array<u32, 16>(196611,3,917518,14,0,196611,0,917518,858993459,13107,4008636142,61166,0,858993459,0,4008636142,),
    array<u32, 16>(458759,7,786444,12,0,458759,0,786444,2004318071,30583,3435973836,52428,0,2004318071,0,3435973836,),
    array<u32, 16>(983055,15,524296,8,0,983055,0,524296,4294967295,65535,2290649224,34952,0,4294967295,0,2290649224,),
    array<u32, 16>(1114129,17,16711935,255,0,1114129,0,16711935,286265616,4368,4293984240,65520,0,286265616,0,4293984240,),
    array<u32, 16>(3342387,51,15597806,238,0,3342387,0,15597806,858796848,13104,4007718624,61152,0,858796848,0,4007718624,),
    array<u32, 16>(7798903,119,13369548,204,0,7798903,0,13369548,2003859312,30576,3435187392,52416,0,2003859312,0,3435187392,),
    array<u32, 16>(16711935,255,8913032,136,0,16711935,0,8913032,4293984240,65520,2290124928,34944,0,4293984240,0,2290124928,),
    array<u32, 16>(17891601,273,268374015,4095,0,17891601,0,268374015,285217024,4352,4278255360,65280,0,285217024,0,4278255360,),
    array<u32, 16>(53674803,819,250482414,3822,0,53674803,0,250482414,855651072,13056,3993038336,60928,0,855651072,0,3993038336,),
    array<u32, 16>(125241207,1911,214699212,3276,0,125241207,0,214699212,1996519168,30464,3422604288,52224,0,1996519168,0,3422604288,),
    array<u32, 16>(268374015,4095,143132808,2184,0,268374015,0,143132808,4278255360,65280,2281736192,34816,0,4278255360,0,2281736192,),
    array<u32, 16>(286331153,4369,4294967295,65535,0,286331153,0,4294967295,268439552,4096,4026593280,61440,0,268439552,0,4026593280,),
    array<u32, 16>(858993459,13107,4008636142,61166,0,858993459,0,4008636142,805318656,12288,3758153728,57344,0,805318656,0,3758153728,),
    array<u32, 16>(2004318071,30583,3435973836,52428,0,2004318071,0,3435973836,1879076864,28672,3221274624,49152,0,1879076864,0,3221274624,),
    array<u32, 16>(4294967295,65535,2290649224,34952,0,4294967295,0,2290649224,4026593280,61440,2147516416,32768,0,4026593280,0,2147516416,),
    array<u32, 16>(65537,65537,983055,983055,0,65536,0,983040,286331153,286331153,4294967295,4294967295,0,286326784,0,4294901760,),
    array<u32, 16>(196611,196611,917518,917518,0,196608,0,917504,858993459,858993459,4008636142,4008636142,0,858980352,0,4008574976,),
    array<u32, 16>(458759,458759,786444,786444,0,458752,0,786432,2004318071,2004318071,3435973836,3435973836,0,2004287488,0,3435921408,),
    array<u32, 16>(983055,983055,524296,524296,0,983040,0,524288,4294967295,4294967295,2290649224,2290649224,0,4294901760,0,2290614272,),
    array<u32, 16>(1114129,1114129,16711935,16711935,0,1114112,0,16711680,286265616,286265616,4293984240,4293984240,0,286261248,0,4293918720,),
    array<u32, 16>(3342387,3342387,15597806,15597806,0,3342336,0,15597568,858796848,858796848,4007718624,4007718624,0,858783744,0,4007657472,),
    array<u32, 16>(7798903,7798903,13369548,13369548,0,7798784,0,13369344,2003859312,2003859312,3435187392,3435187392,0,2003828736,0,3435134976,),
    array<u32, 16>(16711935,16711935,8913032,8913032,0,16711680,0,8912896,4293984240,4293984240,2290124928,2290124928,0,4293918720,0,2290089984,),
    array<u32, 16>(17891601,17891601,268374015,268374015,0,17891328,0,268369920,285217024,285217024,4278255360,4278255360,0,285212672,0,4278190080,),
    array<u32, 16>(53674803,53674803,250482414,250482414,0,53673984,0,250478592,855651072,855651072,3993038336,3993038336,0,855638016,0,3992977408,),
    array<u32, 16>(125241207,125241207,214699212,214699212,0,125239296,0,214695936,1996519168,1996519168,3422604288,3422604288,0,1996488704,0,3422552064,),
    array<u32, 16>(268374015,268374015,143132808,143132808,0,268369920,0,143130624,4278255360,4278255360,2281736192,2281736192,0,4278190080,0,2281701376,),
    array<u32, 16>(286331153,286331153,4294967295,4294967295,0,286326784,0,4294901760,268439552,268439552,4026593280,4026593280,0,268435456,0,4026531840,),
    array<u32, 16>(858993459,858993459,4008636142,4008636142,0,858980352,0,4008574976,805318656,805318656,3758153728,3758153728,0,805306368,0,3758096384,),
    array<u32, 16>(2004318071,2004318071,3435973836,3435973836,0,2004287488,0,3435921408,1879076864,1879076864,3221274624,3221274624,0,1879048192,0,3221225472,),
    array<u32, 16>(4294967295,4294967295,2290649224,2290649224,0,4294901760,0,2290614272,4026593280,4026593280,2147516416,2147516416,0,4026531840,0,2147483648,),
);
