pub mod types;
pub mod update;

mod detail;
pub(crate) mod mipmap;
mod node;

#[cfg(test)]
mod tests;

pub use crate::spatial::math::vector::{V3c, V3cf32};
pub use types::{
    Albedo, BoxTree, BoxTreeEntry, MIPMapStrategy, MIPResamplingMethods, StrategyUpdater, VoxelData,
};

use crate::{
    boxtree::{
        detail::child_sectant_for,
        types::{BrickData, NodeChildren, NodeContent, OctreeError, PaletteIndexValues},
    },
    object_pool::{empty_marker, ObjectPool},
    spatial::{
        math::{flat_projection, matrix_index_for},
        Cube,
    },
};
use std::{collections::HashMap, hash::Hash, path::Path};

#[cfg(feature = "serialization")]
use serde::{de::DeserializeOwned, Serialize};

#[cfg(feature = "bytecode")]
use bendy::{decoding::FromBencode, encoding::ToBencode};

//####################################################################################
//     ███████      █████████  ███████████ ███████████   ██████████ ██████████
//   ███░░░░░███   ███░░░░░███░█░░░███░░░█░░███░░░░░███ ░░███░░░░░█░░███░░░░░█
//  ███     ░░███ ███     ░░░ ░   ░███  ░  ░███    ░███  ░███  █ ░  ░███  █ ░
// ░███      ░███░███             ░███     ░██████████   ░██████    ░██████
// ░███      ░███░███             ░███     ░███░░░░░███  ░███░░█    ░███░░█
// ░░███     ███ ░░███     ███    ░███     ░███    ░███  ░███ ░   █ ░███ ░   █
//  ░░░███████░   ░░█████████     █████    █████   █████ ██████████ ██████████
//    ░░░░░░░      ░░░░░░░░░     ░░░░░    ░░░░░   ░░░░░ ░░░░░░░░░░ ░░░░░░░░░░
//  ██████████ ██████   █████ ███████████ ███████████   █████ █████
// ░░███░░░░░█░░██████ ░░███ ░█░░░███░░░█░░███░░░░░███ ░░███ ░░███
//  ░███  █ ░  ░███░███ ░███ ░   ░███  ░  ░███    ░███  ░░███ ███
//  ░██████    ░███░░███░███     ░███     ░██████████    ░░█████
//  ░███░░█    ░███ ░░██████     ░███     ░███░░░░░███    ░░███
//  ░███ ░   █ ░███  ░░█████     ░███     ░███    ░███     ░███
//  ██████████ █████  ░░█████    █████    █████   █████    █████
// ░░░░░░░░░░ ░░░░░    ░░░░░    ░░░░░    ░░░░░   ░░░░░    ░░░░░
//####################################################################################
impl<'a, T: VoxelData> From<(&'a Albedo, &'a T)> for BoxTreeEntry<'a, T> {
    fn from((albedo, data): (&'a Albedo, &'a T)) -> Self {
        BoxTreeEntry::Complex(albedo, data)
    }
}

#[macro_export]
macro_rules! voxel_data {
    ($data:expr) => {
        BoxTreeEntry::Informative($data)
    };
    () => {
        BoxTreeEntry::Empty
    };
}

impl<'a, T: VoxelData> From<&'a Albedo> for BoxTreeEntry<'a, T> {
    fn from(albedo: &'a Albedo) -> Self {
        BoxTreeEntry::Visual(albedo)
    }
}

impl<'a, T: VoxelData> BoxTreeEntry<'a, T> {
    pub fn albedo(&self) -> Option<&'a Albedo> {
        match self {
            BoxTreeEntry::Empty => None,
            BoxTreeEntry::Visual(albedo) => Some(albedo),
            BoxTreeEntry::Informative(_) => None,
            BoxTreeEntry::Complex(albedo, _) => Some(albedo),
        }
    }

    pub fn data(&self) -> Option<&'a T> {
        match self {
            BoxTreeEntry::Empty => None,
            BoxTreeEntry::Visual(_) => None,
            BoxTreeEntry::Informative(data) => Some(data),
            BoxTreeEntry::Complex(_, data) => Some(data),
        }
    }

    pub fn is_none(&self) -> bool {
        match self {
            BoxTreeEntry::Empty => true,
            BoxTreeEntry::Visual(albedo) => albedo.is_transparent(),
            BoxTreeEntry::Informative(data) => data.is_empty(),
            BoxTreeEntry::Complex(albedo, data) => albedo.is_transparent() && data.is_empty(),
        }
    }

    pub fn is_some(&self) -> bool {
        !self.is_none()
    }
}

