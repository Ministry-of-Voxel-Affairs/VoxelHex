use crate::spatial::lut::SECTANT_OFFSET_LUT;
use crate::{
    boxtree::{
        iterate::MIPResamplingFunction,
        types::{
            BoxTreeEntry, BrickData, MIPMapStrategy, MIPResamplingMethods, NodeChildren,
            NodeContent, PaletteIndexValues, StrategyUpdater,
        },
        Albedo, BoxTree, VoxelData, BOX_NODE_CHILDREN_COUNT, BOX_NODE_DIMENSION,
    },
    object_pool::empty_marker,
    spatial::{
        math::{flat_projection, matrix_index_for, offset_sectant, vector::V3c},
        Cube,
    },
};
use std::collections::HashMap;

impl<T: VoxelData> BoxTree<T> {
    //####################################################################################
    //  ██████   ██████ █████ ███████████
    // ░░██████ ██████ ░░███ ░░███░░░░░███
    //  ░███░█████░███  ░███  ░███    ░███
    //  ░███░░███ ░███  ░███  ░██████████
    //  ░███ ░░░  ░███  ░███  ░███░░░░░░
    //  ░███      ░███  ░███  ░███
    //  █████     █████ █████ █████
    // ░░░░░     ░░░░░ ░░░░░ ░░░░░
    //  █████  █████ ███████████  ██████████     █████████   ███████████ ██████████
    // ░░███  ░░███ ░░███░░░░░███░░███░░░░███   ███░░░░░███ ░█░░░███░░░█░░███░░░░░█
    //  ░███   ░███  ░███    ░███ ░███   ░░███ ░███    ░███ ░   ░███  ░  ░███  █ ░
    //  ░███   ░███  ░██████████  ░███    ░███ ░███████████     ░███     ░██████
    //  ░███   ░███  ░███░░░░░░   ░███    ░███ ░███░░░░░███     ░███     ░███░░█
    //  ░███   ░███  ░███         ░███    ███  ░███    ░███     ░███     ░███ ░   █
    //  ░░████████   █████        ██████████   █████   █████    █████    ██████████
    //   ░░░░░░░░   ░░░░░        ░░░░░░░░░░   ░░░░░   ░░░░░    ░░░░░    ░░░░░░░░░░
    //####################################################################################
    /// Updates the MIP for the given node at the given position. It expects that MIPS of child nodes are up-to-date.
    /// * `node_key` - The node to update teh MIP for
    /// * `node_bounds` - The bounds of the target node
    /// * `position` - The global position in alignment with node bounds
    pub(crate) fn update_mip(&mut self, node_key: usize, node_bounds: &Cube, position: &V3c<u32>) {
        if !self.mip_map_strategy.enabled {
            return;
        }
        debug_assert_eq!(
            0,
            node_bounds.size as u32 % self.brick_dim,
            "Expected node bounds to be the multiple of DIM"
        );

        let mip_level = (node_bounds.size / self.brick_dim as f32).log2() as usize;
        let dominant_bottom =
            if let Some(strategy) = self.mip_map_strategy.resampling_methods.get(&mip_level) {
                matches!(strategy, MIPResamplingMethods::PointFilterBD)
            } else {
                false
            };
        let sampler =
            if let Some(sampler) = self.mip_map_strategy.resampling_methods.get(&mip_level) {
                sampler.clone()
            } else {
                MIPResamplingMethods::default()
            };

        // determine the sampling range
        let mut node = self.nodes.get_mut(node_key);
        let (sample_start, sample_size) = match &node.content {
            NodeContent::Nothing => {
                debug_assert!(
                    matches!(node.children, NodeChildren::NoChildren),
                    "Expected empty node to not have children: {:?}",
                    node.children
                );
                return;
            }
            NodeContent::UniformLeaf(_brick) => {
                if !matches!(node.mip, BrickData::Empty) {
                    // Uniform Leaf nodes do not need a MIP, because their content is equivalent with it
                    node.mip = BrickData::Empty;
                }
                return;
            }
            NodeContent::Leaf(_) => {
                let sample_size = (node_bounds.size as u32 / self.brick_dim)
                    .min(self.brick_dim * BOX_NODE_DIMENSION as u32);
                let sample_start = V3c::from(
                    (*position - (*position % sample_size))
                        * BOX_NODE_DIMENSION as u32
                        * self.brick_dim,
                ) / node_bounds.size;
                let sample_start: V3c<u32> = sample_start.floor().into();
                debug_assert!(
                    sample_start.x + sample_size
                        <= (node_bounds.min_position.x + node_bounds.size) as u32,
                    "Mipmap sampling out of bounds for x component: ({} + {}) > ({} + {})",
                    sample_start.x,
                    sample_size,
                    node_bounds.min_position.x,
                    node_bounds.size
                );
                debug_assert!(
                    sample_start.y + sample_size
                        <= (node_bounds.min_position.y + node_bounds.size) as u32,
                    "Mipmap sampling out of bounds for y component: ({} + {}) > ({} + {})",
                    sample_start.y,
                    sample_size,
                    node_bounds.min_position.y,
                    node_bounds.size
                );
                debug_assert!(
                    sample_start.z + sample_size
                        <= (node_bounds.min_position.z + node_bounds.size) as u32,
                    "Mipmap sampling out of bounds for z component: ({} + {}) > ({} + {})",
                    sample_start.z,
                    sample_size,
                    node_bounds.min_position.z,
                    node_bounds.size
                );
                (sample_start, sample_size)
            }
            NodeContent::Internal if dominant_bottom => {
                let sample_size = node_bounds.size as u32 / self.brick_dim;
                let sample_start = V3c::from(*position - (*position % sample_size));
                let sample_start: V3c<u32> = sample_start.floor().into();
                debug_assert!(
                    sample_start.x + sample_size
                        <= (node_bounds.min_position.x + node_bounds.size) as u32,
                    "Mipmap sampling out of bounds for x component: ({} + {}) > ({} + {})",
                    sample_start.x,
                    sample_size,
                    node_bounds.min_position.x,
                    node_bounds.size
                );
                debug_assert!(
                    sample_start.y + sample_size
                        <= (node_bounds.min_position.y + node_bounds.size) as u32,
                    "Mipmap sampling out of bounds for y component: ({} + {}) > ({} + {})",
                    sample_start.y,
                    sample_size,
                    node_bounds.min_position.y,
                    node_bounds.size
                );
                debug_assert!(
                    sample_start.z + sample_size
                        <= (node_bounds.min_position.z + node_bounds.size) as u32,
                    "Mipmap sampling out of bounds for z component: ({} + {}) > ({} + {})",
                    sample_start.z,
                    sample_size,
                    node_bounds.min_position.z,
                    node_bounds.size
                );
                (sample_start, sample_size)
            }
            NodeContent::Internal => {
                let sample_size = BOX_NODE_DIMENSION as u32;
                let pos_in_bounds = V3c::from(*position) - node_bounds.min_position;
                let sample_start_v1 =
                    pos_in_bounds * BOX_NODE_DIMENSION as f32 * self.brick_dim as f32
                        / node_bounds.size; // Transform into BOX_NODE_DIMENSION*DIM space
                let sample_start_v2: V3c<u32> = sample_start_v1.floor().into();
                let sample_start = sample_start_v2 - (sample_start_v2 % BOX_NODE_DIMENSION as u32); // sample from grid of BOX_NODE_DIMENSION

                debug_assert!(
                    sample_start.x + sample_size <= BOX_NODE_DIMENSION as u32 * self.brick_dim,
                    "Mipmap sampling out of bounds for x component: ({} + {}) > (BOX_NODE_DIMENSION * {})",
                    sample_start.x,
                    sample_size,
                    self.brick_dim
                );
                debug_assert!(
                    sample_start.y + sample_size <= BOX_NODE_DIMENSION as u32 * self.brick_dim,
                    "Mipmap sampling out of bounds for y component: ({} + {}) > (BOX_NODE_DIMENSION * {})",
                    sample_start.y,
                    sample_size,
                    self.brick_dim
                );
                debug_assert!(
                    sample_start.z + sample_size <= BOX_NODE_DIMENSION as u32 * self.brick_dim,
                    "Mipmap sampling out of bounds for z component: ({} + {}) > (BOX_NODE_DIMENSION * {})",
                    sample_start.z,
                    sample_size,
                    self.brick_dim
                );
                (sample_start, sample_size)
            }
        };
        std::mem::drop(node);

        let sampled_color = match self.nodes.get(node_key).content {
            NodeContent::Nothing | NodeContent::UniformLeaf(_) => None,
            NodeContent::Leaf(_) => {
                sampler.execute(&sample_start, sample_size, |pos| -> Option<Albedo> {
                    NodeContent::pix_get_ref(
                        &self.get_internal(node_key, *node_bounds, pos),
                        &self.voxel_color_palette,
                        &self.voxel_data_palette,
                    )
                    .albedo()
                    .copied()
                })
            }
            NodeContent::Internal if dominant_bottom => {
                sampler.execute(&sample_start, sample_size, |pos| -> Option<Albedo> {
                    NodeContent::pix_get_ref(
                        &self.get_internal(node_key, *node_bounds, pos),
                        &self.voxel_color_palette,
                        &self.voxel_data_palette,
                    )
                    .albedo()
                    .copied()
                })
            }
            NodeContent::Internal => {
                sampler.execute(
                    &sample_start,
                    sample_size,
                    |pos_in_parent_mip| -> Option<Albedo> {
                        // Current position spans BOX_NODE_DIMENSION bricks, but in special cases
                        // the brick dimension might be smaller, than the sample size, e.g. when brick_dim == 1
                        // In this case the target child_sectant needs to be updated dynamically to accomodate this
                        // It would be possible to use an if condition to handle when brick_dim == 1
                        // but the performance gain is neglegible
                        let child_sectant = offset_sectant(
                            &((*pos_in_parent_mip).into()),
                            (self.brick_dim * BOX_NODE_DIMENSION as u32) as f32,
                        );

                        if empty_marker::<u32>() as usize
                            == self.nodes.get(node_key).child(child_sectant)
                        {
                            return None;
                        }

                        let pos_in_child_mip = V3c::from(*pos_in_parent_mip)
                            - SECTANT_OFFSET_LUT[child_sectant as usize]
                                * (self.brick_dim * BOX_NODE_DIMENSION as u32) as f32;

                        match &self
                            .nodes
                            .get(self.nodes.get(node_key).child(child_sectant))
                            .mip
                        {
                            BrickData::Empty => None,
                            BrickData::Solid(voxel) => NodeContent::pix_get_ref(
                                voxel,
                                &self.voxel_color_palette,
                                &self.voxel_data_palette,
                            )
                            .albedo()
                            .copied(),
                            BrickData::Parted(brick) => {
                                let mip_index = flat_projection(
                                    pos_in_child_mip.x as usize,
                                    pos_in_child_mip.y as usize,
                                    pos_in_child_mip.z as usize,
                                    self.brick_dim as usize,
                                );
                                NodeContent::pix_get_ref(
                                    &brick[mip_index],
                                    &self.voxel_color_palette,
                                    &self.voxel_data_palette,
                                )
                                .albedo()
                                .copied()
                            }
                        }
                    },
                )
            }
        };

        // Assemble MIP entry
        let mip_entry = if let Some(ref color) = sampled_color {
            if let Some(color_distance_threshold) = self
                .mip_map_strategy
                .resampling_color_matching_thresholds
                .get(&mip_level)
            {
                let mut similar_color = None;
                let color_distance_threshold = color_distance_threshold * 255.;
                for palette_index in 0..self.voxel_color_palette.len() {
                    // if a color if close enoguh i.e. distance is below distance threshold, it will do
                    if color.distance_from(&self.voxel_color_palette[palette_index])
                        < color_distance_threshold
                    {
                        similar_color = Some(palette_index as u16);
                        break;
                    }
                }

                if let Some(similar_color) = similar_color {
                    // Generate Voxel entry from available color
                    Some(
                        // self.add_to_palette(&OctreeEntry::Visual(&similar_color))
                        NodeContent::pix_visual(similar_color),
                    )
                } else {
                    // Add new color to the color palette
                    Some(self.add_to_palette(&BoxTreeEntry::Visual(color)))
                }
            } else {
                // Add new color to the color palette
                Some(self.add_to_palette(&BoxTreeEntry::Visual(color)))
            }
        } else {
            None
        };

        if let Some(mip_entry) = mip_entry {
            // Set MIP entry
            let pos_in_mip = matrix_index_for(node_bounds, position, self.brick_dim);
            let flat_pos_in_mip = flat_projection(
                pos_in_mip.x,
                pos_in_mip.y,
                pos_in_mip.z,
                self.brick_dim as usize,
            );
            let mip = &mut self.nodes.get_mut(node_key).mip;
            match mip {
                BrickData::Empty => {
                    let mut new_brick_data =
                        vec![empty_marker::<PaletteIndexValues>(); self.brick_dim.pow(3) as usize];
                    new_brick_data[flat_pos_in_mip] = mip_entry;
                    *mip = BrickData::Parted(new_brick_data);
                }
                BrickData::Solid(voxel) => {
                    let mut new_brick_data = vec![*voxel; self.brick_dim.pow(3) as usize];
                    new_brick_data[flat_pos_in_mip] = mip_entry;
                    *mip = BrickData::Parted(new_brick_data);
                }
                BrickData::Parted(brick) => {
                    brick[flat_pos_in_mip] = mip_entry;
                }
            }
        }
    }
}

