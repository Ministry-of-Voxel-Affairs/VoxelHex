use crate::{
    boxtree::{Albedo, BoxTree, BoxTreeEntry, BOX_NODE_CHILDREN_COUNT},
    spatial::{lut::SECTANT_OFFSET_LUT, math::vector::V3c},
    voxel_data,
};
use num_traits::Zero;

#[test]
fn test_simplest_insert_and_get() {
    let red: Albedo = 0xFF000001.into();
    let mut tree: BoxTree = BoxTree::new(4, 1).ok().unwrap();
    tree.auto_simplify = false;
    tree.insert(&V3c::new(0, 0, 0), &red)
        .expect("boxtree insert");

    assert!(tree.get(&V3c::new(0, 0, 0)) == (&red).into());
}

#[test]
fn test_simple_insert_and_get_where_dim_is_1() {
    let red: Albedo = 0xFF0000FF.into();
    let green: Albedo = 0x00FF00FF.into();
    let blue: Albedo = 0x0000FFFF.into();

    let mut tree: BoxTree = BoxTree::new(4, 1).ok().unwrap();
    tree.auto_simplify = false;
    tree.insert(&V3c::new(1, 0, 0), &red)
        .expect("insert to work");
    tree.insert(&V3c::new(0, 1, 0), &green)
        .expect("insert to work");
    tree.insert(&V3c::new(0, 0, 1), &blue)
        .expect("insert to work");

    assert!(tree.get(&V3c::new(1, 0, 0)) == (&red).into());
    assert!(tree.get(&V3c::new(0, 1, 0)) == (&green).into());
    assert!(tree.get(&V3c::new(0, 0, 1)) == (&blue).into());
    assert!(tree.get(&V3c::new(1, 1, 1)) == BoxTreeEntry::Empty);

    // Overwrite some data as well
    tree.insert(&V3c::new(1, 0, 0), &green)
        .expect("insert to work");
    assert!(tree.get(&V3c::new(1, 0, 0)) == (&green).into());
    assert!(tree.get(&V3c::new(0, 1, 0)) == (&green).into());
    assert!(tree.get(&V3c::new(0, 0, 1)) == (&blue).into());
    assert!(tree.get(&V3c::new(1, 1, 1)) == BoxTreeEntry::Empty);
}

#[test]
fn test_insert_empty() {
    let mut tree: BoxTree = BoxTree::new(4, 1).ok().unwrap();
    tree.auto_simplify = false;
    tree.insert(&V3c::new(0, 0, 0), BoxTreeEntry::Empty).ok();
    assert!(tree.get(&V3c::new(0, 0, 0)) == BoxTreeEntry::Empty);
}

#[test]
fn test_complex_insert_and_get() {
    let red: Albedo = 0xFF0000FF.into();
    let green: Albedo = 0x00FF00FF.into();

    let mut tree: BoxTree = BoxTree::new(4, 1).ok().unwrap();
    tree.auto_simplify = false;
    tree.insert(&V3c::new(1, 0, 0), (&red, &3))
        .expect("insert to work");
    tree.insert(&V3c::new(0, 1, 0), (&green, &1))
        .expect("insert to work");
    tree.insert(&V3c::new(0, 0, 1), voxel_data!(&2))
        .expect("insert to work");

    let hit = tree.get(&V3c::new(1, 0, 0));
    assert!(
        hit == (&red, &3).into(),
        "Hit mismatch at {:?}: {:?} <> {:?}",
        (0, 0, 0),
        hit,
        (&red, &3)
    );
    let hit = tree.get(&V3c::new(0, 1, 0));
    assert!(
        hit == (&green, &1).into(),
        "Hit mismatch at {:?}: {:?} <> {:?}",
        (0, 0, 0),
        hit,
        (&green, &1)
    );
    let hit = tree.get(&V3c::new(0, 0, 1));
    assert!(
        hit == voxel_data!(&2),
        "Hit mismatch at {:?}: {:?} <> {:?}",
        (0, 0, 0),
        hit,
        &2
    );
    let hit = tree.get(&V3c::new(1, 1, 1));
    assert!(
        hit == BoxTreeEntry::Empty,
        "Hit mismatch at {:?}: {:?} <> {:?}",
        (0, 0, 0),
        hit,
        BoxTreeEntry::<u32>::Empty
    );

    // Overwrite some data as well
    tree.insert(&V3c::new(1, 0, 0), voxel_data!(&3))
        .expect("insert to work");
    assert!(tree.get(&V3c::new(1, 0, 0)) == voxel_data!(&3));
    assert!(tree.get(&V3c::new(0, 1, 0)) == (&green, &1).into());
    assert!(tree.get(&V3c::new(0, 0, 1)) == voxel_data!(&2));
    assert!(tree.get(&V3c::new(1, 1, 1)) == BoxTreeEntry::Empty);
}

#[test]
fn test_simple_insert_and_get_where_dim_is_2() {
    let red: Albedo = 0xFF0000FF.into();
    let green: Albedo = 0x00FF00FF.into();
    let blue: Albedo = 0x0000FFFF.into();

    let mut tree: BoxTree = BoxTree::new(8, 2).ok().unwrap();
    tree.auto_simplify = false;
    tree.insert(&V3c::new(1, 0, 0), &red).ok().unwrap();
    tree.insert(&V3c::new(0, 1, 0), &green).ok().unwrap();
    tree.insert(&V3c::new(0, 0, 1), &blue).ok().unwrap();
    assert!(tree.get(&V3c::new(1, 0, 0)) == (&red).into());
    assert!(tree.get(&V3c::new(0, 1, 0)) == (&green).into());
    assert!(tree.get(&V3c::new(0, 0, 1)) == (&blue).into());

    tree.insert(&V3c::new(3, 0, 0), &red).ok().unwrap();
    tree.insert(&V3c::new(0, 3, 0), &green).ok().unwrap();
    tree.insert(&V3c::new(0, 0, 3), &blue).ok().unwrap();
    assert!(tree.get(&V3c::new(3, 0, 0)) == (&red).into());
    assert!(tree.get(&V3c::new(0, 3, 0)) == (&green).into());
    assert!(tree.get(&V3c::new(0, 0, 3)) == (&blue).into());

    // Overwrite some data as well
    tree.insert(&V3c::new(1, 0, 0), &green)
        .expect("insert to work");
    assert!(tree.get(&V3c::new(1, 0, 0)) == (&green).into());
    assert!(tree.get(&V3c::new(0, 1, 0)) == (&green).into());
    assert!(tree.get(&V3c::new(0, 0, 1)) == (&blue).into());
}

#[test]
fn test_insert_at_lod_where_dim_is_1() {
    let red: Albedo = 0xFF0000FF.into();
    let green: Albedo = 0x00FF00FF.into();

    let mut tree: BoxTree = BoxTree::new(16, 1).ok().unwrap();
    tree.auto_simplify = false;

    // This will set the area equal to 8 1-sized nodes
    tree.insert_at_lod(&V3c::new(0, 0, 0), 2, &red)
        .ok()
        .unwrap();

    assert!(tree.get(&V3c::new(0, 0, 0)) == (&red).into());
    assert!(tree.get(&V3c::new(0, 0, 1)) == (&red).into());
    assert!(tree.get(&V3c::new(0, 1, 0)) == (&red).into());
    assert!(tree.get(&V3c::new(0, 1, 1)) == (&red).into());
    assert!(tree.get(&V3c::new(1, 0, 0)) == (&red).into());
    assert!(tree.get(&V3c::new(1, 0, 1)) == (&red).into());
    assert!(tree.get(&V3c::new(1, 1, 0)) == (&red).into());
    assert!(tree.get(&V3c::new(1, 1, 1)) == (&red).into());

    // This will set the area equal to 64 1-sized nodes:
    // a size-4 node includes 2 levels,
    // 1-sized nodes at the bottom level doesn't have children,
    // 2-sized nodes above have 8 children each
    // so one 4-sized node has 8*8 = 64 children
    tree.insert_at_lod(&V3c::new(0, 0, 0), 4, &green)
        .ok()
        .unwrap();
    let mut hits = 0;
    for x in 0..4 {
        for y in 0..4 {
            for z in 0..4 {
                let hit = tree.get(&V3c::new(x, y, z));
                if hit != BoxTreeEntry::Empty {
                    assert!(
                        hit == (&green).into(),
                        "Hit mismatch at {:?}: {:?} <> {:?}",
                        (x, y, z),
                        hit,
                        green,
                    );
                    hits += 1;
                }
            }
        }
    }
    assert_eq!(hits, 64);
}

