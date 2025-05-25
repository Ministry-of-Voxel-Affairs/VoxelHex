use crate::{
    boxtree::{
        types::{MIPMapStrategy, OctreeError},
        Albedo, BoxTree, BoxTreeEntry, V3c, VoxelData,
    },
    spatial::math::{convert_coordinate, CoordinateSystemType},
};
use dot_vox::{Color, DotVoxData, Model, SceneNode, Size, Voxel};
use nalgebra::Matrix3;
use num_traits::Num;
use std::{convert::From, hash::Hash, path::Path};

#[cfg(feature = "serialization")]
use serde::{de::DeserializeOwned, Serialize};

#[cfg(feature = "bytecode")]
use bendy::{decoding::FromBencode, encoding::ToBencode};

impl From<Albedo> for Color {
    fn from(color: Albedo) -> Self {
        Self {
            r: color.r,
            g: color.g,
            b: color.b,
            a: color.a,
        }
    }
}

impl From<Color> for Albedo {
    fn from(color: Color) -> Self {
        Self {
            r: color.r,
            g: color.g,
            b: color.b,
            a: color.a,
        }
    }
}

impl From<Voxel> for V3c<i32> {
    fn from(other: Voxel) -> Self {
        Self {
            x: other.x as i32,
            y: other.y as i32,
            z: other.z as i32,
        }
    }
}

impl From<Size> for V3c<i32> {
    fn from(other: Size) -> Self {
        Self {
            x: other.x as i32,
            y: other.y as i32,
            z: other.z as i32,
        }
    }
}

/// Gives the size of a tree which fits the given model size
pub fn model_size_to_tree_size(model_size: &V3c<i32>, brick_dimension: u32) -> u32 {
    let model_size = model_size.x.max(model_size.y).max(model_size.z);
    let tree_size = (model_size as f32 / brick_dimension as f32).log(4.).ceil() as u32;
    4_u32.pow(tree_size) * brick_dimension
}

/// Converts the given byte value to a rotation matrix
/// Rotation matrix in voxel context enables 90 degr rotations only, so the contents of the matrix is restricted to 0,1,-1
/// Takes into consideration, that the stored matrix is row-major, while Matrix3 storage is column major
fn parse_rotation_matrix(b: u8) -> Matrix3<i8> {
    let mut result = Matrix3::<i8>::new(0, 0, 0, 0, 0, 0, 0, 0, 0);

    // decide absolute values of each row
    let index_in_first_row = b & 0x3;
    let index_in_second_row = ((b & (0x3 << 2)) >> 2) & 0x3;
    let index_in_third_row = !(index_in_first_row ^ index_in_second_row) & 0x3;
    debug_assert!(index_in_first_row < 3);
    debug_assert!(index_in_second_row < 3);
    debug_assert!(index_in_third_row < 3);
    debug_assert!(index_in_first_row != index_in_second_row);
    debug_assert!(index_in_first_row != index_in_third_row);
    debug_assert!(index_in_second_row != index_in_third_row);

    // decide the sign of the values
    let sign_first_row = if 0 == (b & 0x10) { 1 } else { -1 };
    let sign_second_row = if 0 == (b & 0x20) { 1 } else { -1 };
    let sign_third_row = if 0 == (b & 0x40) { 1 } else { -1 };

    // set the values in the matrix
    result.data.0[index_in_first_row as usize][0] = sign_first_row;
    result.data.0[index_in_second_row as usize][1] = sign_second_row;
    result.data.0[index_in_third_row as usize][2] = sign_third_row;

    result
}

impl<T> V3c<T>
where
    T: Num + Clone + Copy + From<i8>,
{
    fn transformed(self, matrix: &Matrix3<i8>) -> Self {
        V3c::new(
            self.x * matrix.m11.into() + self.y * matrix.m12.into() + self.z * matrix.m13.into(),
            self.x * matrix.m21.into() + self.y * matrix.m22.into() + self.z * matrix.m23.into(),
            self.x * matrix.m31.into() + self.y * matrix.m32.into() + self.z * matrix.m33.into(),
        )
    }
}