//####################################################################################
//  ██████   ██████ █████ ███████████       █████████   ███████████  █████
// ░░██████ ██████ ░░███ ░░███░░░░░███     ███░░░░░███ ░░███░░░░░███░░███
//  ░███░█████░███  ░███  ░███    ░███    ░███    ░███  ░███    ░███ ░███
//  ░███░░███ ░███  ░███  ░██████████     ░███████████  ░██████████  ░███
//  ░███ ░░░  ░███  ░███  ░███░░░░░░      ░███░░░░░███  ░███░░░░░░   ░███
//  ░███      ░███  ░███  ░███            ░███    ░███  ░███         ░███
//  █████     █████ █████ █████           █████   █████ █████        █████
// ░░░░░     ░░░░░ ░░░░░ ░░░░░           ░░░░░   ░░░░░ ░░░░░        ░░░░░
//####################################################################################
impl Default for MIPMapStrategy {
    fn default() -> Self {
        MIPMapStrategy {
            enabled: false,
            resampling_methods: HashMap::from([
                (1, MIPResamplingMethods::Posterize(0.05)),
                (2, MIPResamplingMethods::BoxFilter),
                (3, MIPResamplingMethods::BoxFilter),
                (4, MIPResamplingMethods::BoxFilter),
            ]),
            resampling_color_matching_thresholds: HashMap::from([(2, 0.1), (3, 0.05), (4, 0.02)]),
        }
    }
}