#[test]
fn test_insert_at_lod_where_dim_is_2() {
    let red: Albedo = 0xFF0000FF.into();
    let green: Albedo = 0x00FF00FF.into();

    let mut tree: BoxTree = BoxTree::new(8, 2).ok().unwrap();
    tree.auto_simplify = false;

    // This will set the area equal to 8 1-sized nodes
    tree.insert_at_lod(&V3c::new(0, 0, 0), 2, &red)
        .ok()
        .unwrap();

    assert!(tree.get(&V3c::new(0, 0, 0)) == (&red).into());
    assert!(tree.get(&V3c::new(0, 0, 1)) == (&red).into());
    assert!(tree.get(&V3c::new(0, 1, 0)) == (&red).into());
    assert!(tree.get(&V3c::new(0, 1, 1)) == (&red).into());
    assert!(tree.get(&V3c::new(1, 0, 0)) == (&red).into());
    assert!(tree.get(&V3c::new(1, 0, 1)) == (&red).into());
    assert!(tree.get(&V3c::new(1, 1, 0)) == (&red).into());
    assert!(tree.get(&V3c::new(1, 1, 1)) == (&red).into());

    // This will set the area equal to 64 1-sized nodes:
    // a size-4 node includes 2 levels,
    // 1-sized nodes at the bottom level doesn't have children,
    // 2-sized nodes above have 8 children each
    // so one 4-sized node has 8*8 = 64 children
    tree.insert_at_lod(&V3c::new(0, 0, 0), 4, &green)
        .ok()
        .unwrap();
    let mut hits = 0;
    for x in 0..4 {
        for y in 0..4 {
            for z in 0..4 {
                let hit = tree.get(&V3c::new(x, y, z));
                if hit != BoxTreeEntry::Empty {
                    assert!(
                        hit == (&green).into(),
                        "Hit mismatch at {:?}: {:?} <> {:?}",
                        (x, y, z),
                        hit,
                        green,
                    );
                    hits += 1;
                }
            }
        }
    }
    assert_eq!(hits, 64);
}

#[test]
fn test_case_simplified_insert_separated_by_clear_where_dim_is_1() {
    let tree_size = 16;
    const MATRIX_DIMENSION: u32 = 1;
    let red: Albedo = 0xFF0000FF.into();
    let mut tree: BoxTree = BoxTree::new(tree_size, MATRIX_DIMENSION).ok().unwrap();

    for x in 0..tree_size {
        for y in 0..tree_size {
            for z in 0..tree_size {
                tree.insert(&V3c::new(x, y, z), &red).ok().unwrap();
            }
        }
    }

    tree.clear(&V3c::new(3, 3, 3)).ok().unwrap();
    assert_eq!(tree.get(&V3c::new(3, 3, 3)), BoxTreeEntry::Empty);

    let mut hits = 0;
    for x in 0..tree_size {
        for y in 0..tree_size {
            for z in 0..tree_size {
                let hit = tree.get(&V3c::new(x, y, z));
                if hit != BoxTreeEntry::Empty {
                    assert_eq!(hit, (&red).into(), "Hit mismatch at {:?}", (x, y, z));
                    hits += 1;
                }
            }
        }
    }

    assert_eq!(hits, tree_size.pow(3) - 1);
}

#[test]
fn test_case_simplified_insert_separated_by_clear_where_dim_is_2() {
    let tree_size = 8;
    const MATRIX_DIMENSION: u32 = 2;
    let red: Albedo = 0xFF0000FF.into();
    let mut tree: BoxTree = BoxTree::new(tree_size, MATRIX_DIMENSION).ok().unwrap();

    for x in 0..tree_size {
        for y in 0..tree_size {
            for z in 0..tree_size {
                tree.insert(&V3c::new(x, y, z), &red).ok().unwrap();
            }
        }
    }

    let item_at_333 = tree.get(&V3c::new(3, 3, 3));
    assert_eq!(
        item_at_333,
        (&red).into(),
        "Hit mismatch at {:?}",
        (3, 3, 3)
    );
    tree.clear(&V3c::new(3, 3, 3)).ok().unwrap();
    let item_at_333 = tree.get(&V3c::new(3, 3, 3));
    assert_eq!(
        item_at_333,
        BoxTreeEntry::Empty,
        "Hit mismatch at {:?}",
        (3, 3, 3)
    );

    let mut hits = 0;
    for x in 0..tree_size {
        for y in 0..tree_size {
            for z in 0..tree_size {
                let hit = tree.get(&V3c::new(x, y, z));
                if hit != BoxTreeEntry::Empty {
                    assert_eq!(hit, (&red).into(), "Hit mismatch at {:?}", (x, y, z));
                    hits += 1;
                }
            }
        }
    }

    assert_eq!(hits, 511);
}

#[test]
fn test_case_simplified_insert_separated_by_clear_where_dim_is_4() {
    let tree_size = 16;
    const MATRIX_DIMENSION: u32 = 4;
    let red: Albedo = 0xFF0000FF.into();
    let mut tree: BoxTree = BoxTree::new(tree_size, MATRIX_DIMENSION).ok().unwrap();

    for x in 0..tree_size {
        for y in 0..tree_size {
            for z in 0..tree_size {
                tree.insert(&V3c::new(x, y, z), &red).ok().unwrap();
            }
        }
    }

    tree.clear(&V3c::new(3, 3, 3)).ok().unwrap();
    assert_eq!(tree.get(&V3c::new(3, 3, 3)), BoxTreeEntry::Empty);

    let mut hits = 0;
    for x in 0..tree_size {
        for y in 0..tree_size {
            for z in 0..tree_size {
                let hit = tree.get(&V3c::new(x, y, z));
                if hit != BoxTreeEntry::Empty {
                    assert_eq!(hit, (&red).into(), "Hit mismatch at {:?}", (x, y, z));
                    hits += 1;
                }
            }
        }
    }
    assert_eq!(hits, tree_size.pow(3) - 1);
}

#[test]
fn test_update_color() {
    let red: Albedo = 0xFF0000FF.into();
    let green: Albedo = 0x00FF00FF.into();
    let mut tree: BoxTree = BoxTree::new(4, 1).ok().unwrap();
    tree.auto_simplify = false;

    tree.insert(&V3c::new(0, 0, 0), (&red, &3)).ok();
    assert!(tree.get(&V3c::new(0, 0, 0)) == (&red, &3).into());

    tree.update(&V3c::new(0, 0, 0), &green).ok();
    let hit = tree.get(&V3c::new(0, 0, 0));
    assert_eq!(hit, (&green, &3).into(), "Hit mismatch at {:?}", (0, 0, 0));
}

#[test]
fn test_update_data() {
    let red: Albedo = 0xFF0000FF.into();
    let mut tree: BoxTree = BoxTree::new(4, 1).ok().unwrap();
    tree.auto_simplify = false;

    tree.insert(&V3c::new(0, 0, 0), (&red, &3)).ok();
    assert!(tree.get(&V3c::new(0, 0, 0)) == (&red, &3).into());

    tree.update(&V3c::new(0, 0, 0), BoxTreeEntry::Informative(&4))
        .ok();
    let hit = tree.get(&V3c::new(0, 0, 0));
    assert_eq!(hit, (&red, &4).into(), "Hit mismatch at {:?}", (0, 0, 0));
}

#[test]
fn test_update_empty() {
    let red: Albedo = 0xFF0000FF.into();
    let mut tree: BoxTree = BoxTree::new(4, 1).ok().unwrap();
    tree.auto_simplify = false;

    tree.insert(&V3c::new(0, 0, 0), (&red, &3)).ok();
    assert!(tree.get(&V3c::new(0, 0, 0)) == (&red, &3).into());

    tree.update(&V3c::new(0, 0, 0), BoxTreeEntry::Empty).ok();
    assert!(tree.get(&V3c::new(0, 0, 0)) == (&red, &3).into());
}

