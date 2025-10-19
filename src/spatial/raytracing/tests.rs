#[cfg(test)]
#[cfg(feature = "bevy_wgpu")]
mod brick_tests {

    use crate::{boxtree::V3c, spatial::Cube};

    #[test]
    fn test_brick_slot_creation() {
        const BRICK_DIM: u32 = 32;
        let brick_slot = Cube::brick_slot_for(&V3c::new(0., 0., 0.), BRICK_DIM);
        assert_eq!(brick_slot.size, BRICK_DIM as f32);
        assert_eq!(brick_slot.min_position, V3c::new(0., 0., 0.));

        let brick_slot = Cube::brick_slot_for(&V3c::new(3., 5., 12.), BRICK_DIM);
        assert_eq!(brick_slot.size, BRICK_DIM as f32);
        assert_eq!(brick_slot.min_position, V3c::new(0., 0., 0.));

        let brick_slot = Cube::brick_slot_for(&V3c::new(0., 5., 31.), BRICK_DIM);
        assert_eq!(brick_slot.size, BRICK_DIM as f32);
        assert_eq!(brick_slot.min_position, V3c::new(0., 0., 0.));

        let brick_slot = Cube::brick_slot_for(&V3c::new(16., 5., 32.), BRICK_DIM);
        assert_eq!(brick_slot.size, BRICK_DIM as f32);
        assert_eq!(brick_slot.min_position, V3c::new(0., 0., 32.));
    }
}

#[cfg(test)]
mod raytracing_tests {
    use crate::spatial::{
        math::offset_sectant, raytracing::plane_line_intersection, raytracing::Ray, Cube, V3c,
    };

    #[test]
    fn test_plane_line_intersection() {
        assert!(
            plane_line_intersection(
                // plane
                &V3c::new(0., 0., 0.),
                &V3c::new(0., 1., 0.),
                // line
                &V3c::new(0., 1., 0.),
                &V3c::new(1., 0., 0.),
            ) == None
        );

        assert!(
            plane_line_intersection(
                // plane
                &V3c::new(0., 0., 0.),
                &V3c::new(0., 1., 0.),
                // line
                &V3c::new(0., 1., 0.),
                &V3c::new(0., -1., 0.),
            ) == Some(1.)
        );

        assert!(
            plane_line_intersection(
                // plane
                &V3c::new(0., 0., 0.),
                &V3c::new(0., 1., 0.),
                // line
                &V3c::new(0., 0., 0.),
                &V3c::new(1., 0., 0.),
            ) == Some(0.)
        );
    }

    #[test]
    fn test_cube_bounds() {
        let size = 20.;
        let cube = Cube {
            min_position: V3c::default(),
            size,
        };

        // Test front bottom left
        let bound_fbl = Cube::child_bounds_for(&cube, offset_sectant(&V3c::unit(0.0), size));
        assert_eq!(bound_fbl.min_position, V3c::unit(0.0));
        assert_eq!(bound_fbl.size, 5.0);

        // Test front bottom right
        let bound_fbl = Cube::child_bounds_for(&cube, offset_sectant(&V3c::new(17., 0., 0.), size));
        assert_eq!(bound_fbl.min_position, V3c::new(15.0, 0.0, 0.0));
        assert_eq!(bound_fbl.size, 5.0);

        // Test back bottom left
        let bound_fbl = Cube::child_bounds_for(&cube, offset_sectant(&V3c::new(0., 0., 17.), size));
        assert_eq!(bound_fbl.min_position, V3c::new(0.0, 0.0, 15.0));
        assert_eq!(bound_fbl.size, 5.0);

        // Test back bottom right
        let bound_fbl =
            Cube::child_bounds_for(&cube, offset_sectant(&V3c::new(17., 0., 17.), size));
        assert_eq!(bound_fbl.min_position, V3c::new(15.0, 0.0, 15.0));
        assert_eq!(bound_fbl.size, 5.0);

        // Test front top left
        let bound_fbl = Cube::child_bounds_for(&cube, offset_sectant(&V3c::new(0., 17., 0.), size));
        assert_eq!(bound_fbl.min_position, V3c::new(0.0, 15.0, 0.0));
        assert_eq!(bound_fbl.size, 5.0);

        // Test front top right
        let bound_fbl =
            Cube::child_bounds_for(&cube, offset_sectant(&V3c::new(17., 17., 0.), size));
        assert_eq!(bound_fbl.min_position, V3c::new(15.0, 15.0, 0.0));
        assert_eq!(bound_fbl.size, 5.0);

        // Test back top left
        let bound_fbl =
            Cube::child_bounds_for(&cube, offset_sectant(&V3c::new(0., 17., 17.), size));
        assert_eq!(bound_fbl.min_position, V3c::new(0.0, 15.0, 15.0));
        assert_eq!(bound_fbl.size, 5.0);

        // Test back top right
        let bound_fbl =
            Cube::child_bounds_for(&cube, offset_sectant(&V3c::new(17., 17., 17.), size));
        assert_eq!(bound_fbl.min_position, V3c::new(15.0, 15.0, 15.0));
        assert_eq!(bound_fbl.size, 5.0);
    }