impl MIPMapStrategy {
    /// To reduce adding new colors during MIP resampling operations, each MIP level has
    /// a color similarity method where colors within the same threshold are recognized as the same
    /// This reduces the number of introduced colors.
    pub fn get_new_color_similarity_at(&self, mip_level: usize) -> f32 {
        self.resampling_color_matching_thresholds
            .get(&mip_level)
            .cloned()
            .unwrap_or(0.)
    }

    /// Checks the value and integrates itinto the object
    pub(crate) fn set_color_similarity_thr_internal(
        &mut self,
        mip_level: usize,
        mut similarity_thr: f32,
    ) {
        debug_assert!(
            (0. ..=1.).contains(&similarity_thr),
            "Color similarity Threshold {similarity_thr} out of range!"
        );
        similarity_thr = similarity_thr.clamp(0., 1.);

        self.resampling_color_matching_thresholds
            .insert(mip_level, similarity_thr);
    }

    /// Sets the similarity threshold for a color to be discarded for a similar, already available color
    /// during MIP resampling operations. This reduces the number of introduced colors by MIP bricks
    pub fn set_color_similarity_thr_at(mut self, mip_level: usize, similarity_thr: f32) -> Self {
        self.set_color_similarity_thr_internal(mip_level, similarity_thr);
        self
    }

