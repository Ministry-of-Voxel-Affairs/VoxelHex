use crate::{
    boxtree::{
        types::{BrickData, NodeContent},
        BoxTree, VoxelData, BOX_NODE_CHILDREN_COUNT, OOB_SECTANT,
    },
    object_pool::empty_marker,
    raytracing::bevy::types::{
        BoxTreeGPUDataHandler, BoxTreeRenderData, BrickOwnedBy, BrickUpdate, CacheUpdatePackage,
        VictimPointer,
    },
};
use bendy::{decoding::FromBencode, encoding::ToBencode};
use std::{hash::Hash, ops::Range};

//##############################################################################
//  █████   █████ █████   █████████  ███████████ █████ ██████   ██████
// ░░███   ░░███ ░░███   ███░░░░░███░█░░░███░░░█░░███ ░░██████ ██████
//  ░███    ░███  ░███  ███     ░░░ ░   ░███  ░  ░███  ░███░█████░███
//  ░███    ░███  ░███ ░███             ░███     ░███  ░███░░███ ░███
//  ░░███   ███   ░███ ░███             ░███     ░███  ░███ ░░░  ░███
//   ░░░█████░    ░███ ░░███     ███    ░███     ░███  ░███      ░███
//     ░░███      █████ ░░█████████     █████    █████ █████     █████
//      ░░░      ░░░░░   ░░░░░░░░░     ░░░░░    ░░░░░ ░░░░░     ░░░░░
//  ███████████  ███████████ ███████████
// ░░███░░░░░███░█░░░███░░░█░░███░░░░░███
//  ░███    ░███░   ░███  ░  ░███    ░███
//  ░██████████     ░███     ░██████████
//  ░███░░░░░░      ░███     ░███░░░░░███
//  ░███            ░███     ░███    ░███
//  █████           █████    █████   █████
// ░░░░░           ░░░░░    ░░░░░   ░░░░░
//##############################################################################
impl VictimPointer {
    /// Returns the size of the buffer this pointer covers
    pub(crate) fn len(&self) -> usize {
        self.max_meta_len
    }

    /// Returns true if no new nodes can be added without overwriting another
    pub(crate) fn is_full(&self) -> bool {
        self.max_meta_len <= self.stored_items
    }

    /// Creates object, based on the given cache length it should cover
    pub(crate) fn new(max_meta_len: usize) -> Self {
        Self {
            max_meta_len,
            loop_count: 0,
            stored_items: 1,
            meta_index: 1,
            child: 0,
        }
    }

    /// Steps the iterator forward to the next children, if available, or the next node.
    /// Wraps around
    pub(crate) fn step(&mut self) {
        if self.child >= (BOX_NODE_CHILDREN_COUNT - 1) {
            self.skip_node();
        } else {
            self.child += 1;
        }
    }

    /// Steps the iterator forward one node
    pub(crate) fn skip_node(&mut self) {
        if self.meta_index == 0 {
            self.loop_count += 1;
            self.meta_index = self.max_meta_len - 1;
        } else {
            self.meta_index -= 1;
        }
        self.child = 0;
    }

    /// Provides the number of times the victim node pointer has started from the first element in the cache
    pub(crate) fn get_loop_count(&self) -> usize {
        self.loop_count
    }