#[test]
fn test_uniform_solid_leaf_separated_by_clear_where_dim_is_1() {
    let tree_size = 4;
    const MATRIX_DIMENSION: u32 = 1;
    let mut tree: BoxTree = BoxTree::new(tree_size, MATRIX_DIMENSION).ok().unwrap();

    // Fill each sectant of the leaf with the same data, it should become a uniform leaf
    let color_base_original: Albedo = 0xFFFF00FF.into();

    for sectant in 0..BOX_NODE_CHILDREN_COUNT {
        tree.insert(
            &V3c::from(SECTANT_OFFSET_LUT[sectant] * tree_size as f32),
            &color_base_original,
        )
        .ok()
        .unwrap();
    }

    assert_eq!(tree.get(&V3c::unit(0)), (&color_base_original).into());

    // Separate Uniform leaf by clearing a voxel
    tree.clear(&V3c::unit(0)).ok().unwrap();
    assert_eq!(tree.get(&V3c::unit(0)), BoxTreeEntry::Empty);

    // The rest of the voxels should remain intact
    for sectant in 1..BOX_NODE_CHILDREN_COUNT {
        let pos = V3c::<u32>::from(SECTANT_OFFSET_LUT[sectant] * tree_size as f32);
        assert_eq!(
            tree.get(&pos),
            (&color_base_original).into(),
            "Hit mismatch at position: {:?}",
            pos
        );
    }
}

#[test]
fn test_uniform_solid_leaf_separated_by_insert_where_dim_is_1() {
    let tree_size = 4;
    const MATRIX_DIMENSION: u32 = 1;
    let mut tree: BoxTree = BoxTree::new(tree_size, MATRIX_DIMENSION).ok().unwrap();

    // Fill each sectant of the leaf with the same data, it should become a uniform leaf
    let color_base_original: Albedo = 0xFFFF00FF.into();

    for sectant in 0..BOX_NODE_CHILDREN_COUNT {
        let start_pos = V3c::<u32>::from(SECTANT_OFFSET_LUT[sectant]);
        tree.insert(&start_pos, &color_base_original).ok().unwrap();
    }

    let item_at_000 = tree.get(&V3c::unit(0));
    assert!(item_at_000 == (&color_base_original).into());

    // Separate Uniform leaf by overwriting a voxel
    let color_base_modified: Albedo = 0xFFFF00FF.into();
    tree.insert(&V3c::unit(0), &color_base_modified)
        .ok()
        .unwrap();
    assert!(tree.get(&V3c::unit(0)) == (&color_base_modified).into());

    // The rest of the voxels should remain intact
    for sectant in 1..BOX_NODE_CHILDREN_COUNT {
        let start_pos = V3c::<u32>::from(SECTANT_OFFSET_LUT[sectant]);
        assert!(tree.get(&start_pos) == (&color_base_original).into());
    }
}

#[test]
fn test_uniform_solid_leaf_separated_by_clear_where_dim_is_4() {
    let tree_size = 16;
    const MATRIX_DIMENSION: u32 = 4;
    let mut tree: BoxTree = BoxTree::new(tree_size, MATRIX_DIMENSION).ok().unwrap();

    // Fill each sectant with the same data, they should become a solid bricks
    let color_base = 0xFFFF00AA;
    for sectant in 0..BOX_NODE_CHILDREN_COUNT {
        let start_pos = V3c::<u32>::from(SECTANT_OFFSET_LUT[sectant] * tree_size as f32);
        for x in 0..MATRIX_DIMENSION {
            for y in 0..MATRIX_DIMENSION {
                for z in 0..MATRIX_DIMENSION {
                    tree.insert(
                        &(start_pos + V3c::new(x, y, z)),
                        &Albedo::from(color_base + sectant as u32),
                    )
                    .expect("Tree insertion error");
                }
            }
        }
    }

    // Separate Uniform leaf by clearing a voxel
    assert!(tree.get(&V3c::unit(0)) == (&Albedo::from(color_base)).into());
    tree.clear(&V3c::unit(0)).ok().unwrap();
    assert!(tree.get(&V3c::unit(0)) == BoxTreeEntry::Empty);

    // The rest of the voxels should remain intact
    for sectant in 0..BOX_NODE_CHILDREN_COUNT {
        let start_pos = V3c::<u32>::from(SECTANT_OFFSET_LUT[sectant] * tree_size as f32);
        for x in 0..MATRIX_DIMENSION {
            for y in 0..MATRIX_DIMENSION {
                for z in 0..MATRIX_DIMENSION {
                    if x == 0 && y == 0 && z == 0 && sectant == 0 {
                        continue;
                    }
                    assert!(
                        tree.get(&(start_pos + V3c::new(x, y, z)))
                            == (&Albedo::from(color_base + sectant as u32)).into(),
                    );
                }
            }
        }
    }
}

#[test]
fn test_uniform_solid_leaf_separated_by_insert_where_dim_is_4() {
    let tree_size = 16;
    const MATRIX_DIMENSION: u32 = 4;
    let mut tree: BoxTree = BoxTree::new(tree_size, MATRIX_DIMENSION).ok().unwrap();

    // Fill each sectant with the same data, they should become a solid bricks
    let color_base = 0xFFFF00AA;
    for sectant in 0..BOX_NODE_CHILDREN_COUNT {
        let start_pos = V3c::<u32>::from(SECTANT_OFFSET_LUT[sectant] * tree_size as f32);
        for x in 0..MATRIX_DIMENSION {
            for y in 0..MATRIX_DIMENSION {
                for z in 0..MATRIX_DIMENSION {
                    tree.insert(
                        &(start_pos + V3c::new(x, y, z)),
                        &Albedo::from(color_base + sectant as u32),
                    )
                    .ok()
                    .unwrap();
                }
            }
        }
    }

    // Separate Uniform leaf by overwriting a voxel
    assert!(tree.get(&V3c::unit(0)) == (&Albedo::from(color_base)).into());
    let color_base_modified: Albedo = 0x000000FF.into();
    tree.insert(&V3c::unit(0), &color_base_modified)
        .ok()
        .unwrap();
    assert!(tree.get(&V3c::unit(0)) == (&color_base_modified).into());

    // The rest of the voxels should remain intact
    for sectant in 0..BOX_NODE_CHILDREN_COUNT {
        let start_pos = V3c::<u32>::from(SECTANT_OFFSET_LUT[sectant] * tree_size as f32);
        for x in 0..MATRIX_DIMENSION {
            for y in 0..MATRIX_DIMENSION {
                for z in 0..MATRIX_DIMENSION {
                    if x == 0 && y == 0 && z == 0 && sectant == 0 {
                        continue;
                    }
                    assert!(
                        tree.get(&(start_pos + V3c::new(x, y, z)))
                            == (&Albedo::from(color_base + sectant as u32)).into(),
                    );
                }
            }
        }
    }
}