/// Iterates the given dot_vox data and calls the given function on every model in the scene
fn iterate_vox_tree<F: FnMut(&Model, &V3c<i32>, &Matrix3<i8>)>(
    vox_tree: &DotVoxData,
    frame: usize,
    mut fun: F,
) {
    let mut node_stack: Vec<(u32, V3c<i32>, Matrix3<i8>, u32)> = Vec::new();

    match &vox_tree.scenes[0] {
        SceneNode::Transform {
            attributes: _,
            frames: _,
            child,
            layer_id: _,
        } => {
            node_stack.push((*child, V3c::unit(0), Matrix3::identity(), 0));
        }
        _ => {
            panic!("The root node for a magicka voxel DAG should be a translation")
        }
    }

    while !node_stack.is_empty() {
        let (current_node, translation, rotation, index) = *node_stack.last().unwrap();
        match &vox_tree.scenes[current_node as usize] {
            SceneNode::Transform {
                attributes: _,
                frames,
                child,
                layer_id: _,
            } => {
                let used_frame = if frame < frames.len() { frame } else { 0 };
                let translation = if let Some(t) = frames[used_frame].attributes.get("_t") {
                    translation
                        + t.split(" ")
                            .map(|x| x.parse().expect("Not an integer!"))
                            .collect::<Vec<i32>>()
                            .into()
                } else {
                    translation
                };
                let orientation = if let Some(r) = frames[used_frame].attributes.get("_r") {
                    rotation
                        * parse_rotation_matrix(
                            r.parse()
                                .expect("Expected valid u8 byte to parse rotation matrix"),
                        )
                } else {
                    Matrix3::identity()
                };
                // the index variable for a Transform stores whether to go above or below a level next
                if 0 == index {
                    // 0 == index ==> iterate into the child of the translation
                    node_stack.last_mut().unwrap().3 += 1;
                    node_stack.push((*child, translation, orientation, 0));
                } else {
                    // 0 != index ==> remove translation and iterate into parent
                    node_stack.pop();
                }
            }
            SceneNode::Group {
                attributes: _,
                children,
            } => {
                if (index as usize) < children.len() {
                    node_stack.last_mut().unwrap().3 += 1;
                    node_stack.push((children[index as usize], translation, rotation, 0));
                } else {
                    node_stack.pop();
                }
            }
            SceneNode::Shape {
                attributes: _,
                models,
            } => {
                for model in models {
                    if model
                        .attributes
                        .get("_f")
                        .unwrap_or(&"0".to_string())
                        .parse::<usize>()
                        .expect("Expected frame attribute of Voxel Model to be a parsable integer")
                        == frame
                    {
                        fun(
                            &vox_tree.models[model.model_id as usize],
                            &translation,
                            &rotation,
                        );
                    }
                }
                node_stack.pop();
                if let Some(parent) = node_stack.last_mut() {
                    parent.3 += 1;
                }
            }
        }
    }
}

impl MIPMapStrategy {
    pub fn load_vox_file<
        P: AsRef<Path>,
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
    >(
        self,
        brick_dimension: u32,
        filename: &str,
    ) -> Result<BoxTree<T>, &'static str> {
        let (vox_data, min_position, mut max_position) =
            BoxTree::<T>::load_vox_file_internal(filename);
        max_position -= min_position;
        let tree_size = model_size_to_tree_size(&max_position, brick_dimension);

        let mut shocovox_boxtree =
            BoxTree::<T>::new(tree_size, brick_dimension).unwrap_or_else(|err| {
                panic!(
                    "Expected to build a valid boxtree with dimension {:?} and brick dimension {:?}; Instead: {:?}",
                    tree_size,
                    brick_dimension,
                    err
                )
            });