    /// Provides the first available index in the metadata buffer which can be overwritten
    /// Optionally the source where the child can be taken from.
    /// And finally the index range where nodes were updated
    fn first_available_node(
        &mut self,
        render_data: &mut BoxTreeRenderData,
    ) -> (usize, Option<(usize, u8)>, Range<usize>) {
        // If there is space left in the cache, use it all up
        if !self.is_full() {
            render_data.used_bits[self.stored_items] |= 0x01;
            self.meta_index = self.stored_items;
            self.stored_items += 1;
            return (
                self.meta_index,
                None,
                self.meta_index..(self.meta_index + 1),
            );
        }

        // look for the next internal node ( with node children )
        let mut modified_range = self.meta_index..(self.meta_index + 1);
        loop {
            modified_range.start = modified_range.start.min(self.meta_index);
            modified_range.end = modified_range.end.max(self.meta_index + 1);

            let child_offset = self.meta_index * BOX_NODE_CHILDREN_COUNT + self.child;
            let child_meta_index = render_data.node_children[child_offset] as usize;

            // child of non-leaf node at target is not empty, which means
            // the target child might point to an internal node if it's valid
            if 0 == (render_data.node_metadata[self.meta_index / 8]
                & (0x01 << (self.meta_index % 8)))
                && child_meta_index != empty_marker::<u32>() as usize
            {
                debug_assert!(
                    child_meta_index < render_data.used_bits.len(),
                    "Expected child[{:?}] of meta_node[{:?}]({:#10X}) to point inside index. Child: {:?}",
                    self.child,
                    self.meta_index,
                    render_data.node_metadata[self.meta_index / 8],
                    child_meta_index
                );
                if 0 == (render_data.used_bits[child_meta_index] & 0x01) {
                    render_data.used_bits[child_meta_index] |= 0x01;
                    return (
                        child_meta_index,
                        Some((self.meta_index, self.child as u8)),
                        modified_range,
                    );
                } else {
                    // mark child as unused
                    render_data.used_bits[child_meta_index] &= !0x01;
                }
            }
            self.step();
        }
    }
}

impl BoxTreeGPUDataHandler {
    //##############################################################################
    //  ██████████     █████████   ███████████   █████████
    // ░░███░░░░███   ███░░░░░███ ░█░░░███░░░█  ███░░░░░███
    //  ░███   ░░███ ░███    ░███ ░   ░███  ░  ░███    ░███
    //  ░███    ░███ ░███████████     ░███     ░███████████
    //  ░███    ░███ ░███░░░░░███     ░███     ░███░░░░░███
    //  ░███    ███  ░███    ░███     ░███     ░███    ░███
    //  ██████████   █████   █████    █████    █████   █████
    // ░░░░░░░░░░   ░░░░░   ░░░░░    ░░░░░    ░░░░░   ░░░░░

    //  ██████████   ██████████  █████████  █████   █████████  ██████   █████
    // ░░███░░░░███ ░░███░░░░░█ ███░░░░░███░░███   ███░░░░░███░░██████ ░░███
    //  ░███   ░░███ ░███  █ ░ ░███    ░░░  ░███  ███     ░░░  ░███░███ ░███
    //  ░███    ░███ ░██████   ░░█████████  ░███ ░███          ░███░░███░███
    //  ░███    ░███ ░███░░█    ░░░░░░░░███ ░███ ░███    █████ ░███ ░░██████
    //  ░███    ███  ░███ ░   █ ███    ░███ ░███ ░░███  ░░███  ░███  ░░█████
    //  ██████████   ██████████░░█████████  █████ ░░█████████  █████  ░░█████
    // ░░░░░░░░░░   ░░░░░░░░░░  ░░░░░░░░░  ░░░░░   ░░░░░░░░░  ░░░░░    ░░░░░
    //##############################################################################

    /// Provides the mask inside a metadata element if the brick under the given index is used.
    fn get_brick_used(used_bits: &[u32], brick_index: usize) -> bool {
        0 != (used_bits[brick_index / 31] & (0x01 << (1 + (brick_index % 31))))
    }

    /// Updates the given metadata array to set the given brick as used
    fn set_brick_used(used_bits: &mut [u32], brick_index: usize, used: bool) {
        if used {
            used_bits[brick_index / 31] |= 0x01 << (1 + (brick_index % 31));
        } else {
            used_bits[brick_index / 31] &= !(0x01 << (1 + (brick_index % 31)));
        }
    }