    /// Sets the comolr reduction strategy similarity thresholds for each given MIP level
    pub fn set_color_similarity_thr(self, levels: impl IntoIterator<Item = (usize, f32)>) -> Self {
        let mut chain = self;
        for (mip_level, similarity_thr) in levels {
            chain = chain.set_color_similarity_thr_at(mip_level, similarity_thr);
        }
        chain
    }

    /// Provides the strategy for a MIP level during resample operations, if any
    pub fn get_method_at(&self, mip_level: usize) -> MIPResamplingMethods {
        self.resampling_methods
            .get(&mip_level)
            .cloned()
            .unwrap_or(MIPResamplingMethods::BoxFilter)
    }

    pub(crate) fn set_method_at_internal(
        &mut self,
        mip_level: usize,
        mut method: MIPResamplingMethods,
    ) {
        if let MIPResamplingMethods::Posterize(ref mut thr)
        | MIPResamplingMethods::PosterizeBD(ref mut thr) = method
        {
            debug_assert!(
                *thr >= 0. && *thr <= 1.,
                "Posterize Threshold {thr} out of range!"
            );
            *thr = thr.clamp(0., 1.);
        }
        self.resampling_methods.insert(mip_level, method);
    }

    /// Sets the strategy for a MIP level during resample operations
    /// In case method has a parameter, it is clamped to 0. <= thr <= 1.
    pub fn set_method_at(mut self, mip_level: usize, method: MIPResamplingMethods) -> Self {
        self.set_method_at_internal(mip_level, method);
        self
    }