        shocovox_boxtree.mip_map_strategy.enabled = self.enabled;
        shocovox_boxtree.mip_map_strategy.resampling_methods = self.resampling_methods.clone();
        shocovox_boxtree
            .mip_map_strategy
            .resampling_color_matching_thresholds =
            self.resampling_color_matching_thresholds.clone();
        shocovox_boxtree.load_vox_data_internal(&vox_data, &min_position);
        Ok(shocovox_boxtree)
    }
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
            + VoxelData,
        #[cfg(all(feature = "bytecode", not(feature = "serialization")))] T: FromBencode + ToBencode + Default + Eq + Clone + Hash + VoxelData,
        #[cfg(all(not(feature = "bytecode"), feature = "serialization"))] T: Serialize + DeserializeOwned + Default + Eq + Clone + Hash + VoxelData,
        #[cfg(all(not(feature = "bytecode"), not(feature = "serialization")))] T: Default + Eq + Clone + Hash + VoxelData,
    > BoxTree<T>
{
    pub fn load_vox_file(filename: &str, brick_dimension: u32) -> Result<Self, &'static str> {
        let (vox_data, min_position, mut max_position) = Self::load_vox_file_internal(filename);
        max_position -= min_position;
        let tree_size = model_size_to_tree_size(&max_position, brick_dimension);

        let mut shocovox_boxtree =
            BoxTree::<T>::new(tree_size, brick_dimension).unwrap_or_else(|err| {
                panic!(
                    "Expected to build a valid boxtree with dimension {:?} and brick dimension {:?}; Instead: {:?}",
                    tree_size,
                    brick_dimension.to_owned(),
                    err
                )
            });

        shocovox_boxtree.load_vox_data_internal(&vox_data, &min_position);
        Ok(shocovox_boxtree)
    }

    /// Loads data from the given filename
    /// * `returns` - (file_data, voxel_minimum_position_lyup, voxel_maximum_position_lyup)
    pub(crate) fn load_vox_file_internal<P: AsRef<Path>>(
        filename: P,
    ) -> (DotVoxData, V3c<i32>, V3c<i32>) {
        let vox_tree = dot_vox::load(
            filename
                .as_ref()
                .to_str()
                .expect("Expected {filename} to be interpreted as `&str`!"),
        )
        .expect(
            &format!(
                "Expected file {:?} to be a valid .vox file!",
                filename
                    .as_ref()
                    .to_str()
                    .unwrap_or("path_name_conversion_failed")
            )
            .to_owned(),
        );

        let mut min_position_rzup = V3c::<i32>::new(i32::MAX, i32::MAX, i32::MAX);
        let mut max_position_rzup = V3c::<i32>::new(i32::MIN, i32::MIN, i32::MIN);
        iterate_vox_tree(&vox_tree, 0, |model, model_position_rzup, orientation| {
            let model_size_half_rzup = V3c::from(model.size).transformed(orientation) / 2;
            min_position_rzup.x = min_position_rzup
                .x
                .min(model_position_rzup.x - model_size_half_rzup.x)
                .min(model_position_rzup.x + model_size_half_rzup.x);
            min_position_rzup.y = min_position_rzup
                .y
                .min(model_position_rzup.y - model_size_half_rzup.y)
                .min(model_position_rzup.y + model_size_half_rzup.y);
            min_position_rzup.z = min_position_rzup
                .z
                .min(model_position_rzup.z - model_size_half_rzup.z)
                .min(model_position_rzup.z + model_size_half_rzup.z);

            max_position_rzup.x = max_position_rzup
                .x
                .max(model_position_rzup.x + model_size_half_rzup.x)
                .max(model_position_rzup.x - model_size_half_rzup.x);
            max_position_rzup.y = max_position_rzup
                .y
                .max(model_position_rzup.y + model_size_half_rzup.y)
                .max(model_position_rzup.y - model_size_half_rzup.y);
            max_position_rzup.z = max_position_rzup
                .z
                .max(model_position_rzup.z + model_size_half_rzup.z)
                .max(model_position_rzup.z - model_size_half_rzup.z);
        });

        (
            vox_tree,
            convert_coordinate(
                min_position_rzup,
                CoordinateSystemType::Rzup,
                CoordinateSystemType::Lyup,
            ),
            convert_coordinate(
                max_position_rzup,
                CoordinateSystemType::Rzup,
                CoordinateSystemType::Lyup,
            ),
        )
    }

    pub(crate) fn load_vox_data_internal(
        &mut self,
        vox_tree: &DotVoxData,
        min_position_lyup: &V3c<i32>,
    ) {
        let auto_simplify_enabled = self.auto_simplify;
        self.auto_simplify = false;

        let min_position_rzup = convert_coordinate(
            *min_position_lyup,
            CoordinateSystemType::Lyup,
            CoordinateSystemType::Rzup,
        );
        iterate_vox_tree(vox_tree, 0, |model, position_rzup, orientation| {
            let model_size_half_rzup = V3c::from(model.size).transformed(orientation) / 2;
            let model_bottom_left_rzup = *position_rzup - model_size_half_rzup - min_position_rzup
                // If the index delta is negative(because of orientation),
                // voxel is set based on model[size - i - 1][..][..], instead of model[i][..][..]
                // this requires a correction in every dimension where the index is below 0
                + V3c::new(
                    if model_size_half_rzup.x < 0 { -1 } else { 0 },
                    if model_size_half_rzup.y < 0 { -1 } else { 0 },
                    if model_size_half_rzup.z < 0 { -1 } else { 0 },
                );
            for voxel in &model.voxels {
                let voxel_position_lyup = convert_coordinate(
                    model_bottom_left_rzup + V3c::from(*voxel).transformed(orientation),
                    CoordinateSystemType::Rzup,
                    CoordinateSystemType::Lyup,
                );
                match self.insert(
                    &V3c::from(voxel_position_lyup),
                    BoxTreeEntry::Visual(&(vox_tree.palette[voxel.i as usize].into())),
                ) {
                    Ok(_) => {}
                    Err(boxtree_error) => match boxtree_error {
                        OctreeError::InvalidPosition { .. } => {
                            panic!(
                                "inserting into boxtree at at invalid position: {:?}",
                                boxtree_error
                            )
                        }
                        _ => panic!("inserting into boxtree yielded: {:?}", boxtree_error),
                    },
                }
            }
        });

        if auto_simplify_enabled {
            self.simplify(Self::ROOT_NODE_KEY as usize, true);
            self.auto_simplify = auto_simplify_enabled;
        }
    }
}