    /// Creates the descriptor bytes for the given node
    fn inject_node_properties<T>(
        meta_array: &mut [u32],
        node_index: usize,
        tree: &BoxTree<T>,
        node_key: usize,
    ) where
        T: Default + Clone + Eq + VoxelData + Hash,
    {
        // set node type
        match tree.nodes.get(node_key) {
            NodeContent::Internal(_) | NodeContent::Nothing => {
                meta_array[node_index / 8] &= !(0x01 << (node_index % 8));
                meta_array[node_index / 8] &= !(0x01 << (8 + (node_index % 8)));
            }
            NodeContent::Leaf(_bricks) => {
                meta_array[node_index / 8] |= 0x01 << (node_index % 8);
                meta_array[node_index / 8] &= !(0x01 << (8 + (node_index % 8)));
            }
            NodeContent::UniformLeaf(_brick) => {
                meta_array[node_index / 8] |= 0x01 << (node_index % 8);
                meta_array[node_index / 8] |= 0x01 << (8 + (node_index % 8));
            }
        };

        // set node MIP properties
        if let BrickData::Solid(_) | BrickData::Parted(_) = tree.node_mips[node_key] {
            meta_array[node_index / 8] |= 0x01 << (16 + (node_index % 8));
        }
    }

    //##############################################################################
    //  ██████████ ███████████     █████████    █████████  ██████████
    // ░░███░░░░░█░░███░░░░░███   ███░░░░░███  ███░░░░░███░░███░░░░░█
    //  ░███  █ ░  ░███    ░███  ░███    ░███ ░███    ░░░  ░███  █ ░
    //  ░██████    ░█████████    ░███████████ ░░█████████  ░██████
    //  ░███░░█    ░███░░░░░███  ░███░░░░░███  ░░░░░░░░███ ░███░░█
    //  ░███ ░   █ ░███    ░███  ░███    ░███  ███    ░███ ░███ ░   █
    //  ██████████ █████   █████ █████   █████░░█████████  ██████████
    // ░░░░░░░░░░ ░░░░░   ░░░░░ ░░░░░   ░░░░░  ░░░░░░░░░  ░░░░░░░░░░
    //  ██████   █████    ███████    ██████████   ██████████
    // ░░██████ ░░███   ███░░░░░███ ░░███░░░░███ ░░███░░░░░█
    //  ░███░███ ░███  ███     ░░███ ░███   ░░███ ░███  █ ░
    //  ░███░░███░███ ░███      ░███ ░███    ░███ ░██████
    //  ░███ ░░██████ ░███      ░███ ░███    ░███ ░███░░█
    //  ░███  ░░█████ ░░███     ███  ░███    ███  ░███ ░   █
    //  █████  ░░█████ ░░░███████░   ██████████   ██████████
    // ░░░░░    ░░░░░    ░░░░░░░    ░░░░░░░░░░   ░░░░░░░░░░
    //    █████████  █████   █████ █████ █████       ██████████
    //   ███░░░░░███░░███   ░░███ ░░███ ░░███       ░░███░░░░███
    //  ███     ░░░  ░███    ░███  ░███  ░███        ░███   ░░███
    // ░███          ░███████████  ░███  ░███        ░███    ░███
    // ░███          ░███░░░░░███  ░███  ░███        ░███    ░███
    // ░░███     ███ ░███    ░███  ░███  ░███      █ ░███    ███
    //  ░░█████████  █████   █████ █████ ███████████ ██████████
    //   ░░░░░░░░░  ░░░░░   ░░░░░ ░░░░░ ░░░░░░░░░░░ ░░░░░░░░░░
    //##############################################################################
    /// Erases the child node pointed by the given victim pointer
    /// returns with the vector of brick updates and node index values modified
    fn erase_node_child<'a, T>(
        &mut self,
        meta_index: usize,
        child_sectant: usize,
        tree: &'a BoxTree<T>,
    ) -> (Vec<BrickUpdate<'a>>, Vec<usize>)
    where
        T: Default + Clone + Eq + VoxelData + Hash,
    {
        let mut modified_bricks = Vec::new();
        let mut modified_nodes = vec![meta_index];
        debug_assert!(
            self.node_key_vs_meta_index.contains_right(&meta_index),
            "Expected parent node to be in metadata index hash! (meta: {meta_index})"
        );
        let parent_key = self
            .node_key_vs_meta_index
            .get_by_right(&meta_index)
            .unwrap();

        debug_assert!(
            tree.nodes.key_is_valid(*parent_key),
            "Expected parent node({:?}) to be valid",
            parent_key
        );

        // Erase connection to parent
        let parent_first_child_index = meta_index * BOX_NODE_CHILDREN_COUNT;
        let parent_children_offset = parent_first_child_index + child_sectant;
        let child_descriptor = self.render_data.node_children[parent_children_offset] as usize;
        debug_assert_ne!(
            child_descriptor,
            empty_marker::<u32>() as usize,
            "Expected erased child[{}] of node[{}] meta[{}] to be an erasable node/brick",
            child_sectant,
            parent_key,
            meta_index
        );

        // Erase child connection
        match tree.nodes.get(*parent_key) {
            NodeContent::Nothing => {
                panic!("HOW DO I ERASE NOTHING. AMERICA EXPLAIN")
            }
            NodeContent::Internal(_) | NodeContent::Leaf(_) => {
                self.render_data.node_children[parent_children_offset] = empty_marker::<u32>();
            }
            NodeContent::UniformLeaf(_) => {
                self.render_data.node_children[parent_first_child_index] = empty_marker::<u32>();
            }
        }

        match tree.nodes.get(*parent_key) {
            NodeContent::Nothing => {
                panic!("HOW DO I ERASE NOTHING. AMERICA EXPLAIN")
            }
            NodeContent::Internal(_occupied_bits) => {
                debug_assert!(
                    self.node_key_vs_meta_index
                        .contains_right(&child_descriptor),
                    "Expected erased child node index[{child_descriptor}] to be in metadata index hash!"
                );
                let child_key = self
                    .node_key_vs_meta_index
                    .get_by_right(&child_descriptor)
                    .unwrap();
                debug_assert!(
                    tree.nodes.key_is_valid(*child_key),
                    "Expected erased child node({child_key}) to be valid"
                );

                // Erase MIP connection, Erase ownership as well
                let child_mip = self.render_data.node_mips[child_descriptor];
                if child_mip != empty_marker::<u32>() {
                    self.render_data.node_mips[child_descriptor] = empty_marker();
                    if matches!(tree.node_mips[*child_key], BrickData::Parted(_)) {
                        self.brick_ownership
                            .insert(child_mip as usize, BrickOwnedBy::NotOwned);
                    }
                }

                modified_nodes.push(child_descriptor);
            }
            NodeContent::UniformLeaf(_) | NodeContent::Leaf(_) => {
                let brick_index = child_descriptor & 0x7FFFFFFF;
                debug_assert!(
                    (0 == child_sectant)
                        || matches!(tree.nodes.get(*parent_key), NodeContent::Leaf(_)),
                    "Expected child sectant in uniform leaf to be 0 in: {:?}",
                    (meta_index, child_sectant)
                );
                if child_descriptor != empty_marker::<u32>() as usize {
                    self.brick_ownership
                        .insert(brick_index, BrickOwnedBy::NotOwned);
                    Self::set_brick_used(&mut self.render_data.used_bits, brick_index, false);
                    modified_bricks.push(BrickUpdate {
                        brick_index,
                        data: None,
                    });
                }
            }
        }

        //return with updated ranges in voxels and metadata
        (modified_bricks, modified_nodes)
    }

    //##############################################################################
    //  ██████   █████    ███████    ██████████   ██████████
    // ░░██████ ░░███   ███░░░░░███ ░░███░░░░███ ░░███░░░░░█
    //  ░███░███ ░███  ███     ░░███ ░███   ░░███ ░███  █ ░
    //  ░███░░███░███ ░███      ░███ ░███    ░███ ░██████
    //  ░███ ░░██████ ░███      ░███ ░███    ░███ ░███░░█
    //  ░███  ░░█████ ░░███     ███  ░███    ███  ░███ ░   █
    //  █████  ░░█████ ░░░███████░   ██████████   ██████████
    // ░░░░░    ░░░░░    ░░░░░░░    ░░░░░░░░░░   ░░░░░░░░░░
    //##############################################################################
    /// Writes the data of the given node to the first available index
    /// * `returns` - Upon success, returns (meta_index, brick updates, modified_nodes)
    pub(crate) fn add_node<
        'a,
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
        #[cfg(all(not(feature = "bytecode"), not(feature = "serialization")))] T: Default + Eq + Clone + Hash + VoxelData,
    >(
        &mut self,
        tree: &'a BoxTree<T>,
        node_key: usize,
    ) -> (usize, CacheUpdatePackage<'a>) {
        debug_assert!(
            !self.node_key_vs_meta_index.contains_left(&node_key)
                || BoxTree::<T>::ROOT_NODE_KEY == node_key as u32,
            "Trying to add already available node twice!"
        );

        // Determine the index in meta, overwrite a currently present node if needed
        let (node_element_index, robbed_parent, modified_usage_range) =
            if BoxTree::<T>::ROOT_NODE_KEY == node_key as u32 {
                (0, None, 0..1)
            } else {
                self.victim_node.first_available_node(&mut self.render_data)
            };
        let (modified_bricks, modified_nodes) = if let Some(robbed_parent) = robbed_parent {
            debug_assert_eq!(
                (self.render_data.node_children
                    [robbed_parent.0 * BOX_NODE_CHILDREN_COUNT + robbed_parent.1 as usize])
                    as usize,
                node_element_index,
                "Expected child[{:?}] of node[{:?}] to be node[{:?}] instead of {:?}*!",
                robbed_parent.1,
                robbed_parent.0,
                node_element_index,
                self.render_data.node_children
                    [robbed_parent.0 * BOX_NODE_CHILDREN_COUNT + robbed_parent.1 as usize]
            );
            self.erase_node_child(robbed_parent.0, robbed_parent.1 as usize, tree)
        } else {
            (Vec::new(), vec![node_element_index])
        };

        // Inject Node properties to render data
        self.node_key_vs_meta_index
            .insert(node_key, node_element_index);
        Self::inject_node_properties(
            &mut self.render_data.node_metadata,
            node_element_index,
            tree,
            node_key,
        );

        // Update occupancy in ocbits
        let child_children_offset = node_element_index * BOX_NODE_CHILDREN_COUNT;
        let occupied_bits = tree.stored_occupied_bits(node_key);
        self.render_data.node_ocbits[node_element_index * 2] =
            (occupied_bits & 0x00000000FFFFFFFF) as u32;
        self.render_data.node_ocbits[node_element_index * 2 + 1] =
            ((occupied_bits & 0xFFFFFFFF00000000) >> 32) as u32;

        // Add empty children
        self.render_data.node_children.splice(
            (child_children_offset)..(child_children_offset + BOX_NODE_CHILDREN_COUNT),
            vec![empty_marker::<u32>(); BOX_NODE_CHILDREN_COUNT],
        );

        // Add MIP entry
        self.render_data.node_mips[node_element_index] = match tree.node_mips[node_key] {
            BrickData::Solid(voxel) => 0x80000000 | voxel, // In case MIP is solid, it is pointing to the color palette
            BrickData::Empty | BrickData::Parted(_) => {
                //TODO: add MIP from ownership
                empty_marker() // parted bricks need to be uploaded; empty MIPS are stored with empty_marker
            }
        };

        // Add child nodes of new child if any is available
        let parent_first_child_index = node_element_index * BOX_NODE_CHILDREN_COUNT;
        match tree.nodes.get(node_key) {
            NodeContent::Nothing => {}
            NodeContent::Internal(_) => {
                for sectant in 0..BOX_NODE_CHILDREN_COUNT {
                    let child_key = tree.node_children[node_key].child(sectant as u8);
                    if child_key != empty_marker::<u32>() as usize {
                        self.render_data.node_children[parent_first_child_index + sectant] = *self
                            .node_key_vs_meta_index
                            .get_by_left(&child_key)
                            .unwrap_or(&(empty_marker::<u32>() as usize))
                            as u32;
                    } else {
                        self.render_data.node_children[parent_first_child_index + sectant] =
                            empty_marker::<u32>();
                    }
                }
            }
            NodeContent::UniformLeaf(brick) => {
                if let BrickData::Solid(voxel) = brick {
                    self.render_data.node_children[parent_first_child_index] = 0x80000000 | *voxel;
                } else {
                    self.render_data.node_children[parent_first_child_index] =
                        empty_marker::<u32>();
                }
            }
            NodeContent::Leaf(bricks) => {
                for (sectant, brick) in bricks.iter().enumerate().take(BOX_NODE_CHILDREN_COUNT) {
                    if let BrickData::Solid(voxel) = brick {
                        self.render_data.node_children[parent_first_child_index + sectant] =
                            0x80000000 | voxel;
                    } else {
                        let node_entry = BrickOwnedBy::NodeAsChild(node_key as u32, sectant as u8);
                        let brick_ownership =
                            self.brick_ownership.get_by_right(&node_entry).cloned();
                        if let Some(brick_index) = brick_ownership {
                            self.render_data.node_children[parent_first_child_index + sectant] =
                                0x7FFFFFFF & brick_index as u32;
                            self.brick_ownership.insert(brick_index, node_entry);
                            Self::set_brick_used(
                                &mut self.render_data.used_bits,
                                brick_index,
                                true,
                            );
                        } else {
                            self.render_data.node_children[parent_first_child_index + sectant] =
                                empty_marker::<u32>();
                        }
                    }
                }
            }
        }
        (
            node_element_index,
            CacheUpdatePackage {
                brick_updates: modified_bricks,
                modified_usage_range,
                modified_nodes,
            },
        )
    }

    //##############################################################################
    //  ███████████  ███████████   █████   █████████  █████   ████
    // ░░███░░░░░███░░███░░░░░███ ░░███   ███░░░░░███░░███   ███░
    //  ░███    ░███ ░███    ░███  ░███  ███     ░░░  ░███  ███
    //  ░██████████  ░██████████   ░███ ░███          ░███████
    //  ░███░░░░░███ ░███░░░░░███  ░███ ░███          ░███░░███
    //  ░███    ░███ ░███    ░███  ░███ ░░███     ███ ░███ ░░███
    //  ███████████  █████   █████ █████ ░░█████████  █████ ░░████
    // ░░░░░░░░░░░  ░░░░░   ░░░░░ ░░░░░   ░░░░░░░░░  ░░░░░   ░░░░
    //##############################################################################
    /// Provides the index of the first brick available to be overwritten, through the second chance algorithm
    /// * `returns` - The index of the first erasable brick inside the cache and the range of bricks updated
    fn first_available_brick(&mut self) -> (usize, Range<usize>) {
        let mut brick_index;
        let mut index_range = self.victim_brick..(self.victim_brick + 1);
        loop {
            brick_index = self.victim_brick;
            index_range.start = index_range.start.min(brick_index);
            index_range.end = index_range.end.max(brick_index + 1);
            if BrickOwnedBy::NotOwned
                == *self
                    .brick_ownership
                    .get_by_left(&brick_index)
                    .unwrap_or(&BrickOwnedBy::NotOwned)
                || !Self::get_brick_used(&self.render_data.used_bits, brick_index)
            {
                Self::set_brick_used(&mut self.render_data.used_bits, brick_index, true);
                break;
            }

            // mark current brick to be deleted at next encounter and step the iterator forward
            Self::set_brick_used(&mut self.render_data.used_bits, brick_index, false);
            self.victim_brick = (brick_index + 1) % (self.render_data.used_bits.len() * 31);
        }
        (brick_index, index_range)
    }

    /// Makes space for the requested brick and updates brick ownership if needed
    /// * `tree` - The boxtree where the brick is found
    /// * `node_key` - The key for the requested leaf node, whoose child needs to be uploaded
    /// * `target_sectant` - The sectant where the target brick lies
    /// * `returns` - child descriptor, brick updates applied, nodes updated during insertion
    pub(crate) fn add_brick<'a, T>(
        &mut self,
        tree: &'a BoxTree<T>,
        node_key: usize,
        target_sectant: u8,
    ) -> (usize, CacheUpdatePackage<'a>)
    where
        T: Default + Clone + Eq + Send + Sync + Hash + VoxelData + 'static,
    {
        // In case OOB sectant, the target brick to add is the MIP for the node
        let (brick, node_entry) = if target_sectant == OOB_SECTANT {
            (
                &tree.node_mips[node_key],
                BrickOwnedBy::NodeAsMIP(node_key as u32),
            )
        } else {
            (
                match tree.nodes.get(node_key) {
                    NodeContent::UniformLeaf(brick) => brick,
                    NodeContent::Leaf(bricks) => &bricks[target_sectant as usize],
                    NodeContent::Nothing | NodeContent::Internal(_) => {
                        panic!("Trying to add brick from Internal or empty node!")
                    }
                },
                BrickOwnedBy::NodeAsChild(node_key as u32, target_sectant),
            )
        };

        match brick {
            BrickData::Empty => (
                empty_marker::<u32>() as usize,
                CacheUpdatePackage::default(),
            ),
            BrickData::Solid(_voxel) => unreachable!("Shouldn't try to upload solid MIP bricks"),
            BrickData::Parted(brick) => {
                let (brick_index, modified_brick_range) = self.first_available_brick();
                let (mut modified_bricks, modified_nodes) = match *self
                    .brick_ownership
                    .get_by_left(&brick_index)
                    .unwrap_or(&BrickOwnedBy::NotOwned)
                {
                    BrickOwnedBy::NodeAsChild(key, sectant) => {
                        if self
                            .node_key_vs_meta_index
                            .get_by_left(&(key as usize))
                            .is_some()
                        {
                            self.erase_node_child(
                                *self
                                    .node_key_vs_meta_index
                                    .get_by_left(&(key as usize))
                                    .unwrap(),
                                sectant as usize,
                                tree,
                            )
                        } else {
                            (Vec::new(), Vec::new())
                        }
                    }
                    BrickOwnedBy::NodeAsMIP(key) => {
                        // erase MIP from node if present
                        if self
                            .node_key_vs_meta_index
                            .get_by_left(&(key as usize))
                            .is_some()
                        {
                            let robbed_meta_index = *self
                                .node_key_vs_meta_index
                                .get_by_left(&(key as usize))
                                .unwrap();
                            self.render_data.node_mips[robbed_meta_index] = empty_marker();
                            (Vec::new(), vec![robbed_meta_index])
                        } else {
                            (Vec::new(), Vec::new())
                        }
                    }
                    BrickOwnedBy::NotOwned => (Vec::new(), Vec::new()),
                };

                self.brick_ownership.insert(brick_index, node_entry);

                debug_assert_eq!(
                    tree.brick_dim.pow(3) as usize,
                    brick.len(),
                    "Expected Brick slice to align to tree brick dimension"
                );
                modified_bricks.push(BrickUpdate {
                    brick_index,
                    data: Some(&brick[..]),
                });

                (
                    0x7FFFFFFF & brick_index, // Child descriptor for parted brick as described in @node_children
                    CacheUpdatePackage {
                        brick_updates: modified_bricks,
                        modified_usage_range: Range {
                            start: modified_brick_range.start / 31,
                            end: (modified_brick_range.end / 31 + 1)
                                .min(self.render_data.used_bits.len()),
                        },
                        modified_nodes,
                    },
                )
            }
        }
    }
}