//####################################################################################
//     ███████      █████████  ███████████ ███████████   ██████████ ██████████
//   ███░░░░░███   ███░░░░░███░█░░░███░░░█░░███░░░░░███ ░░███░░░░░█░░███░░░░░█
//  ███     ░░███ ███     ░░░ ░   ░███  ░  ░███    ░███  ░███  █ ░  ░███  █ ░
// ░███      ░███░███             ░███     ░██████████   ░██████    ░██████
// ░███      ░███░███             ░███     ░███░░░░░███  ░███░░█    ░███░░█
// ░░███     ███ ░░███     ███    ░███     ░███    ░███  ░███ ░   █ ░███ ░   █
//  ░░░███████░   ░░█████████     █████    █████   █████ ██████████ ██████████
//    ░░░░░░░      ░░░░░░░░░     ░░░░░    ░░░░░   ░░░░░ ░░░░░░░░░░ ░░░░░░░░░░
//####################################################################################
pub(crate) const OOB_SECTANT: u8 = 64;
pub(crate) const BOX_NODE_DIMENSION: usize = 4;
pub(crate) const BOX_NODE_CHILDREN_COUNT: usize = 64;

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
    /// converts the data structure to a byte representation
    #[cfg(feature = "bytecode")]
    pub fn to_bytes(&self) -> Vec<u8> {
        self.to_bencode()
            .expect("Failed to serialize Octree to Bytes")
    }

    /// parses the data structure from a byte string
    #[cfg(feature = "bytecode")]
    pub fn from_bytes(bytes: Vec<u8>) -> Self {
        Self::from_bencode(&bytes).expect("Failed to serialize Octree from Bytes")
    }

    /// saves the data structure to the given file path
    #[cfg(feature = "bytecode")]
    pub fn save(&self, path: &str) -> Result<(), std::io::Error> {
        use std::fs::File;
        use std::io::Write;
        let mut file = File::create(path)?;
        file.write_all(&self.to_bytes())?;
        Ok(())
    }

    /// loads the data structure from the given file path
    #[cfg(feature = "bytecode")]
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self, std::io::Error> {
        use std::fs::File;
        use std::io::Read;
        let mut file = File::open(path)?;
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)?;
        Ok(Self::from_bytes(bytes))
    }

    /// creates an boxtree with the given size
    /// * `brick_dimension` - must be one of `(2^x)` and smaller than the size of the boxtree
    /// * `size` - must be `brick_dimension * (4^x)`, e.g: brick_dimension == 2 --> size can be 8,32,128...
    pub fn new(size: u32, brick_dimension: u32) -> Result<Self, OctreeError> {
        if 0 == size || (brick_dimension as f32).log(2.0).fract() != 0.0 {
            return Err(OctreeError::InvalidBrickDimension(brick_dimension));
        }
        if brick_dimension > size
            || 0 == size
            || (size as f32 / brick_dimension as f32).log(4.0).fract() != 0.0
        {
            return Err(OctreeError::InvalidSize(size));
        }
        if size < (brick_dimension * BOX_NODE_DIMENSION as u32) {
            return Err(OctreeError::InvalidStructure(
                "Octree size must be larger, than BOX_NODE_DIMENSION * brick dimension".into(),
            ));
        }
        let node_count_estimation = (size / brick_dimension).pow(3);
        let mut nodes = ObjectPool::with_capacity(node_count_estimation.min(1024) as usize);
        let root_node_key = nodes.push(NodeContent::Nothing); // The first element is the root Node
        assert!(root_node_key == 0);
        Ok(Self {
            auto_simplify: true,
            boxtree_size: size,
            brick_dim: brick_dimension,
            nodes,
            node_children: vec![NodeChildren::default()],
            node_mips: vec![BrickData::Empty],
            voxel_color_palette: vec![],
            voxel_data_palette: vec![],
            map_to_color_index_in_palette: HashMap::new(),
            map_to_data_index_in_palette: HashMap::new(),
            mip_map_strategy: MIPMapStrategy::default(),
        })
    }

    /// Getter function for the boxtree
    /// * Returns immutable reference to the data at the given position, if there is any
    pub fn get(&self, position: &V3c<u32>) -> BoxTreeEntry<T> {
        NodeContent::pix_get_ref(
            &self.get_internal(
                Self::ROOT_NODE_KEY as usize,
                Cube::root_bounds(self.boxtree_size as f32),
                position,
            ),
            &self.voxel_color_palette,
            &self.voxel_data_palette,
        )
    }

    /// Internal Getter function for the boxtree, to be able to call get from within the tree itself
    /// * Returns immutable reference to the data of the given node at the given position, if there is any
    fn get_internal(
        &self,
        mut current_node_key: usize,
        mut current_bounds: Cube,
        position: &V3c<u32>,
    ) -> PaletteIndexValues {
        let position = V3c::from(*position);
        if !current_bounds.contains(&position) {
            return empty_marker();
        }

        loop {
            match self.nodes.get(current_node_key) {
                NodeContent::Nothing => return empty_marker(),
                NodeContent::Leaf(bricks) => {
                    // In case brick_dimension == boxtree size, the root node can not be a leaf...
                    debug_assert!(self.brick_dim < self.boxtree_size);

                    // Hash the position to the target child
                    let child_sectant_at_position = child_sectant_for(&current_bounds, &position);

                    // If the child exists, query it for the voxel
                    match &bricks[child_sectant_at_position as usize] {
                        BrickData::Empty => {
                            return empty_marker();
                        }
                        BrickData::Parted(brick) => {
                            current_bounds =
                                Cube::child_bounds_for(&current_bounds, child_sectant_at_position);
                            let mat_index = matrix_index_for(
                                &current_bounds,
                                &V3c::from(position),
                                self.brick_dim,
                            );
                            let mat_index = flat_projection(
                                mat_index.x as usize,
                                mat_index.y as usize,
                                mat_index.z as usize,
                                self.brick_dim as usize,
                            );
                            if !NodeContent::pix_points_to_empty(
                                &brick[mat_index],
                                &self.voxel_color_palette,
                                &self.voxel_data_palette,
                            ) {
                                return brick[mat_index];
                            }
                            return empty_marker();
                        }
                        BrickData::Solid(voxel) => {
                            return *voxel;
                        }
                    }
                }
                NodeContent::UniformLeaf(brick) => match brick {
                    BrickData::Empty => {
                        return empty_marker();
                    }
                    BrickData::Parted(brick) => {
                        let mat_index =
                            matrix_index_for(&current_bounds, &V3c::from(position), self.brick_dim);
                        let mat_index = flat_projection(
                            mat_index.x as usize,
                            mat_index.y as usize,
                            mat_index.z as usize,
                            self.brick_dim as usize,
                        );
                        return brick[mat_index];
                    }
                    BrickData::Solid(voxel) => {
                        return *voxel;
                    }
                },
                NodeContent::Internal(occupied_bits) => {
                    // Hash the position to the target child
                    let child_sectant_at_position = child_sectant_for(&current_bounds, &position);
                    let child_at_position =
                        self.node_children[current_node_key].child(child_sectant_at_position);

                    // There is a valid child at the given position inside the node, recurse into it
                    if self.nodes.key_is_valid(child_at_position as usize) {
                        debug_assert_ne!(
                            0,
                            occupied_bits & (0x01 << child_sectant_at_position),
                            "Node[{:?}] under {:?} \n has a child(node[{:?}]) in sectant[{:?}](global position: {:?}), which is incompatible with the occupancy bitmap: {:#10X}; \n child node: {:?}; child node children: {:?};",
                            current_node_key,
                            current_bounds,
                            self.node_children[current_node_key].child(child_sectant_at_position),
                            child_sectant_at_position,
                            position, occupied_bits,
                            self.nodes.get(self.node_children[current_node_key].child(child_sectant_at_position)),
                            self.node_children[self.node_children[current_node_key].child(child_sectant_at_position)]
                        );
                        current_node_key = child_at_position as usize;
                        current_bounds =
                            Cube::child_bounds_for(&current_bounds, child_sectant_at_position);
                    } else {
                        return empty_marker();
                    }
                }
            }
        }
    }

    /// Tells the radius of the area covered by the boxtree
    pub fn get_size(&self) -> u32 {
        self.boxtree_size
    }

    /// Object to set the MIP map strategy for each MIP level inside the boxtree
    pub fn albedo_mip_map_resampling_strategy(&mut self) -> StrategyUpdater<T> {
        StrategyUpdater(self)
    }
}