#[cfg(test)]
mod boxtree_tests {
    use super::parse_rotation_matrix;
    use nalgebra::Matrix3;

    #[test]
    fn test_matrix_parse() {
        let test_matrix = Matrix3::<i8>::new(1, 0, 0, 0, 1, 0, 0, 0, 1);
        assert!(test_matrix.m11 == 1);
        assert!(test_matrix.m22 == 1);
        assert!(test_matrix.m33 == 1);

        let parsed_matrix = parse_rotation_matrix(4);
        assert!(parse_rotation_matrix(4) == test_matrix);
        assert!(parsed_matrix.m11 == 1);
        assert!(parsed_matrix.m22 == 1);
        assert!(parsed_matrix.m33 == 1);

        // https://github.com/ephtracy/voxel-model/blob/master/MagicaVoxel-file-format-vox-extension.txt
        let example = Matrix3::<i8>::new(0, 1, 0, 0, 0, -1, -1, 0, 0);
        let parsed_example =
            parse_rotation_matrix((1 << 0) | (2 << 2) | (0 << 4) | (1 << 5) | (1 << 6));
        assert!(parsed_example == example);
        assert!(parsed_example.m12 == 1);
        assert!(parsed_example.m23 == -1);
        assert!(parsed_example.m31 == -1);
    }
}