    /// Sets the strategy for a MIP level during resample operations
    pub fn set_method(
        self,
        levels: impl IntoIterator<Item = (usize, MIPResamplingMethods)>,
    ) -> Self {
        let mut chain = self;
        for (mip_level, method) in levels {
            chain = chain.set_method_at(mip_level, method);
        }
        chain
    }

    /// Returns true if MIP maps are enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Enables or disables mipmap feature for albedo values
    pub fn set_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
}

impl<T: VoxelData> StrategyUpdater<'_, T> {
    /// Resets the strategy for MIP maps during resample operations
    pub fn reset(self) -> Self {
        self.0.mip_map_strategy = MIPMapStrategy::default();
        self
    }

    /// To reduce adding new colors during MIP resampling operations, each MIP level has
    /// a color similarity method where colors within the same threshold are recognized as the same
    /// This reduces the number of introduced colors.
    pub fn get_new_color_similarity_at(&self, mip_level: usize) -> f32 {
        self.0
            .mip_map_strategy
            .get_new_color_similarity_at(mip_level)
    }

    /// Sets the similarity threshold for a color to be discarded for a similar, already available color
    /// during MIP resampling operations. This reduces the number of introduced colors by MIP bricks
    pub fn set_color_similarity_thr_at(self, mip_level: usize, similarity_thr: f32) -> Self {
        self.0
            .mip_map_strategy
            .set_color_similarity_thr_internal(mip_level, similarity_thr);
        self
    }

    /// Sets the comolr reduction strategy similarity thresholds for each given MIP level
    pub fn set_color_similarity_thr(self, levels: impl IntoIterator<Item = (usize, f32)>) -> Self {
        let mut chain = self;
        for (mip_level, similarity_thr) in levels {
            chain = chain.set_color_similarity_thr_at(mip_level, similarity_thr);
        }
        chain
    }

    /// Provides the strategy for a MIP level during resample operations, if any
    pub fn get_method_at(&self, mip_level: usize) -> MIPResamplingMethods {
        self.0.mip_map_strategy.get_method_at(mip_level)
    }

    /// Sets the strategy for a MIP level during resample operations
    /// In case method has a parameter, it is clamped to 0. <= thr <= 1.
    pub fn set_method_at(self, mip_level: usize, method: MIPResamplingMethods) -> Self {
        self.0
            .mip_map_strategy
            .set_method_at_internal(mip_level, method);
        self
    }

    /// Sets the strategy for a MIP level during resample operations
    pub fn set_method(
        self,
        levels: impl IntoIterator<Item = (usize, MIPResamplingMethods)>,
    ) -> Self {
        let mut chain = self;
        for (mip_level, method) in levels {
            chain = chain.set_method_at(mip_level, method)
        }
        chain
    }

    //####################################################################################
    // ██████   ██████ █████ ███████████
    // ░░██████ ██████ ░░███ ░░███░░░░░███
    //  ░███░█████░███  ░███  ░███    ░███
    //  ░███░░███ ░███  ░███  ░██████████
    //  ░███ ░░░  ░███  ░███  ░███░░░░░░
    //  ░███      ░███  ░███  ░███
    //  █████     █████ █████ █████
    // ░░░░░     ░░░░░ ░░░░░ ░░░░░
    //  ███████████   ██████████   █████████    █████████   █████         █████████
    // ░░███░░░░░███ ░░███░░░░░█  ███░░░░░███  ███░░░░░███ ░░███         ███░░░░░███
    //  ░███    ░███  ░███  █ ░  ███     ░░░  ░███    ░███  ░███        ███     ░░░
    //  ░██████████   ░██████   ░███          ░███████████  ░███       ░███
    //  ░███░░░░░███  ░███░░█   ░███          ░███░░░░░███  ░███       ░███
    //  ░███    ░███  ░███ ░   █░░███     ███ ░███    ░███  ░███      █░░███     ███
    //  █████   █████ ██████████ ░░█████████  █████   █████ ███████████ ░░█████████
    // ░░░░░   ░░░░░ ░░░░░░░░░░   ░░░░░░░░░  ░░░░░   ░░░░░ ░░░░░░░░░░░   ░░░░░░░░░
    //####################################################################################
    /// Recalculates MIPs for the whole content of the boxtree
    pub fn recalculate_mips(&mut self) {
        // Generating MIPs need to happen while traveling the graph in a DFS manner
        // in order to generate MIPs for the leaf nodes first
        let mut node_stack = vec![(
            BoxTree::<T>::ROOT_NODE_KEY as usize,
            Cube::root_bounds(self.0.boxtree_size as f32),
            0u8,
        )];
        while !node_stack.is_empty() {
            let tree = &mut self.0;
            let (current_node_key, current_bounds, target_sectant) = node_stack.last().unwrap();

            // evaluate current node and return to its parent node
            if BOX_NODE_CHILDREN_COUNT <= *target_sectant as usize {
                self.recalculate_mip(*current_node_key, current_bounds);
                node_stack.pop();
                if let Some(parent) = node_stack.last_mut() {
                    parent.2 += 1;
                }
                continue;
            }

            match tree.nodes.get(*current_node_key).content {
                NodeContent::Nothing => unreachable!("BFS shouldn't evaluate empty children"),
                NodeContent::Internal => {
                    let target_child_key = tree.nodes.get(*current_node_key).child(*target_sectant);
                    if tree.nodes.key_is_valid(target_child_key)
                        && !matches!(
                            tree.nodes.get(target_child_key).content,
                            NodeContent::Nothing
                        )
                    {
                        node_stack.push((
                            target_child_key,
                            current_bounds.child_bounds_for(*target_sectant),
                            0,
                        ));
                    } else {
                        node_stack.last_mut().unwrap().2 += 1;
                    }
                }
                NodeContent::Leaf(_) | NodeContent::UniformLeaf(_) => {
                    // Set current child iterator to OOB, to evaluate it and move on
                    node_stack.last_mut().unwrap().2 = BOX_NODE_CHILDREN_COUNT as u8;
                }
            }
        }
    }

    /// Enables or disables mipmap feature for albedo values
    pub fn switch_albedo_mip_maps(mut self, enabled: bool) -> Self {
        let tree = &mut self.0;
        let mips_on_previously = tree.mip_map_strategy.enabled;
        tree.mip_map_strategy.enabled = enabled;

        // go through every node and set its mip-maps in case the feature is just enabled
        // and if there's anything to iterate into
        if tree.mip_map_strategy.enabled
            && mips_on_previously != enabled
            && tree.nodes.get(BoxTree::<T>::ROOT_NODE_KEY as usize).content != NodeContent::Nothing
        {
            self.recalculate_mips();
        }
        self
    }

    /// Resamples every voxel for the MIP of the given node
    pub(crate) fn recalculate_mip(&mut self, node_key: usize, node_bounds: &Cube) {
        let tree = &mut self.0;
        if !tree.mip_map_strategy.enabled {
            return;
        }

        tree.nodes.get_mut(node_key).mip = BrickData::Empty;
        for x in 0..tree.brick_dim {
            for y in 0..tree.brick_dim {
                for z in 0..tree.brick_dim {
                    let pos: V3c<f32> = node_bounds.min_position
                        + (V3c::<f32>::new(x as f32, y as f32, z as f32) * node_bounds.size
                            / tree.brick_dim as f32)
                            .round();
                    tree.update_mip(node_key, node_bounds, &V3c::from(pos));
                }
            }
        }
    }

    #[cfg(test)]
    /// Sample the MIP of the root node, or its children
    /// * `sectant` - the child to sample, in case `BOX_NODE_CHILDREN_COUNT` the root MIP is sampled
    /// * `position` - the position inside the MIP, expected to be in range `0..self.brick_dim` for all components
    pub(crate) fn sample_root_mip(
        &'_ self,
        sectant: u8,
        position: &V3c<u32>,
    ) -> BoxTreeEntry<'_, T> {
        let tree = &self.0;
        let node_key: usize = if BOX_NODE_CHILDREN_COUNT <= sectant as usize {
            BoxTree::<T>::ROOT_NODE_KEY as usize
        } else {
            tree.nodes
                .get(BoxTree::<T>::ROOT_NODE_KEY as usize)
                .child(sectant) as usize
        };

        if !tree.nodes.key_is_valid(node_key) {
            return BoxTreeEntry::Empty;
        }
        match &tree.nodes.get(node_key).mip {
            BrickData::Empty => BoxTreeEntry::Empty,
            BrickData::Solid(voxel) => NodeContent::pix_get_ref(
                &voxel,
                &tree.voxel_color_palette,
                &tree.voxel_data_palette,
            ),
            BrickData::Parted(brick) => {
                let flat_index = flat_projection(
                    position.x as usize,
                    position.y as usize,
                    position.z as usize,
                    tree.brick_dim as usize,
                );
                NodeContent::pix_get_ref(
                    &brick[flat_index],
                    &tree.voxel_color_palette,
                    &tree.voxel_data_palette,
                )
            }
        }
    }
}