    #[test]
    fn test_cube_contains_ray() {
        let cube = Cube {
            min_position: V3c::unit(0.0),
            size: 4.0,
        };

        let ray_above = Ray {
            origin: V3c {
                x: 2.,
                y: 5.,
                z: 2.,
            },
            direction: V3c {
                x: 0.,
                y: -1.,
                z: 0.,
            },
        };
        assert!(cube.intersect_ray(&ray_above).is_some());

        let ray_below = Ray {
            origin: V3c {
                x: 2.,
                y: -5.,
                z: 2.,
            },
            direction: V3c {
                x: 0.,
                y: 1.,
                z: 0.,
            },
        };
        assert!(cube.intersect_ray(&ray_below).is_some());

        let ray_miss = Ray {
            origin: V3c {
                x: 2.,
                y: 5.,
                z: 2.,
            },
            direction: V3c {
                x: 0.,
                y: 1.,
                z: 0.,
            },
        };
        assert!(cube.intersect_ray(&ray_miss).is_none());

        let ray_hit = Ray {
            origin: V3c {
                x: -1.,
                y: -1.,
                z: -1.,
            },
            direction: V3c {
                x: 1.,
                y: 1.,
                z: 1.,
            }
            .normalized(),
        };

        assert!(cube.intersect_ray(&ray_hit).is_some());

        let corner_hit = Ray {
            origin: V3c {
                x: -1.,
                y: -1.,
                z: -1.,
            },
            direction: V3c {
                x: 1.,
                y: 1.,
                z: 1.,
            }
            .normalized(),
        };

        assert!(cube.intersect_ray(&corner_hit).is_some());

        let origin = V3c {
            x: 4.,
            y: -1.,
            z: 4.,
        };
        let corner_miss = Ray {
            direction: (V3c {
                x: 4.055,
                y: 4.055,
                z: 4.055,
            } - origin)
                .normalized(),
            origin,
        };
        assert!(!cube.intersect_ray(&corner_miss).is_some());

        let ray_still_miss = Ray {
            origin: V3c {
                x: -1.,
                y: -1.,
                z: -1.,
            },
            direction: V3c {
                x: 1.,
                y: 100.,
                z: 1.,
            }
            .normalized(),
        };
        assert!(cube.intersect_ray(&ray_still_miss).is_none());
    }

    #[test]
    fn test_edge_case_cube_intersect_inwards_pointing_vector() {
        let ray = Ray {
            origin: V3c {
                x: 8.0,
                y: 4.0,
                z: 5.0,
            },
            direction: V3c {
                x: -0.842701,
                y: -0.24077171,
                z: -0.48154342,
            },
        };
        let cube = Cube {
            min_position: V3c {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            size: 8.0,
        };
        let hit = cube.intersect_ray(&ray).unwrap();
        assert!(hit.impact_distance.is_some() && hit.impact_distance.unwrap() == 0.);
    }

    #[test]
    fn test_edge_case_cube_internal_ray_targeting_corners() {
        let ray = Ray {
            origin: V3c {
                x: 5.0,
                y: 8.0,
                z: 5.0,
            },
            direction: V3c {
                x: -0.48507127,
                y: -0.7276069,
                z: -0.48507127,
            },
        };
        let cube = Cube {
            min_position: V3c {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            size: 16.0,
        };
        let hit = cube.intersect_ray(&ray).unwrap();
        assert!(hit.impact_distance.is_none());
    }

    #[test]
    fn test_edge_case_cube_bottom_edge() {
        let ray = Ray {
            origin: V3c {
                x: 6.0,
                y: 7.0,
                z: 6.0,
            },
            direction: V3c {
                x: -0.6154574,
                y: -0.49236596,
                z: -0.6154574,
            },
        };
        let cube = Cube {
            min_position: V3c {
                x: 0.0,
                y: 2.0,
                z: 0.0,
            },
            size: 2.0,
        };
        let hit = cube.intersect_ray(&ray).unwrap();
        assert!(hit.impact_distance.is_some_and(|d| d > 0.0));
    }
}