#[test]
fn test_uniform_parted_brick_leaf_separated_by_clear_where_dim_is_4() {
    let tree_size = 16;
    const MATRIX_DIMENSION: u32 = 4;
    let mut tree: BoxTree = BoxTree::new(tree_size, MATRIX_DIMENSION).ok().unwrap();

    // Fill each sectant with the same data
    for sectant in 0..BOX_NODE_CHILDREN_COUNT {
        for x in 0..MATRIX_DIMENSION {
            for y in 0..MATRIX_DIMENSION {
                for z in 0..MATRIX_DIMENSION {
                    let pos = V3c::<u32>::from(SECTANT_OFFSET_LUT[sectant] * tree_size as f32)
                        + V3c::new(x, y, z);
                    let color_for_pos = Albedo {
                        r: (x - (x % (MATRIX_DIMENSION / 2))) as u8,
                        g: (y - (y % (MATRIX_DIMENSION / 2))) as u8,
                        b: (z - (z % (MATRIX_DIMENSION / 2))) as u8,
                        a: 255,
                    };
                    tree.insert(&pos, &color_for_pos).ok().unwrap();
                    assert_eq!(
                        tree.get(&pos),
                        (&color_for_pos).into(),
                        "hit mismatch at position: {:?} + {:?} = {:?}",
                        (x, y, z),
                        sectant,
                        pos
                    );
                }
            }
        }
    }

    // Separate Uniform leaf by clearing a voxel
    assert_eq!(tree.get(&V3c::unit(0)), (&Albedo::from(0x000000FF)).into());
    tree.clear(&V3c::unit(1)).ok().unwrap();
    assert!(tree.get(&V3c::unit(1)).is_none());

    // The rest of the voxels should remain the same
    for sectant in 0..BOX_NODE_CHILDREN_COUNT {
        for x in 0..MATRIX_DIMENSION {
            for y in 0..MATRIX_DIMENSION {
                for z in 0..MATRIX_DIMENSION {
                    let pos = V3c::<u32>::from(SECTANT_OFFSET_LUT[sectant] * tree_size as f32)
                        + V3c::new(x, y, z);
                    if x == 1 && y == 1 && z == 1 && sectant == 0 {
                        assert!(tree.get(&pos).is_none());
                    } else {
                        assert_eq!(
                            tree.get(&pos),
                            (&Albedo {
                                r: (x - (x % (MATRIX_DIMENSION / 2))) as u8,
                                g: (y - (y % (MATRIX_DIMENSION / 2))) as u8,
                                b: (z - (z % (MATRIX_DIMENSION / 2))) as u8,
                                a: 255,
                            })
                                .into(),
                            "hit mismatch at position: {:?} + {:?} = {:?}",
                            (x, y, z),
                            sectant,
                            pos,
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn test_simple_uniform_parted_brick_leaf_overwrites_separated_by_insert_where_dim_is_2() {
    let tree_size = 8;
    const MATRIX_DIMENSION: u32 = 2;
    let mut tree: BoxTree = BoxTree::new(tree_size, MATRIX_DIMENSION).ok().unwrap();

    // Fill each sectant inside the brick with the same data
    let color_base_original = 0xF00000FF;
    let mut color_base = color_base_original;
    for sectant in 0..BOX_NODE_CHILDREN_COUNT {
        let start_pos = V3c::<u32>::from(SECTANT_OFFSET_LUT[sectant] * MATRIX_DIMENSION as f32);
        tree.insert_at_lod(&start_pos, MATRIX_DIMENSION, &Albedo::from(color_base))
            .ok()
            .unwrap();
        color_base += 0x02;
    }

    let item_at_000 = tree.get(&V3c::unit(0));
    assert_eq!(item_at_000, (&Albedo::from(color_base_original)).into());

    // Separate Uniform leaf by setting a voxel
    let color_base_modified: Albedo = 0x000000FF.into();
    tree.insert(&V3c::unit(0), &color_base_modified)
        .ok()
        .unwrap();
    assert_eq!(tree.get(&V3c::unit(0)), (&color_base_modified).into());
}

#[test]
fn test_simple_uniform_parted_brick_leaf_separated_by_insert_where_dim_is_2() {
    let tree_size = 8;
    const MATRIX_DIMENSION: u32 = 2;
    let mut tree: BoxTree = BoxTree::new(tree_size, MATRIX_DIMENSION).ok().unwrap();

    // Fill each sectant with the same data
    let color_base_original = 0xF00000FF;
    let mut color_base = color_base_original;
    for sectant in 0..BOX_NODE_CHILDREN_COUNT {
        let start_pos = V3c::<u32>::from(SECTANT_OFFSET_LUT[sectant] * tree_size as f32);
        tree.insert_at_lod(&start_pos, MATRIX_DIMENSION, &Albedo::from(color_base))
            .ok()
            .unwrap();
        color_base += 0x02;
    }

    let item_at_000 = tree.get(&V3c::unit(0));
    assert_eq!(item_at_000, (&Albedo::from(color_base_original)).into());

    // Separate Uniform leaf by setting a voxel
    let color_base_modified: Albedo = 0x000000FF.into();
    tree.insert(&V3c::unit(0), &color_base_modified)
        .ok()
        .unwrap();
    assert_eq!(tree.get(&V3c::unit(0)), (&color_base_modified).into());

    // The rest of the voxels should remain intact
    for x in 0..MATRIX_DIMENSION {
        for y in 0..MATRIX_DIMENSION {
            for z in 0..MATRIX_DIMENSION {
                color_base = color_base_original;
                for sectant in 0..BOX_NODE_CHILDREN_COUNT {
                    let start_pos =
                        V3c::<u32>::from(SECTANT_OFFSET_LUT[sectant] * tree_size as f32)
                            + V3c::new(x, y, z);
                    if x == 0 && y == 0 && z == 0 && sectant == 0 {
                        assert_eq!(
                            tree.get(&start_pos),
                            (&Albedo::from(color_base_modified)).into()
                        );
                    } else {
                        assert_eq!(tree.get(&start_pos), (&Albedo::from(color_base)).into());
                    }
                    color_base += 0x02;
                }
            }
        }
    }
}

#[test]
fn test_simple_uniform_parted_brick_leaf_separated_by_insert_where_dim_is_4() {
    let tree_size = 16;
    const MATRIX_DIMENSION: u32 = 4;
    let mut tree: BoxTree = BoxTree::new(tree_size, MATRIX_DIMENSION).ok().unwrap();

    // Fill each sectant with the same data
    for sectant in 0..BOX_NODE_CHILDREN_COUNT {
        for x in 0..MATRIX_DIMENSION {
            for y in 0..MATRIX_DIMENSION {
                for z in 0..MATRIX_DIMENSION {
                    let pos = V3c::<u32>::from(SECTANT_OFFSET_LUT[sectant] * tree_size as f32)
                        + V3c::new(x, y, z);
                    let color_for_pos = Albedo {
                        r: (x - (x % (MATRIX_DIMENSION / 2))) as u8,
                        g: (y - (y % (MATRIX_DIMENSION / 2))) as u8,
                        b: (z - (z % (MATRIX_DIMENSION / 2))) as u8,
                        a: 255,
                    };
                    tree.insert(&pos, &color_for_pos).ok().unwrap();
                    assert_eq!(
                        tree.get(&pos),
                        (&color_for_pos).into(),
                        "hit mismatch at position: {:?} + {:?} = {:?}",
                        (x, y, z),
                        sectant,
                        pos
                    );
                }
            }
        }
    }

    let item_at_000 = tree.get(&V3c::unit(0));
    assert_eq!(item_at_000, (&Albedo::from(0x000000FF)).into());

    // Separate Uniform leaf by setting a voxel
    let color_base_modified: Albedo = 0xFF0000FF.into();
    tree.insert(&V3c::unit(1), &color_base_modified)
        .ok()
        .unwrap();
    assert_eq!(tree.get(&V3c::unit(1)), (&color_base_modified).into());

    let item_at_000 = tree.get(&V3c::unit(0));
    assert_eq!(item_at_000, (&Albedo::from(0x000000FF)).into());

    // The rest of the voxels should remain the same
    for sectant in 0..BOX_NODE_CHILDREN_COUNT {
        for x in 0..MATRIX_DIMENSION {
            for y in 0..MATRIX_DIMENSION {
                for z in 0..MATRIX_DIMENSION {
                    let pos = V3c::<u32>::from(SECTANT_OFFSET_LUT[sectant] * tree_size as f32)
                        + V3c::new(x, y, z);
                    if x == 1 && y == 1 && z == 1 && sectant == 0 {
                        assert_eq!(tree.get(&pos), (&Albedo::from(color_base_modified)).into());
                    } else {
                        assert_eq!(
                            tree.get(&pos),
                            (&Albedo {
                                r: (x - (x % (MATRIX_DIMENSION / 2))) as u8,
                                g: (y - (y % (MATRIX_DIMENSION / 2))) as u8,
                                b: (z - (z % (MATRIX_DIMENSION / 2))) as u8,
                                a: 255,
                            })
                                .into(),
                            "hit mismatch at position: {:?} + {:?} = {:?}",
                            (x, y, z),
                            sectant,
                            pos,
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn test_insert_at_lod_with_unaligned_position_where_dim_is_4() {
    let red: Albedo = 0xFF0000FF.into();

    let mut tree: BoxTree = BoxTree::new(16, 4).ok().unwrap();
    tree.auto_simplify = false;

    tree.insert_at_lod(&V3c::new(1, 1, 1), 4, &red)
        .ok()
        .unwrap();

    // This will set the area equal to 64 1-sized nodes:
    // a size-4 node includes 2 levels,
    // 1-sized nodes at the bottom level doesn't have children,
    // 2-sized nodes above have 8 children each
    // so one 4-sized node has 8*8 = 64 children
    let mut hits = 0;
    for x in 0..4 {
        for y in 0..4 {
            for z in 0..4 {
                let hit = tree.get(&V3c::new(x, y, z));
                if hit != BoxTreeEntry::Empty {
                    assert!(
                        hit == (&red).into(),
                        "Hit mismatch at {:?}: {:?} <> {:?}",
                        (x, y, z),
                        hit,
                        red,
                    );
                    hits += 1;
                }
            }
        }
    }

    // At most one brick can be updated; Starting from 1,1,1 the updated area spans 3x3x3,
    // thus the number of voxels updated are 27
    assert!(hits == 27, "Expected 27 hits instead of {hits}");
}

#[test]
fn test_insert_at_lod_with_unaligned_size_where_dim_is_1() {
    let red: Albedo = 0xFF0000FF.into();

    let mut tree: BoxTree = BoxTree::new(16, 1).ok().unwrap();
    tree.auto_simplify = false;

    tree.insert_at_lod(&V3c::new(2, 2, 2), 3, &red)
        .ok()
        .unwrap();
    let mut hits = 0;
    for x in 0..8 {
        for y in 0..8 {
            for z in 0..8 {
                let hit = tree.get(&V3c::new(x, y, z));
                if hit != BoxTreeEntry::Empty {
                    assert!(
                        hit == (&red).into(),
                        "Hit mismatch at {:?}: {:?} <> {:?}",
                        (x, y, z),
                        hit,
                        red,
                    );
                    hits += 1;
                }
            }
        }
    }

    assert_eq!(hits, 8);
}

#[test]
fn test_insert_at_lod_with_unaligned_size_and_position_where_dim_is_1() {
    let red: Albedo = 0xFF0000FF.into();

    let mut tree: BoxTree = BoxTree::new(16, 1).ok().unwrap();
    tree.auto_simplify = false;

    tree.insert_at_lod(&V3c::new(3, 3, 3), 3, &red)
        .ok()
        .unwrap();
    let mut hits = 0;
    for x in 0..8 {
        for y in 0..8 {
            for z in 0..8 {
                let hit = tree.get(&V3c::new(x, y, z));
                if hit != BoxTreeEntry::Empty {
                    assert!(
                        hit == (&red).into(),
                        "Hit mismatch at {:?}: {:?} <> {:?}",
                        (x, y, z),
                        hit,
                        red,
                    );
                    hits += 1;
                }
            }
        }
    }

    // Since the position points to a corner of a brick, only one voxel is inserted
    assert_eq!(hits, 1);
}

#[test]
fn test_insert_at_lod_with_unaligned_size_where_dim_is_4() {
    let red: Albedo = 0xFF0000FF.into();

    let mut tree: BoxTree = BoxTree::new(16, 4).ok().unwrap();
    tree.auto_simplify = false;

    tree.insert_at_lod(&V3c::new(1, 1, 1), 3, &red)
        .ok()
        .unwrap();

    assert!(tree.get(&V3c::new(1, 1, 1)) == (&red).into());
    let mut hits = 0;
    for x in 0..8 {
        for y in 0..8 {
            for z in 0..8 {
                let hit = tree.get(&V3c::new(x, y, z));
                if hit != BoxTreeEntry::Empty {
                    assert!(
                        hit == (&red).into(),
                        "Hit mismatch at {:?}: {:?} <> {:?}",
                        (x, y, z),
                        hit,
                        red,
                    );
                    hits += 1;
                }
            }
        }
    }
    assert_eq!(hits, 27);
}

#[test]
fn test_insert_at_lod_with_simplify() {
    let red: Albedo = 0xFF0000FF.into();
    let green: Albedo = 0x00FF00FF.into();

    let mut tree: BoxTree = BoxTree::new(16, 1).ok().unwrap();

    // This will set the area equal to 8 1-sized nodes
    tree.insert_at_lod(&V3c::new(4, 0, 0), 2, &red)
        .ok()
        .unwrap();

    assert_eq!(tree.get(&V3c::new(4, 0, 0)), (&red).into());
    assert_eq!(tree.get(&V3c::new(4, 0, 1)), (&red).into());
    assert_eq!(tree.get(&V3c::new(4, 1, 0)), (&red).into());
    assert_eq!(tree.get(&V3c::new(4, 1, 1)), (&red).into());
    assert_eq!(tree.get(&V3c::new(5, 0, 0)), (&red).into());
    assert_eq!(tree.get(&V3c::new(5, 0, 1)), (&red).into());
    assert_eq!(tree.get(&V3c::new(5, 1, 0)), (&red).into());
    assert_eq!(tree.get(&V3c::new(5, 1, 1)), (&red).into());

    // This will set the area equal to 64 1-sized nodes:
    // a size-4 node includes 2 levels,
    // 1-sized nodes at the bottom level doesn't have children,
    // 2-sized nodes above have 8 children each
    // so one 4-sized node has 8*8 = 64 children
    tree.insert_at_lod(&V3c::new(0, 0, 0), 4, &green)
        .ok()
        .unwrap();
    let mut hits = 0;
    for x in 0..4 {
        for y in 0..4 {
            for z in 0..4 {
                let hit = tree.get(&V3c::new(x, y, z));
                if hit != BoxTreeEntry::Empty {
                    assert_eq!(hit, (&green).into(), "Hit mismatch at {:?}", (x, y, z));
                    hits += 1;
                }
            }
        }
    }

    for x in 4..6 {
        for y in 0..2 {
            for z in 0..2 {
                let hit = tree.get(&V3c::new(x, y, z));
                if hit != BoxTreeEntry::Empty {
                    assert_eq!(hit, (&red).into(), "Hit mismatch at {:?}", (x, y, z));
                    hits += 1;
                }
            }
        }
    }
    assert_eq!(hits, (64 + 8));
}

#[test]
fn test_simplifyable_insert_and_get_where_dim_is_1() {
    let red: Albedo = 0xFF0000FF.into();
    let green: Albedo = 0x00FF00FF.into();

    const SIZE: u32 = 4;
    let mut tree: BoxTree = BoxTree::new(SIZE, 1).ok().unwrap();

    // The below set of values should be simplified to a single node
    for x in 0..SIZE {
        for y in 0..SIZE {
            for z in 0..SIZE {
                tree.insert(&V3c::new(x, y, z), &red).ok().unwrap();
            }
        }
    }

    // The below should brake the simplified node back to its parts
    tree.insert(&V3c::new(0, 0, 0), &green).ok().unwrap();

    // Integrity should be kept
    assert!(tree.get(&V3c::new(0, 0, 0)) == (&green).into());
    for x in 1..SIZE {
        for y in 1..SIZE {
            for z in 1..SIZE {
                assert!(tree.get(&V3c::new(x, y, z)) == (&red).into());
            }
        }
    }
}

#[test]
fn test_simplifyable_insert_and_get_where_dim_is_2() {
    let red: Albedo = 0xFF0000FF.into();
    let green: Albedo = 0x00FF00FF.into();

    const SIZE: u32 = 8;
    let mut tree: BoxTree = BoxTree::new(SIZE, 2).ok().unwrap();

    // The below set of values should be simplified to a single node
    for x in 0..SIZE {
        for y in 0..SIZE {
            for z in 0..SIZE {
                tree.insert(&V3c::new(x, y, z), &red).ok().unwrap();
            }
        }
    }

    // The below should brake the simplified node back to its parts
    tree.insert(&V3c::new(0, 0, 0), &green).ok().unwrap();

    // Integrity should be kept
    assert!(tree.get(&V3c::new(0, 0, 0)) == (&green).into());
    for x in 1..SIZE {
        for y in 1..SIZE {
            for z in 1..SIZE {
                assert!(tree.get(&V3c::new(x, y, z)) == (&red).into());
            }
        }
    }
}

#[test]
fn test_simple_clear_where_dim_is_1() {
    let red: Albedo = 0xFF0000FF.into();
    let green: Albedo = 0x00FF00FF.into();
    let blue: Albedo = 0x0000FFFF.into();

    let mut tree: BoxTree = BoxTree::new(4, 1).ok().unwrap();
    tree.auto_simplify = false;
    tree.insert(&V3c::new(1, 0, 0), &red).ok().unwrap();
    tree.insert(&V3c::new(0, 1, 0), &green).ok().unwrap();
    tree.insert(&V3c::new(0, 0, 1), &blue).ok().unwrap();
    tree.clear(&V3c::new(0, 0, 1)).ok().unwrap();

    assert_eq!(tree.get(&V3c::new(1, 0, 0)), (&red).into());
    assert_eq!(tree.get(&V3c::new(0, 1, 0)), (&green).into());
    assert_eq!(tree.get(&V3c::new(0, 0, 1)), BoxTreeEntry::Empty);
    assert_eq!(tree.get(&V3c::new(1, 1, 1)), BoxTreeEntry::Empty);
}

#[test]
fn test_simple_clear_where_dim_is_2() {
    let red: Albedo = 0xFF0000FF.into();
    let green: Albedo = 0x00FF00FF.into();
    let blue: Albedo = 0x0000FFFF.into();

    let mut tree: BoxTree = BoxTree::new(8, 2).ok().unwrap();
    tree.auto_simplify = false;

    tree.insert(&V3c::new(1, 0, 0), &red).ok().unwrap();
    assert_eq!(tree.get(&V3c::new(1, 0, 0)), (&red).into());

    tree.insert(&V3c::new(0, 1, 0), &green).ok().unwrap();
    assert_eq!(tree.get(&V3c::new(0, 1, 0)), (&green).into());

    tree.insert(&V3c::new(0, 0, 1), &blue).ok().unwrap();
    assert_eq!(tree.get(&V3c::new(0, 0, 1)), (&blue).into());

    tree.clear(&V3c::new(0, 0, 1)).ok().unwrap();
    assert_eq!(tree.get(&V3c::new(0, 0, 1)), BoxTreeEntry::Empty);

    assert_eq!(tree.get(&V3c::new(1, 0, 0)), (&red).into());
    assert_eq!(tree.get(&V3c::new(0, 1, 0)), (&green).into());
    assert_eq!(tree.get(&V3c::new(0, 0, 1)), BoxTreeEntry::Empty);
    assert_eq!(tree.get(&V3c::new(1, 1, 1)), BoxTreeEntry::Empty);
}

#[test]
fn test_clear_small_part_of_large_node_ocbits_resolution_test() {
    const TREE_SIZE: u32 = 128;
    const BRICK_DIMENSION: u32 = 8;
    let red: Albedo = 0xFF0000FF.into();
    let mut tree: BoxTree = BoxTree::new(TREE_SIZE, BRICK_DIMENSION).ok().unwrap();

    tree.insert(&V3c::new(0, 1, 1), &red).ok().unwrap();
    tree.insert(&V3c::new(1, 0, 0), &red).ok().unwrap();

    assert_eq!(tree.get(&V3c::new(0, 1, 1)), (&red).into());
    assert_eq!(tree.get(&V3c::new(1, 0, 0)), (&red).into());

    tree.clear(&V3c::new(1, 0, 0)).ok().unwrap();
    assert_eq!(tree.get(&V3c::new(1, 0, 0)), voxel_data!());
}

#[test]
fn test_set_small_part_of_large_node_ocbits_resolution_test_underflow() {
    const TREE_SIZE: u32 = 128;
    const BRICK_DIMENSION: u32 = 8;
    let red: Albedo = 0xFF0000FF.into();
    let mut tree: BoxTree = BoxTree::new(TREE_SIZE, BRICK_DIMENSION).ok().unwrap();

    tree.insert_at_lod(&V3c::new(33, 33, 33), 2, &red)
        .ok()
        .unwrap();

    assert_eq!(tree.get(&V3c::new(33, 33, 33)), (&red).into());

    tree.clear(&V3c::new(33, 33, 33)).ok().unwrap();
    assert_eq!(tree.get(&V3c::new(33, 33, 33)), voxel_data!());
}

#[test]
fn test_set_small_part_of_large_node_ocbits_resolution_test_overflow() {
    const TREE_SIZE: u32 = 128;
    const BRICK_DIMENSION: u32 = 8;
    let red: Albedo = 0xFF0000FF.into();
    let mut tree: BoxTree = BoxTree::new(TREE_SIZE, BRICK_DIMENSION).ok().unwrap();

    tree.insert(&V3c::new(31, 31, 31), &red).ok().unwrap();

    assert_eq!(tree.get(&V3c::new(31, 31, 31)), (&red).into());

    tree.clear(&V3c::new(31, 31, 31)).ok().unwrap();
    assert_eq!(tree.get(&V3c::new(31, 31, 31)), voxel_data!());
}

#[test]
fn test_double_clear() {
    let albedo_black: Albedo = 0x000000FF.into();
    let albedo_white: Albedo = 0xFFFFFFFF.into();
    let mut tree: BoxTree = BoxTree::new(4, 1).ok().unwrap();
    tree.auto_simplify = false;
    tree.insert(&V3c::new(1, 0, 0), &albedo_black).ok().unwrap();
    tree.insert(&V3c::new(0, 1, 0), &albedo_white).ok().unwrap();
    tree.insert(&V3c::new(0, 0, 1), &albedo_white).ok().unwrap();
    tree.clear(&V3c::new(0, 0, 1)).ok().unwrap();
    tree.clear(&V3c::new(0, 0, 1)).ok().unwrap();

    assert_eq!(tree.get(&V3c::new(1, 0, 0)), (&albedo_black).into());
    assert_eq!(tree.get(&V3c::new(0, 1, 0)), (&albedo_white).into());
    let item_at_001 = tree.get(&V3c::new(0, 0, 1));
    assert!(item_at_001 == BoxTreeEntry::Empty);
}

#[test]
fn test_simplifyable_clear() {
    let albedo: Albedo = 0xFFAAEEFF.into();
    const SIZE: u32 = 4;
    let mut tree: BoxTree = BoxTree::new(SIZE, 1).ok().unwrap();

    // The below set of values should be simplified to a single node
    for x in 0..SIZE {
        for y in 0..SIZE {
            for z in 0..SIZE {
                tree.insert(&V3c::new(x, y, z), &albedo).ok().unwrap();
            }
        }
    }

    // The below should brake the simplified node back to its party
    tree.clear(&V3c::new(0, 0, 0)).ok().unwrap();

    // Integrity should be kept
    let item_at_000 = tree.get(&V3c::new(0, 0, 0));
    assert!(item_at_000 == BoxTreeEntry::Empty);
    for x in 1..SIZE {
        for y in 1..SIZE {
            for z in 1..SIZE {
                assert!(tree.get(&V3c::new(x, y, z)) == (&albedo).into());
            }
        }
    }
}

#[test]
fn test_simplifyable_clear_where_dim_is_2() {
    let albedo: Albedo = 0xFFAAEEFF.into();
    const SIZE: u32 = 8;
    let mut tree: BoxTree = BoxTree::new(SIZE, 2).ok().unwrap();

    // The below set of values should be simplified to a single node
    for x in 0..SIZE {
        for y in 0..SIZE {
            for z in 0..SIZE {
                tree.insert(&V3c::new(x, y, z), &albedo).ok().unwrap();
            }
        }
    }

    // The below should brake the simplified node back to its party
    tree.clear(&V3c::new(0, 0, 0)).ok().unwrap();

    // Integrity should be kept
    let item_at_000 = tree.get(&V3c::new(0, 0, 0));
    assert!(item_at_000 == BoxTreeEntry::Empty);
    for x in 1..SIZE {
        for y in 1..SIZE {
            for z in 1..SIZE {
                assert!(tree.get(&V3c::new(x, y, z)) == (&albedo).into());
            }
        }
    }
}

#[test]
fn test_clear_to_nothing() {
    let albedo: Albedo = 0xFFAAEEFF.into();
    let entry = BoxTreeEntry::Visual(&albedo);
    let mut tree: BoxTree = BoxTree::new(4, 1).ok().unwrap();

    // The below set of values should be simplified to a single node
    tree.insert(&V3c::new(0, 0, 0), entry).ok().unwrap();
    tree.insert(&V3c::new(0, 0, 1), entry).ok().unwrap();
    tree.insert(&V3c::new(0, 1, 0), entry).ok().unwrap();
    tree.insert(&V3c::new(0, 1, 1), entry).ok().unwrap();
    tree.insert(&V3c::new(1, 0, 0), entry).ok().unwrap();
    tree.insert(&V3c::new(1, 0, 1), entry).ok().unwrap();
    tree.insert(&V3c::new(1, 1, 0), entry).ok().unwrap();
    tree.insert(&V3c::new(1, 1, 1), entry).ok().unwrap();

    // The below should brake the simplified node back to its party
    tree.clear_at_lod(&V3c::new(0, 0, 0), 2).ok().unwrap();

    // Nothing should remain in the tree
    assert_eq!(tree.get(&V3c::new(0, 0, 0)), BoxTreeEntry::Empty);
    assert_eq!(tree.get(&V3c::new(0, 0, 1)), BoxTreeEntry::Empty);
    assert_eq!(tree.get(&V3c::new(0, 1, 0)), BoxTreeEntry::Empty);
    assert_eq!(tree.get(&V3c::new(0, 1, 1)), BoxTreeEntry::Empty);
    assert_eq!(tree.get(&V3c::new(1, 0, 0)), BoxTreeEntry::Empty);
    assert_eq!(tree.get(&V3c::new(1, 0, 1)), BoxTreeEntry::Empty);
    assert_eq!(tree.get(&V3c::new(1, 1, 0)), BoxTreeEntry::Empty);
    assert_eq!(tree.get(&V3c::new(1, 1, 1)), BoxTreeEntry::Empty);
}

#[test]
fn test_clear_edge_case() {
    const TREE_SIZE: u32 = 64;
    const BRICK_DIMENSION: u32 = 16;
    let red: Albedo = 0xFF0000FF.into();
    let mut tree: BoxTree = BoxTree::new(TREE_SIZE, BRICK_DIMENSION).ok().unwrap();

    tree.update(&V3c::new(1, 0, 0), voxel_data!(&0xFACEFEED))
        .ok()
        .unwrap();

    tree.insert_at_lod(&V3c::new(0, 0, 0), 32, &red)
        .ok()
        .unwrap();

    tree.clear_at_lod(&V3c::new(5, 5, 5), 8).ok().unwrap();
    for x in 5..8 {
        for y in 5..8 {
            for z in 5..8 {
                assert_eq!(tree.get(&V3c::new(x, y, z)), voxel_data!());
            }
        }
    }

    for x in 0..5 {
        for y in 0..5 {
            for z in 0..5 {
                assert_eq!(
                    tree.get(&V3c::new(x, y, z)),
                    (&red).into(),
                    "Hit mismatch at {:?}",
                    (x, y, z)
                );
            }
        }
    }

    tree.clear_at_lod(&V3c::new(0, 0, 0), 32).ok().unwrap();
    for x in 0..32 {
        for y in 0..32 {
            for z in 0..32 {
                assert_eq!(
                    tree.get(&V3c::new(x, y, z)),
                    voxel_data!(),
                    "Hit mismatch at {:?}",
                    (x, y, z)
                );
            }
        }
    }
}

#[test]
fn test_clear_at_lod_where_dim_is_1() {
    let albedo: Albedo = 0xFFAAEEFF.into();
    let mut tree: BoxTree = BoxTree::new(16, 1).ok().unwrap();

    // This will set the area equal to 64 1-sized nodes
    tree.insert_at_lod(&V3c::new(0, 0, 0), 4, &albedo)
        .ok()
        .unwrap();

    // This will clear an area equal to 8 1-sized nodes
    tree.clear_at_lod(&V3c::new(0, 0, 0), 2).ok().unwrap();

    let mut hits = 0;
    for x in 0..4 {
        for y in 0..4 {
            for z in 0..4 {
                let hit = tree.get(&V3c::new(x, y, z));
                if hit != BoxTreeEntry::Empty {
                    assert!(
                        hit == (&albedo).into(),
                        "Hit mismatch at {:?}: {:?} <> {:?}",
                        (x, y, z),
                        hit,
                        albedo
                    );
                    hits += 1;
                }
            }
        }
    }

    // number of hits should be the number of nodes set minus the number of nodes cleared
    assert_eq!(hits, (64 - 8));
}

#[test]
fn test_clear_at_lod_where_dim_is_2() {
    let albedo: Albedo = 0xFFAAEEFF.into();
    let mut tree: BoxTree = BoxTree::new(8, 2).ok().unwrap();

    // This will set the area equal to 64 1-sized nodes
    tree.insert_at_lod(&V3c::new(0, 0, 0), 4, &albedo)
        .ok()
        .unwrap();

    let mut hits = 0;
    for x in 0..4 {
        for y in 0..4 {
            for z in 0..4 {
                let hit = tree.get(&V3c::new(x, y, z));
                if hit != BoxTreeEntry::Empty {
                    assert!(
                        hit == (&albedo).into(),
                        "Hit mismatch at {:?}: {:?} <> {:?}",
                        (x, y, z),
                        hit,
                        albedo,
                    );
                    hits += 1;
                }
            }
        }
    }

    // number of hits should be the number of nodes set
    assert_eq!(hits, 64);

    // This will clear an area equal to 8 1-sized nodes
    tree.clear_at_lod(&V3c::new(0, 0, 0), 2).ok().unwrap();

    let mut hits = 0;
    for x in 0..4 {
        for y in 0..4 {
            for z in 0..4 {
                let hit = tree.get(&V3c::new(x, y, z));
                if hit != BoxTreeEntry::Empty {
                    assert!(
                        hit == (&albedo).into(),
                        "Hit mismatch at {:?}: {:?} <> {:?}",
                        (x, y, z),
                        hit,
                        albedo,
                    );
                    hits += 1;
                }
            }
        }
    }

    // number of hits should be the number of nodes set minus the number of nodes cleared
    assert_eq!(hits, (64 - 8));
}

#[test]
fn test_clear_at_lod_with_unaligned_position_where_dim_is_1() {
    let albedo: Albedo = 0xFFAAEEFF.into();
    let mut tree: BoxTree = BoxTree::new(16, 1).ok().unwrap();

    // This will set the area equal to 64 1-sized nodes
    tree.insert_at_lod(&V3c::new(0, 0, 0), 4, &albedo)
        .ok()
        .unwrap();

    // This will clear an area equal to 8 1-sized nodes
    tree.clear_at_lod(&V3c::new(1, 1, 1), 2).ok().unwrap();

    // unset voxels should not be present
    assert!(tree.get(&V3c::new(1, 1, 1)) == BoxTreeEntry::Empty);
    assert!(tree.get(&V3c::new(1, 1, 2)) == BoxTreeEntry::Empty);
    assert!(tree.get(&V3c::new(1, 2, 1)) == BoxTreeEntry::Empty);
    assert!(tree.get(&V3c::new(1, 2, 2)) == BoxTreeEntry::Empty);
    assert!(tree.get(&V3c::new(2, 1, 1)) == BoxTreeEntry::Empty);
    assert!(tree.get(&V3c::new(2, 1, 2)) == BoxTreeEntry::Empty);
    assert!(tree.get(&V3c::new(2, 2, 1)) == BoxTreeEntry::Empty);
    assert!(tree.get(&V3c::new(2, 2, 2)) == BoxTreeEntry::Empty);

    // sampling some voxels who should be present
    assert!(tree.get(&V3c::new(0, 0, 2)).is_some());
    assert!(tree.get(&V3c::new(0, 2, 0)).is_some());
    assert!(tree.get(&V3c::new(0, 2, 2)).is_some());
    assert!(tree.get(&V3c::new(2, 0, 0)).is_some());
    assert!(tree.get(&V3c::new(2, 0, 2)).is_some());
    assert!(tree.get(&V3c::new(2, 2, 0)).is_some());
    assert!(tree.get(&V3c::new(2, 2, 3)).is_some());

    let mut hits = 0;
    for x in 0..4 {
        for y in 0..4 {
            for z in 0..4 {
                let hit = tree.get(&V3c::new(x, y, z));
                if hit != BoxTreeEntry::Empty {
                    assert!(
                        hit == (&albedo).into(),
                        "Hit mismatch at {:?}: {:?} <> {:?}",
                        (x, y, z),
                        hit,
                        albedo,
                    );
                    hits += 1;
                }
            }
        }
    }

    // number of hits should be the number of nodes set minus the number of nodes cleared
    assert!(hits == (64 - 8));
}

#[test]
fn test_clear_at_lod_with_unaligned_position_where_dim_is_4() {
    let albedo: Albedo = 0xFFAAEEFF.into();
    let mut tree: BoxTree = BoxTree::new(16, 4).ok().unwrap();

    tree.insert_at_lod(&V3c::new(0, 0, 0), 8, &albedo)
        .ok()
        .unwrap();

    assert!(tree.get(&V3c::unit(0)).is_some());

    let mut hits = 0;
    for x in 0..8 {
        for y in 0..8 {
            for z in 0..8 {
                let hit = tree.get(&V3c::new(x, y, z));
                assert_eq!(hit, (&albedo).into());
                if hit != BoxTreeEntry::Empty {
                    hits += 1;
                }
            }
        }
    }

    // number of hits should be the number of nodes set
    assert_eq!(hits, 512);

    tree.clear_at_lod(&V3c::new(1, 1, 1), 4).ok().unwrap();
    let mut hits = 0;
    for x in 0..8 {
        for y in 0..8 {
            for z in 0..8 {
                let hit = tree.get(&V3c::new(x, y, z));
                if hit != BoxTreeEntry::Empty {
                    assert!(
                        hit == (&albedo).into(),
                        "Hit mismatch at {:?}: {:?} <> {:?}",
                        (x, y, z),
                        hit,
                        albedo,
                    );
                    hits += 1;
                }
            }
        }
    }

    // number of hits should be the number of nodes set minus the number of nodes cleared
    // Note: Only at most one brick is updated with each update call
    // --> In this case the relevant brick is updated from 1,1,1 ---> 3,3,3 ( inclusive )
    // So 3^3 voxels are cleared == 27
    assert_eq!(hits, (512 - 27));
}

#[test]
fn test_clear_at_lod_with_unaligned_size_where_dim_is_1() {
    let albedo: Albedo = 0xFFAAEEFF.into();
    let mut tree: BoxTree = BoxTree::new(16, 1).ok().unwrap();
    tree.insert_at_lod(&V3c::new(0, 0, 0), 4, &albedo)
        .ok()
        .unwrap();

    let mut hits = 0;
    for x in 0..4 {
        for y in 0..4 {
            for z in 0..4 {
                let hit = tree.get(&V3c::new(x, y, z));
                if hit != BoxTreeEntry::Empty {
                    assert!(
                        hit == (&albedo).into(),
                        "Hit mismatch at {:?}: {:?} <> {:?}",
                        (x, y, z),
                        hit,
                        albedo,
                    );
                    hits += 1;
                }
            }
        }
    }

    // number of hits should be the number of nodes set
    assert_eq!(hits, 64);

    tree.clear_at_lod(&V3c::new(0, 0, 0), 3).ok().unwrap();
    let mut hits = 0;
    for x in 0..4 {
        for y in 0..4 {
            for z in 0..4 {
                let hit = tree.get(&V3c::new(x, y, z));
                if hit != BoxTreeEntry::Empty {
                    assert!(
                        hit == (&albedo).into(),
                        "Hit mismatch at {:?}: {:?} <> {:?}",
                        (x, y, z),
                        hit,
                        albedo,
                    );
                    hits += 1;
                }
            }
        }
    }

    // number of hits should be the number of nodes set minus the number of nodes cleared
    // in this case, clear size is taken as 2 as it is the smaller number where 2^x < clear_size < 2^(x+1)
    assert_eq!(hits, (64 - 27));
}

#[test]
fn test_clear_at_lod_with_unaligned_size_where_dim_is_4() {
    let albedo: Albedo = 0xFFAAEEFF.into();
    let mut tree: BoxTree = BoxTree::new(16, 4).ok().unwrap();
    tree.insert_at_lod(&V3c::new(0, 0, 0), 4, &albedo)
        .ok()
        .unwrap();
    let mut hits = 0;
    for x in 0..8 {
        for y in 0..8 {
            for z in 0..8 {
                let hit = tree.get(&V3c::new(x, y, z));
                if hit != BoxTreeEntry::Empty {
                    assert!(
                        hit == (&albedo).into(),
                        "Hit mismatch at {:?}: {:?} <> {:?}",
                        (x, y, z),
                        hit,
                        albedo,
                    );
                    hits += 1;
                }
            }
        }
    }
    assert_eq!(hits, 64);

    tree.clear_at_lod(&V3c::new(0, 0, 0), 3).ok().unwrap();
    let mut hits = 0;
    for x in 0..8 {
        for y in 0..8 {
            for z in 0..8 {
                let hit = tree.get(&V3c::new(x, y, z));
                if hit != BoxTreeEntry::Empty {
                    assert!(
                        hit == (&albedo).into(),
                        "Hit mismatch at {:?}: {:?} <> {:?}",
                        (x, y, z),
                        hit,
                        albedo,
                    );
                    hits += 1;
                }
            }
        }
    }

    // number of hits should be the number of nodes set minus the number of nodes cleared
    assert_eq!(hits, (64 - 27));
}

#[test]
fn test_clear_whole_nodes_where_dim_is_4() {
    let albedo: Albedo = 0xFFAAEEFF.into();
    let mut tree: BoxTree = BoxTree::new(16, 4).ok().unwrap();
    tree.insert_at_lod(&V3c::new(0, 0, 0), 8, &albedo)
        .ok()
        .unwrap();
    let mut hits = 0;
    for x in 0..8 {
        for y in 0..8 {
            for z in 0..8 {
                let hit = tree.get(&V3c::new(x, y, z));
                if hit != BoxTreeEntry::Empty {
                    assert!(
                        hit == (&albedo).into(),
                        "Hit mismatch at {:?}: {:?} <> {:?}",
                        (x, y, z),
                        hit,
                        albedo,
                    );
                    hits += 1;
                }
            }
        }
    }
    assert_eq!(hits, 512);

    tree.clear_at_lod(&V3c::new(0, 0, 0), 5).ok().unwrap();
    let mut hits = 0;
    for x in 0..8 {
        for y in 0..8 {
            for z in 0..8 {
                let hit = tree.get(&V3c::new(x, y, z));
                if hit != BoxTreeEntry::Empty {
                    assert!(
                        hit == (&albedo).into(),
                        "Hit mismatch at {:?}: {:?} <> {:?}",
                        (x, y, z),
                        hit,
                        albedo,
                    );
                    hits += 1;
                }
            }
        }
    }

    // number of hits should be the number of nodes set minus the number of nodes cleared
    assert_eq!(hits, (512 - 64));
}

#[test]
fn test_overwrite_whole_nodes_where_dim_is_4() {
    let red: Albedo = 0xFF0000FF.into();
    let blue: Albedo = 0x0000FFFF.into();
    let mut tree: BoxTree = BoxTree::new(16, 4).ok().unwrap();
    tree.insert_at_lod(&V3c::new(0, 0, 0), 8, &red)
        .ok()
        .unwrap();
    let mut hits = 0;
    for x in 0..8 {
        for y in 0..8 {
            for z in 0..8 {
                let hit = tree.get(&V3c::new(x, y, z));
                if hit != BoxTreeEntry::Empty {
                    assert!(
                        hit == (&red).into(),
                        "Hit mismatch at {:?}: {:?} <> {:?}",
                        (x, y, z),
                        hit,
                        red,
                    );
                    hits += 1;
                }
            }
        }
    }
    assert_eq!(hits, 512);

    tree.insert_at_lod(&V3c::new(0, 0, 0), 5, &blue)
        .ok()
        .unwrap();
    let mut hits_red = 0;
    let mut hits_blue = 0;
    for x in 0..8 {
        for y in 0..8 {
            for z in 0..8 {
                let hit = tree.get(&V3c::new(x, y, z));
                assert_ne!(hit, BoxTreeEntry::Empty);
                match hit {
                    BoxTreeEntry::Visual(hit) => {
                        if *hit == red {
                            hits_red += 1
                        }
                        if *hit == blue {
                            hits_blue += 1
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    assert_eq!(hits_red, (512 - 64));
    assert_eq!(hits_blue, (64));
}

#[test]
fn test_edge_case_boxtree_set() {
    const TREE_SIZE: u32 = 16;
    const FILL_RANGE_START: u32 = 6;
    let mut tree: BoxTree = BoxTree::new(TREE_SIZE, 1).ok().unwrap();
    for x in FILL_RANGE_START..TREE_SIZE {
        for y in FILL_RANGE_START..TREE_SIZE {
            for z in FILL_RANGE_START..TREE_SIZE {
                let pos = V3c::new(x, y, z);
                tree.insert(&pos, &Albedo::from(x + y + z)).ok().unwrap();
                assert!(tree.get(&pos) == (&Albedo::from(x + y + z)).into());
            }
        }
    }
}

#[test]
fn test_case_inserting_empty() {
    let mut tree: BoxTree = BoxTree::new(4, 1).ok().unwrap();
    tree.insert(&V3c::new(3, 0, 0), &Albedo::zero())
        .ok()
        .unwrap();
    let item = tree.get(&V3c::new(3, 0, 0));
    assert!(
        item == BoxTreeEntry::Empty,
        "Item shouldn't exist: {:?}",
        item
    );
}
