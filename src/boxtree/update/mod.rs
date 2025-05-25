pub mod clear;
pub mod insert;

#[cfg(test)]
mod tests;

use crate::{
    boxtree::{
        child_sectant_for,
        types::{BoxTreeEntry, BrickData, NodeChildren, NodeContent, PaletteIndexValues},
        Albedo, BoxTree, VoxelData, BOX_NODE_CHILDREN_COUNT, BOX_NODE_DIMENSION,
    },
    object_pool::empty_marker,
    spatial::{
        lut::SECTANT_OFFSET_LUT,
        math::{
            flat_projection, matrix_index_for, octant_in_sectants, offset_sectant, vector::V3c,
        },
        update_size_within, Cube,
    },
};
use num_traits::Zero;
use std::{fmt::Debug, hash::Hash};

#[cfg(feature = "bytecode")]
use bendy::{decoding::FromBencode, encoding::ToBencode};

impl<
        #[cfg(all(feature = "bytecode", feature = "serialization"))] T: FromBencode
            + ToBencode
            + Serialize
            + DeserializeOwned
            + Default
            + Eq
            + Clone
            + Hash
            + VoxelData,
        #[cfg(all(feature = "bytecode", not(feature = "serialization")))] T: FromBencode + ToBencode + Default + Eq + Clone + Hash + VoxelData,
        #[cfg(all(not(feature = "bytecode"), feature = "serialization"))] T: Serialize + DeserializeOwned + Default + Eq + Clone + Hash + VoxelData,
        #[cfg(all(not(feature = "bytecode"), not(feature = "serialization")))] T: Default + Eq + Clone + Hash + VoxelData,
    > BoxTree<T>
{
    //####################################################################################
    // ███████████    █████████   █████       ██████████ ███████████ ███████████ ██████████
    // ░░███░░░░░███  ███░░░░░███ ░░███       ░░███░░░░░█░█░░░███░░░█░█░░░███░░░█░░███░░░░░█
    //  ░███    ░███ ░███    ░███  ░███        ░███  █ ░ ░   ░███  ░ ░   ░███  ░  ░███  █ ░
    //  ░██████████  ░███████████  ░███        ░██████       ░███        ░███     ░██████
    //  ░███░░░░░░   ░███░░░░░███  ░███        ░███░░█       ░███        ░███     ░███░░█
    //  ░███         ░███    ░███  ░███      █ ░███ ░   █    ░███        ░███     ░███ ░   █
    //  █████        █████   █████ ███████████ ██████████    █████       █████    ██████████
    // ░░░░░        ░░░░░   ░░░░░ ░░░░░░░░░░░ ░░░░░░░░░░    ░░░░░       ░░░░░    ░░░░░░░░░░
    //####################################################################################
    /// Updates the stored palette by adding the new colors and data in the given entry
    /// Since unused colors are not removed from the palette, possible "pollution" is possible,
    /// where unused colors remain in the palette.
    /// * Returns with the resulting PaletteIndexValues Entry
    pub(crate) fn add_to_palette(&mut self, entry: &BoxTreeEntry<T>) -> PaletteIndexValues {
        match entry {
            BoxTreeEntry::Empty => empty_marker::<PaletteIndexValues>(),
            BoxTreeEntry::Visual(albedo) => {
                if **albedo == Albedo::zero() {
                    return empty_marker();
                }
                let potential_new_albedo_index = self.map_to_color_index_in_palette.keys().len();
                let albedo_index = if let std::collections::hash_map::Entry::Vacant(e) =
                    self.map_to_color_index_in_palette.entry(**albedo)
                {
                    e.insert(potential_new_albedo_index);
                    self.voxel_color_palette.push(**albedo);
                    potential_new_albedo_index
                } else {
                    self.map_to_color_index_in_palette[albedo]
                };
                debug_assert!(
                    albedo_index < u16::MAX as usize,
                    "Albedo color palette overflow!"
                );
                NodeContent::pix_visual(albedo_index as u16)
            }
            BoxTreeEntry::Informative(data) => {
                if data.is_empty() {
                    return empty_marker();
                }
                let potential_new_data_index = self.map_to_data_index_in_palette.keys().len();
                let data_index = if let std::collections::hash_map::Entry::Vacant(e) =
                    self.map_to_data_index_in_palette.entry((*data).clone())
                {
                    e.insert(potential_new_data_index);
                    self.voxel_data_palette.push((*data).clone());
                    potential_new_data_index
                } else {
                    self.map_to_data_index_in_palette[data]
                };
                debug_assert!(
                    data_index < u16::MAX as usize,
                    "Data color palette overflow!"
                );
                NodeContent::pix_informal(data_index as u16)
            }
            BoxTreeEntry::Complex(albedo, data) => {
                if **albedo == Albedo::zero() {
                    return self.add_to_palette(&BoxTreeEntry::Informative(*data));
                } else if data.is_empty() {
                    return self.add_to_palette(&BoxTreeEntry::Visual(albedo));
                }
                let potential_new_albedo_index = self.map_to_color_index_in_palette.keys().len();
                let albedo_index = if let std::collections::hash_map::Entry::Vacant(e) =
                    self.map_to_color_index_in_palette.entry(**albedo)
                {
                    e.insert(potential_new_albedo_index);
                    self.voxel_color_palette.push(**albedo);
                    potential_new_albedo_index
                } else {
                    self.map_to_color_index_in_palette[albedo]
                };
                let potential_new_data_index = self.map_to_data_index_in_palette.keys().len();
                let data_index = if let std::collections::hash_map::Entry::Vacant(e) =
                    self.map_to_data_index_in_palette.entry((*data).clone())
                {
                    e.insert(potential_new_data_index);
                    self.voxel_data_palette.push((*data).clone());
                    potential_new_data_index
                } else {
                    self.map_to_data_index_in_palette[data]
                };
                debug_assert!(
                    albedo_index < u16::MAX as usize,
                    "Albedo color palette overflow!"
                );
                debug_assert!(
                    data_index < u16::MAX as usize,
                    "Data color palette overflow!"
                );
                NodeContent::pix_complex(albedo_index as u16, data_index as u16)
            }
        }
        // find color in the palette is present, add if not
    }

    //####################################################################################
    //  █████       ██████████   █████████   ███████████
    // ░░███       ░░███░░░░░█  ███░░░░░███ ░░███░░░░░░█
    //  ░███        ░███  █ ░  ░███    ░███  ░███   █ ░
    //  ░███        ░██████    ░███████████  ░███████
    //  ░███        ░███░░█    ░███░░░░░███  ░███░░░█
    //  ░███      █ ░███ ░   █ ░███    ░███  ░███  ░
    //  ███████████ ██████████ █████   █████ █████
    // ░░░░░░░░░░░ ░░░░░░░░░░ ░░░░░   ░░░░░ ░░░░░
    //  █████  █████ ███████████  ██████████     █████████   ███████████ ██████████
    // ░░███  ░░███ ░░███░░░░░███░░███░░░░███   ███░░░░░███ ░█░░░███░░░█░░███░░░░░█
    //  ░███   ░███  ░███    ░███ ░███   ░░███ ░███    ░███ ░   ░███  ░  ░███  █ ░
    //  ░███   ░███  ░██████████  ░███    ░███ ░███████████     ░███     ░██████
    //  ░███   ░███  ░███░░░░░░   ░███    ░███ ░███░░░░░███     ░███     ░███░░█
    //  ░███   ░███  ░███         ░███    ███  ░███    ░███     ░███     ░███ ░   █
    //  ░░████████   █████        ██████████   █████   █████    █████    ██████████
    //   ░░░░░░░░   ░░░░░        ░░░░░░░░░░   ░░░░░   ░░░░░    ░░░░░    ░░░░░░░░░░
    //####################################################################################
    /// Updates the given node to be a Leaf, and inserts the provided data for it.
    /// It will update a whole node, or maximum one brick. Brick update range is starting from the position,
    /// goes up to the extent of the brick. Does not set occupancy bitmap of the given node.
    /// * Returns with the size of the actual update
    pub(crate) fn leaf_update(
        &mut self,
        overwrite_if_empty: bool,
        node_key: usize,
        node_bounds: &Cube,
        target_bounds: &Cube,
        target_child_sectant: usize,
        position: &V3c<u32>,
        size: u32,
        target_content: PaletteIndexValues,
    ) -> usize {
        // Update the leaf node, if it is possible as is, and if it's even needed to update
        // and decide if the node content needs to be divided into bricks, and the update function to be called again
        match self.nodes.get_mut(node_key) {
            NodeContent::Leaf(bricks) => {
                // In case brick_dimension == boxtree size, the 0 can not be a leaf...
                debug_assert!(self.brick_dim < self.boxtree_size);
                match &mut bricks[target_child_sectant] {
                    //If there is no brick in the target position of the leaf, create one
                    BrickData::Empty => {
                        // Create a new empty brick at the given sectant
                        let mut new_brick = vec![
                            empty_marker::<PaletteIndexValues>();
                            self.brick_dim.pow(3) as usize
                        ];
                        // update the new empty brick at the given position
                        let update_size = Self::update_brick(
                            overwrite_if_empty,
                            &mut new_brick,
                            target_bounds,
                            self.brick_dim,
                            *position,
                            size,
                            &target_content,
                        );
                        bricks[target_child_sectant] = BrickData::Parted(new_brick);
                        update_size
                    }
                    BrickData::Solid(voxel) => {
                        // In case the data doesn't match the current contents of the node, it needs to be subdivided
                        let update_size;
                        if (NodeContent::pix_points_to_empty(
                            &target_content,
                            &self.voxel_color_palette,
                            &self.voxel_data_palette,
                        ) && !NodeContent::pix_points_to_empty(
                            voxel,
                            &self.voxel_color_palette,
                            &self.voxel_data_palette,
                        )) || (!NodeContent::pix_points_to_empty(
                            &target_content,
                            &self.voxel_color_palette,
                            &self.voxel_data_palette,
                        ) && *voxel != target_content)
                        {
                            // create new brick and update it at the given position
                            let mut new_brick = vec![*voxel; self.brick_dim.pow(3) as usize];
                            update_size = Self::update_brick(
                                overwrite_if_empty,
                                &mut new_brick,
                                target_bounds,
                                self.brick_dim,
                                *position,
                                size,
                                &target_content,
                            );
                            bricks[target_child_sectant] = BrickData::Parted(new_brick);
                        } else {
                            // Since the Voxel already equals the data to be set, no need to update anything
                            update_size = 0;
                        }
                        update_size
                    }
                    BrickData::Parted(brick) => {
                        // Simply update the brick at the given position
                        Self::update_brick(
                            overwrite_if_empty,
                            brick,
                            target_bounds,
                            self.brick_dim,
                            *position,
                            size,
                            &target_content,
                        )
                    }
                }
            }
            NodeContent::UniformLeaf(mat) => {
                match mat {
                    BrickData::Empty => {
                        debug_assert_eq!(
                            self.node_children[node_key],
                            NodeChildren::OccupancyBitmap(0),
                            "Expected Node OccupancyBitmap(0) for empty leaf node instead of {:?}",
                            self.node_children[node_key]
                        );
                        if !NodeContent::pix_points_to_empty(
                            &target_content,
                            &self.voxel_color_palette,
                            &self.voxel_data_palette,
                        ) {
                            let mut new_leaf_content: [BrickData<PaletteIndexValues>;
                                BOX_NODE_CHILDREN_COUNT] =
                                vec![BrickData::Empty; BOX_NODE_CHILDREN_COUNT]
                                    .try_into()
                                    .unwrap();

                            // Add a brick to the target sectant and update with the given data
                            let mut new_brick = vec![
                                self.add_to_palette(&BoxTreeEntry::Empty);
                                self.brick_dim.pow(3) as usize
                            ];
                            let update_size = Self::update_brick(
                                overwrite_if_empty,
                                &mut new_brick,
                                target_bounds,
                                self.brick_dim,
                                *position,
                                size,
                                &target_content,
                            );
                            new_leaf_content[target_child_sectant] = BrickData::Parted(new_brick);
                            *self.nodes.get_mut(node_key) = NodeContent::Leaf(new_leaf_content);
                            return update_size;
                        }
                    }
                    BrickData::Solid(voxel) => {
                        debug_assert!(
                            !NodeContent::pix_points_to_empty(voxel, &self.voxel_color_palette, &self.voxel_data_palette)
                                && (self.node_children[node_key]
                                    == NodeChildren::OccupancyBitmap(u64::MAX))
                                || NodeContent::pix_points_to_empty(voxel, &self.voxel_color_palette, &self.voxel_data_palette)
                                    && (self.node_children[node_key]
                                        == NodeChildren::OccupancyBitmap(0)),
                            "Expected Node occupancy bitmap({:?}) to align for Solid Voxel Brick in Uniform Leaf, which is {}",
                            self.node_children[node_key],
                            if NodeContent::pix_points_to_empty(voxel, &self.voxel_color_palette, &self.voxel_data_palette) {
                                "empty"
                            } else {
                                "not empty"
                            }
                        );

                        // In case the data request doesn't match node content, it needs to be subdivided
                        if NodeContent::pix_points_to_empty(
                            &target_content,
                            &self.voxel_color_palette,
                            &self.voxel_data_palette,
                        ) && NodeContent::pix_points_to_empty(
                            voxel,
                            &self.voxel_color_palette,
                            &self.voxel_data_palette,
                        ) {
                            // Data request is to clear, it aligns with the voxel content,
                            // it's enough to update the node content in this case
                            *self.nodes.get_mut(node_key) = NodeContent::Nothing;
                            return 0;
                        }

                        if !NodeContent::pix_points_to_empty(
                            &target_content,
                            &self.voxel_color_palette,
                            &self.voxel_data_palette,
                        ) && *voxel != target_content
                            || (NodeContent::pix_points_to_empty(
                                &target_content,
                                &self.voxel_color_palette,
                                &self.voxel_data_palette,
                            ) && !NodeContent::pix_points_to_empty(
                                voxel,
                                &self.voxel_color_palette,
                                &self.voxel_data_palette,
                            ))
                        {
                            // Data request doesn't align with the voxel data
                            // create a voxel brick and try to update with the given data
                            *mat = BrickData::Parted(vec![
                                *voxel;
                                (self.brick_dim * self.brick_dim * self.brick_dim)
                                    as usize
                            ]);

                            return self.leaf_update(
                                overwrite_if_empty,
                                node_key,
                                node_bounds,
                                target_bounds,
                                target_child_sectant,
                                position,
                                size,
                                target_content,
                            );
                        }

                        // data request aligns with node content
                        return 0;
                    }
                    BrickData::Parted(brick) => {
                        // Check if the voxel at the target position matches with the data update request
                        // The target position index is to be calculated from the node bounds,
                        // instead of the target bounds because the position should cover the whole leaf
                        // not just one brick in it
                        let mat_index = matrix_index_for(node_bounds, position, self.brick_dim);
                        let mat_index = flat_projection(
                            mat_index.x,
                            mat_index.y,
                            mat_index.z,
                            self.brick_dim as usize,
                        );
                        if 1 < self.brick_dim // BrickData can only stay parted if brick_dimension is above 1
                            && (
                                (
                                    NodeContent::pix_points_to_empty(
                                        &target_content,
                                        &self.voxel_color_palette,
                                        &self.voxel_data_palette,
                                    )
                                    && NodeContent::pix_points_to_empty(
                                        &brick[mat_index],
                                        &self.voxel_color_palette,
                                        &self.voxel_data_palette
                                    )
                                )||(
                                    !NodeContent::pix_points_to_empty(
                                        &target_content,
                                        &self.voxel_color_palette,
                                        &self.voxel_data_palette,
                                    )
                                    && brick[mat_index] == target_content
                                )
                            )
                        {
                            // Target voxel matches with the data request, there's nothing to do!
                            return 0;
                        }

                        // If uniform leaf is the size of one brick, the brick is updated as is
                        if node_bounds.size <= self.brick_dim as f32 && self.brick_dim > 1 {
                            return Self::update_brick(
                                overwrite_if_empty,
                                brick,
                                node_bounds,
                                self.brick_dim,
                                *position,
                                size,
                                &target_content,
                            );
                        }

                        // the data at the position inside the brick doesn't match the given data,
                        // so the leaf needs to be divided into a NodeContent::Leaf(bricks)
                        let mut leaf_data: [BrickData<PaletteIndexValues>;
                            BOX_NODE_CHILDREN_COUNT] =
                            vec![BrickData::Empty; BOX_NODE_CHILDREN_COUNT]
                                .try_into()
                                .unwrap();

                        // Each brick is mapped to take up one subsection of the current data
                        let child_bricks =
                            Self::dilute_brick_data(std::mem::take(brick), self.brick_dim);
                        let mut update_size = 0;
                        for (sectant, mut new_brick) in child_bricks.into_iter().enumerate() {
                            // Also update the brick if it is the target
                            if sectant == target_child_sectant {
                                update_size = Self::update_brick(
                                    overwrite_if_empty,
                                    &mut new_brick,
                                    target_bounds,
                                    self.brick_dim,
                                    *position,
                                    size,
                                    &target_content,
                                );
                            }
                            leaf_data[sectant] = BrickData::Parted(new_brick);
                        }

                        *self.nodes.get_mut(node_key) = NodeContent::Leaf(leaf_data);
                        debug_assert_ne!(
                            0, update_size,
                            "Expected Leaf node to be updated in operation"
                        );
                        return update_size;
                    }
                }
                self.leaf_update(
                    overwrite_if_empty,
                    node_key,
                    node_bounds,
                    target_bounds,
                    target_child_sectant,
                    position,
                    size,
                    target_content,
                )
            }
            NodeContent::Internal(ocbits) => {
                // Warning: Calling leaf update to an internal node might induce data loss - see #69
                self.node_children[node_key] = NodeChildren::OccupancyBitmap(*ocbits);
                *self.nodes.get_mut(node_key) = NodeContent::Leaf(
                    (0..BOX_NODE_CHILDREN_COUNT)
                        .map(|sectant| {
                            self.try_brick_from_node(
                                self.node_children[node_key].child(sectant as u8),
                            )
                        })
                        .collect::<Vec<_>>()
                        .try_into()
                        .unwrap(),
                );
                self.deallocate_children_of(node_key);
                self.leaf_update(
                    overwrite_if_empty,
                    node_key,
                    node_bounds,
                    target_bounds,
                    target_child_sectant,
                    position,
                    size,
                    target_content,
                )
            }
            NodeContent::Nothing => {
                // Calling leaf update on Nothing is an odd thing to do..
                // But possible, if this call is mid-update
                // So let's try to gather all the information possible
                *self.nodes.get_mut(node_key) = NodeContent::Leaf(
                    (0..BOX_NODE_CHILDREN_COUNT)
                        .map(|sectant| {
                            self.try_brick_from_node(
                                self.node_children[node_key].child(sectant as u8),
                            )
                        })
                        .collect::<Vec<_>>()
                        .try_into()
                        .unwrap(),
                );
                self.deallocate_children_of(node_key);
                self.leaf_update(
                    overwrite_if_empty,
                    node_key,
                    node_bounds,
                    target_bounds,
                    target_child_sectant,
                    position,
                    size,
                    target_content,
                )
            }
        }
    }

    /// Calls the given function for every child position inside the given update range
    /// The function is called at least once
    /// * `node_bounds` - The bounds of the updated node
    /// * `position` - The position of the intended update
    /// * `update_size` - Range of the intended update starting from position
    /// * `target_size` - The size of one child inside the updated node
    /// * `fun` - The function to execute
    ///
    /// returns with update size
    fn execute_for_relevant_sectants<F: FnMut(V3c<u32>, u32, u8, &Cube)>(
        node_bounds: &Cube,
        position: &V3c<u32>,
        update_size: u32,
        target_size: f32,
        mut fun: F,
    ) -> usize {
        let children_updated_dimension =
            (update_size_within(node_bounds, position, update_size) as f32 / target_size).ceil()
                as u32;
        for x in 0..children_updated_dimension {
            for y in 0..children_updated_dimension {
                for z in 0..children_updated_dimension {
                    let shifted_position = V3c::from(*position)
                        + V3c::unit(target_size) * V3c::new(x as f32, y as f32, z as f32);
                    let target_child_sectant = child_sectant_for(node_bounds, &shifted_position);
                    let target_bounds = node_bounds.child_bounds_for(target_child_sectant);

                    // In case smaller brick dimensions, it might happen that one update affects multiple sectants
                    // e.g. when a uniform leaf has a parted brick of 2x2x2 --> Setting a value in one element
                    // affects multiple sectants. In these cases, the target size is 0.5, and positions
                    // also move inbetween voxels. Logically this is needed for e.g. setting the correct occupied bits
                    // for a given node. The worst case scenario is some cells are given a value multiple times,
                    // which is acceptable for the time being
                    let target_bounds = Cube {
                        min_position: target_bounds.min_position.floor(),
                        size: target_bounds.size.ceil(),
                    };
                    let (position_in_target, update_size_in_target) = if 0 == x && 0 == y && 0 == z
                    {
                        // Update starts from the start position, goes until end of first target cell
                        (
                            *position,
                            update_size_within(&target_bounds, position, update_size),
                        )
                    } else {
                        // Update starts from the start from update position projected onto target bound edge
                        let update_position = V3c::new(
                            position.x.max(target_bounds.min_position.x as u32),
                            position.y.max(target_bounds.min_position.y as u32),
                            position.z.max(target_bounds.min_position.z as u32),
                        );
                        let trimmed_update_vector =
                            *position + V3c::unit(update_size) - update_position;
                        let update_size_left = trimmed_update_vector
                            .x
                            .min(trimmed_update_vector.y)
                            .min(trimmed_update_vector.z);
                        (
                            update_position,
                            update_size_within(&target_bounds, &update_position, update_size_left),
                        )
                    };

                    fun(
                        position_in_target,
                        update_size_in_target,
                        target_child_sectant,
                        &target_bounds,
                    );
                }
            }
        }
        (target_size * children_updated_dimension as f32) as usize
    }

    //####################################################################################
    //  ███████████  ███████████   █████   █████████  █████   ████
    // ░░███░░░░░███░░███░░░░░███ ░░███   ███░░░░░███░░███   ███░
    //  ░███    ░███ ░███    ░███  ░███  ███     ░░░  ░███  ███
    //  ░██████████  ░██████████   ░███ ░███          ░███████
    //  ░███░░░░░███ ░███░░░░░███  ░███ ░███          ░███░░███
    //  ░███    ░███ ░███    ░███  ░███ ░░███     ███ ░███ ░░███
    //  ███████████  █████   █████ █████ ░░█████████  █████ ░░████
    // ░░░░░░░░░░░  ░░░░░   ░░░░░ ░░░░░   ░░░░░░░░░  ░░░░░   ░░░░
    //####################################################################################
    /// Provides an array of bricks, based on the given brick data, with the same size of the original brick,
    /// each voxel mapped as the new bricks were the children of the given brick
    pub(crate) fn dilute_brick_data<B>(
        brick_data: Vec<B>,
        brick_dim: u32,
    ) -> [Vec<B>; BOX_NODE_CHILDREN_COUNT]
    where
        B: Debug + Clone + Copy + PartialEq,
    {
        debug_assert_eq!(brick_data.len(), brick_dim.pow(3) as usize);

        if 1 == brick_dim {
            debug_assert_eq!(brick_data.len(), 1);
            return vec![brick_data.clone(); BOX_NODE_CHILDREN_COUNT]
                .try_into()
                .unwrap();
        }

        if 2 == brick_dim {
            debug_assert_eq!(brick_data.len(), 8);
            return (0..BOX_NODE_CHILDREN_COUNT)
                .map(|sectant| {
                    vec![brick_data[octant_in_sectants(sectant)]; brick_dim.pow(3) as usize]
                })
                .collect::<Vec<_>>()
                .try_into()
                .unwrap();
        };

        debug_assert!(brick_data.len() <= BOX_NODE_CHILDREN_COUNT);
        let mut result: [Vec<B>; BOX_NODE_CHILDREN_COUNT] = (0..BOX_NODE_CHILDREN_COUNT)
            .map(|sectant| vec![brick_data[sectant]; brick_dim.pow(3) as usize])
            .collect::<Vec<_>>()
            .try_into()
            .unwrap();

        // in case one child can be mapped 1:1 to an element in the brick
        if 4 == brick_dim {
            debug_assert_eq!(brick_data.len(), BOX_NODE_CHILDREN_COUNT);
            return result;
        }

        // Generic case
        // Note: Each value in @result will be overwritten
        for sectant in 0..BOX_NODE_CHILDREN_COUNT {
            // Set the data of the new child
            let brick_offset: V3c<usize> =
                V3c::from(SECTANT_OFFSET_LUT[sectant] * brick_dim as f32);
            let new_brick_flat_offset = flat_projection(
                brick_offset.x,
                brick_offset.y,
                brick_offset.z,
                brick_dim as usize,
            );
            let mut new_brick_data =
                vec![brick_data[new_brick_flat_offset]; brick_dim.pow(3) as usize];
            for x in 0..brick_dim as usize {
                for y in 0..brick_dim as usize {
                    for z in 0..brick_dim as usize {
                        if x < BOX_NODE_DIMENSION
                            && y < BOX_NODE_DIMENSION
                            && z < BOX_NODE_DIMENSION
                        {
                            continue;
                        }
                        let new_brick_flat_offset = flat_projection(x, y, z, brick_dim as usize);
                        let brick_flat_offset = flat_projection(
                            brick_offset.x + x / BOX_NODE_DIMENSION,
                            brick_offset.y + y / BOX_NODE_DIMENSION,
                            brick_offset.z + z / BOX_NODE_DIMENSION,
                            brick_dim as usize,
                        );
                        new_brick_data[new_brick_flat_offset] = brick_data[brick_flat_offset];
                    }
                }
            }
            result[sectant] = new_brick_data;
        }
        result
    }

    /// Updates the content of the given brick and its occupancy bitmap. Each components of mat_index must be smaller, than the size of the brick.
    /// mat_index + size however need not be in bounds, the function will cut each component to fit inside the brick.
    /// * `brick` - mutable reference of the brick to update
    /// * `mat_index` - the first position to update with the given data
    /// * `size` - the number of elements in x,y,z to update with the given data
    /// * `data` - the data  to update the brick with. Erases data in case `None`
    /// * Returns with the size of the update
    fn update_brick(
        overwrite_if_empty: bool,
        brick: &mut [PaletteIndexValues],
        brick_bounds: &Cube,
        brick_dim: u32,
        position: V3c<u32>,
        size: u32,
        data: &PaletteIndexValues,
    ) -> usize {
        debug_assert!(
            brick_bounds.contains(&(position.into())),
            "Expected position {:?} to be contained in brick bounds {:?}",
            position,
            brick_bounds
        );

        let mat_index = matrix_index_for(brick_bounds, &position, brick_dim);
        let update_size = (brick_dim as usize - mat_index.x).min(size as usize);
        for x in mat_index.x..(mat_index.x + size as usize).min(brick_dim as usize) {
            for y in mat_index.y..(mat_index.y + size as usize).min(brick_dim as usize) {
                for z in mat_index.z..(mat_index.z + size as usize).min(brick_dim as usize) {
                    let mat_index = flat_projection(x, y, z, brick_dim as usize);
                    if overwrite_if_empty {
                        brick[mat_index] = *data;
                    } else {
                        if NodeContent::pix_color_is_some(data) {
                            brick[mat_index] =
                                NodeContent::pix_overwrite_color(brick[mat_index], data);
                        }
                        if NodeContent::pix_data_is_some(data) {
                            brick[mat_index] =
                                NodeContent::pix_overwrite_data(brick[mat_index], data);
                        }
                    }
                }
            }
        }
        update_size
    }

    //####################################################################################
    //   █████████  █████ ██████   ██████ ███████████  █████       █████ ███████████ █████ █████
    //  ███░░░░░███░░███ ░░██████ ██████ ░░███░░░░░███░░███       ░░███ ░░███░░░░░░█░░███ ░░███
    // ░███    ░░░  ░███  ░███░█████░███  ░███    ░███ ░███        ░███  ░███   █ ░  ░░███ ███
    // ░░█████████  ░███  ░███░░███ ░███  ░██████████  ░███        ░███  ░███████     ░░█████
    //  ░░░░░░░░███ ░███  ░███ ░░░  ░███  ░███░░░░░░   ░███        ░███  ░███░░░█      ░░███
    //  ███    ░███ ░███  ░███      ░███  ░███         ░███      █ ░███  ░███  ░        ░███
    // ░░█████████  █████ █████     █████ █████        ███████████ █████ █████          █████
    //  ░░░░░░░░░  ░░░░░ ░░░░░     ░░░░░ ░░░░░        ░░░░░░░░░░░ ░░░░░ ░░░░░          ░░░░░
    //####################################################################################
    /// Updates the given node recursively to collapse nodes with uniform children into a leaf
    /// Returns with true if the given node was simplified
    pub(crate) fn simplify(&mut self, node_key: usize, recursive: bool) -> bool {
        if self.nodes.key_is_valid(node_key) {
            #[cfg(debug_assertions)]
            {
                if let NodeContent::Internal(ocbits) = self.nodes.get(node_key) {
                    for sectant in 0..BOX_NODE_CHILDREN_COUNT as u8 {
                        if self.node_empty_at(node_key, sectant) {
                            debug_assert_eq!(
                                0,
                                *ocbits & (0x01 << sectant),
                                "Expected node[{:?}] ocbits({:#10X}) to represent child at sectant[{:?}]: \n{:?}",
                                node_key, ocbits, sectant,
                                self.nodes.get(self.node_children[node_key].child(sectant))
                            )
                        }
                    }
                }
            }

            match self.nodes.get_mut(node_key) {
                NodeContent::Nothing => true,
                NodeContent::UniformLeaf(brick) => {
                    debug_assert!(
                        matches!(
                            self.node_children[node_key],
                            NodeChildren::OccupancyBitmap(_)
                        ),
                        "Uniform leaf has {:?} instead of an Occupancy_bitmap(_)",
                        self.node_children[node_key]
                    );
                    match brick {
                        BrickData::Empty => true,
                        BrickData::Solid(voxel) => {
                            if NodeContent::pix_points_to_empty(
                                voxel,
                                &self.voxel_color_palette,
                                &self.voxel_data_palette,
                            ) {
                                debug_assert_eq!(
                                    0,
                                    if let NodeChildren::OccupancyBitmap(occupied_bits) =
                                        self.node_children[node_key]
                                    {
                                        occupied_bits
                                    } else {
                                        0xD34D
                                    },
                                    "Solid empty voxel should have its occupied bits set to 0, instead of {:#10X}",
                                    if let NodeChildren::OccupancyBitmap(occupied_bits) =
                                        self.node_children[node_key]
                                    {
                                        occupied_bits
                                    } else {
                                        0xD34D
                                    }
                                );
                                *self.nodes.get_mut(node_key) = NodeContent::Nothing;
                                self.node_children[node_key] = NodeChildren::NoChildren;
                                true
                            } else {
                                debug_assert_eq!(
                                    u64::MAX,
                                    if let NodeChildren::OccupancyBitmap(occupied_bits) =
                                        self.node_children[node_key]
                                    {
                                        occupied_bits
                                    } else {
                                        0xD34D
                                    },
                                    "Solid full voxel should have its occupied bits set to u64::MAX, instead of {:#10X}",
                                    if let NodeChildren::OccupancyBitmap(occupied_bits) =
                                        self.node_children[node_key]
                                    {
                                        occupied_bits
                                    } else {
                                        0xD34D
                                    }
                                );
                                false
                            }
                        }
                        BrickData::Parted(_brick) => {
                            if brick.simplify(&self.voxel_color_palette, &self.voxel_data_palette) {
                                debug_assert!(
                                    self.node_children[node_key]
                                        == NodeChildren::OccupancyBitmap(u64::MAX)
                                        || self.node_children[node_key]
                                            == NodeChildren::OccupancyBitmap(0),
                                    "Expected brick occuped bits( inside {:?}) to be either full or empty, becasue it could be simplified",
                                    self.node_children[node_key]
                                );
                                true
                            } else {
                                false
                            }
                        }
                    }
                }
                NodeContent::Leaf(bricks) => {
                    #[cfg(debug_assertions)]
                    {
                        for (sectant, brick) in
                            bricks.iter().enumerate().take(BOX_NODE_CHILDREN_COUNT)
                        {
                            if let BrickData::Solid(_) | BrickData::Empty = brick {
                                // with solid and empty bricks, the relevant occupied bits should either be empty or full
                                if let NodeChildren::OccupancyBitmap(occupied_bits) =
                                    self.node_children[node_key]
                                {
                                    let sectant_bitmask = 0x01 << sectant;
                                    debug_assert!(
                                        0 == occupied_bits & sectant_bitmask
                                            || sectant_bitmask == occupied_bits & sectant_bitmask,
                                        "Brickdata at sectant[{:?}] doesn't match occupied bits: {:?} <> {:#10X}",
                                        sectant, brick, occupied_bits,
                                    );
                                }
                            }
                        }
                    }

                    debug_assert!(
                        matches!(
                            self.node_children[node_key],
                            NodeChildren::OccupancyBitmap(_),
                        ),
                        "Expected node child to be OccupancyBitmap(_) instead of {:?}",
                        self.node_children[node_key]
                    );

                    // Try to simplify bricks
                    let mut simplified = false;
                    let mut is_leaf_uniform_solid = true;
                    let mut uniform_solid_value = None;

                    for brick in bricks.iter_mut().take(BOX_NODE_CHILDREN_COUNT) {
                        simplified |=
                            brick.simplify(&self.voxel_color_palette, &self.voxel_data_palette);

                        if is_leaf_uniform_solid {
                            if let BrickData::Solid(voxel) = brick {
                                if let Some(ref uniform_solid_value) = uniform_solid_value {
                                    if *uniform_solid_value != voxel {
                                        is_leaf_uniform_solid = false;
                                    }
                                } else {
                                    uniform_solid_value = Some(voxel);
                                }
                            } else {
                                is_leaf_uniform_solid = false;
                            }
                        }
                    }

                    // Try to unite bricks into a solid brick
                    let mut unified_brick = BrickData::Empty;
                    if is_leaf_uniform_solid {
                        debug_assert_ne!(uniform_solid_value, None);
                        debug_assert_eq!(
                            self.node_children[node_key],
                            NodeChildren::OccupancyBitmap(u64::MAX),
                            "Expected Leaf with uniform solid value to have u64::MAX value"
                        );
                        *self.nodes.get_mut(node_key) = NodeContent::UniformLeaf(BrickData::Solid(
                            *uniform_solid_value.unwrap(),
                        ));
                        return true;
                    }

                    // Do not try to unite bricks into a uniform brick
                    // since contents are not solid, it is not unifyable
                    // into a 1x1x1 brick ( that's equivalent to a solid brick )
                    if self.brick_dim == 1 {
                        return false;
                    }

                    // Try to unite bricks into a Uniform parted brick
                    let mut unified_brick_data =
                        vec![empty_marker::<PaletteIndexValues>(); self.brick_dim.pow(3) as usize];
                    let mut is_leaf_uniform = true;
                    const BRICK_CELL_SIZE: usize = BOX_NODE_DIMENSION;
                    let superbrick_size = self.brick_dim as f32 * BOX_NODE_DIMENSION as f32;
                    'brick_process: for x in 0..self.brick_dim {
                        for y in 0..self.brick_dim {
                            for z in 0..self.brick_dim {
                                let cell_start =
                                    V3c::new(x as f32, y as f32, z as f32) * BRICK_CELL_SIZE as f32;
                                let ref_sectant =
                                    offset_sectant(&cell_start, superbrick_size) as usize;
                                let pos_in_child =
                                    cell_start - SECTANT_OFFSET_LUT[ref_sectant] * superbrick_size;
                                let ref_voxel = match &bricks[ref_sectant] {
                                    BrickData::Empty => empty_marker(),
                                    BrickData::Solid(voxel) => *voxel,
                                    BrickData::Parted(brick) => {
                                        brick[flat_projection(
                                            pos_in_child.x as usize,
                                            pos_in_child.y as usize,
                                            pos_in_child.z as usize,
                                            self.brick_dim as usize,
                                        )]
                                    }
                                };

                                for cx in 0..BRICK_CELL_SIZE {
                                    for cy in 0..BRICK_CELL_SIZE {
                                        for cz in 0..BRICK_CELL_SIZE {
                                            if !is_leaf_uniform {
                                                break 'brick_process;
                                            }
                                            let pos = cell_start
                                                + V3c::new(cx as f32, cy as f32, cz as f32);
                                            let sectant =
                                                offset_sectant(&pos, superbrick_size) as usize;
                                            let pos_in_child =
                                                pos - SECTANT_OFFSET_LUT[sectant] * superbrick_size;

                                            is_leaf_uniform &= match &bricks[sectant] {
                                                BrickData::Empty => {
                                                    ref_voxel
                                                        == empty_marker::<PaletteIndexValues>()
                                                }
                                                BrickData::Solid(voxel) => ref_voxel == *voxel,
                                                BrickData::Parted(brick) => {
                                                    ref_voxel
                                                        == brick[flat_projection(
                                                            pos_in_child.x as usize,
                                                            pos_in_child.y as usize,
                                                            pos_in_child.z as usize,
                                                            self.brick_dim as usize,
                                                        )]
                                                }
                                            };
                                        }
                                    }
                                }
                                // All voxel are the same in this cell! set value in unified brick
                                unified_brick_data[flat_projection(
                                    x as usize,
                                    y as usize,
                                    z as usize,
                                    self.brick_dim as usize,
                                )] = ref_voxel;
                            }
                        }
                    }

                    // bricks can be represented as a uniform parted brick matrix!
                    if is_leaf_uniform {
                        unified_brick = BrickData::Parted(unified_brick_data);
                        simplified = true;
                    }

                    if !matches!(unified_brick, BrickData::Empty) {
                        *self.nodes.get_mut(node_key) = NodeContent::UniformLeaf(unified_brick);
                    }

                    simplified
                }
                NodeContent::Internal(ocbits) => {
                    if 0 == *ocbits
                        || matches!(self.node_children[node_key], NodeChildren::NoChildren)
                    {
                        if let NodeContent::Nothing = self.nodes.get(node_key) {
                            return false;
                        }

                        *self.nodes.get_mut(node_key) = NodeContent::Nothing;
                        return true;
                    }

                    debug_assert!(
                        matches!(self.node_children[node_key], NodeChildren::Children(_)),
                        "Expected Internal node to have Children instead of {:?}",
                        self.node_children[node_key]
                    );
                    let child_keys =
                        if let NodeChildren::Children(children) = self.node_children[node_key] {
                            children
                        } else {
                            return false;
                        };

                    // Try to simplify each child of the node
                    if recursive {
                        for child_key in child_keys.iter() {
                            self.simplify(*child_key as usize, true);
                        }
                    }

                    for sectant in 1..BOX_NODE_CHILDREN_COUNT {
                        if !self.compare_nodes(child_keys[0] as usize, child_keys[sectant] as usize)
                        {
                            return false;
                        }
                    }

                    // All children are the same!
                    // make the current node a leaf, erase the children
                    debug_assert!(matches!(
                        self.nodes.get(child_keys[0] as usize),
                        NodeContent::Leaf(_) | NodeContent::UniformLeaf(_)
                    ));
                    self.nodes.swap(node_key, child_keys[0] as usize);

                    // Deallocate children, and set correct occupancy bitmap
                    let new_node_children = self.node_children[child_keys[0] as usize];
                    self.deallocate_children_of(node_key);
                    self.node_children[node_key] = new_node_children;

                    // At this point there's no need to call simplify on the new leaf node
                    // because it's been attempted already on the data it copied from
                    true
                }
            }
        } else {
            // can't simplify invalid node
            false
        }
    }
}
