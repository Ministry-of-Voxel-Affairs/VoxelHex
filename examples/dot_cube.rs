#[cfg(feature = "bevy_wgpu")]
use bevy::{prelude::*, window::WindowPlugin};

#[cfg(feature = "bevy_wgpu")]
use bevy_panorbit_camera::{PanOrbitCamera, PanOrbitCameraPlugin};

#[cfg(feature = "bevy_wgpu")]
use iyes_perf_ui::{
    entries::diagnostics::{PerfUiEntryFPS, PerfUiEntryFPSWorst},
    ui::root::PerfUiRoot,
    PerfUiPlugin,
};

#[cfg(feature = "bevy_wgpu")]
use voxelhex::{
    boxtree::{Albedo, BoxTree, BoxTreeEntry, V3c, V3cf32},
    raytracing::{BoxTreeGPUHost, Ray, VhxViewSet, Viewport},
};

#[cfg(feature = "bevy_wgpu")]
const DISPLAY_RESOLUTION: [u32; 2] = [1024, 768];

#[cfg(feature = "bevy_wgpu")]
const BRICK_DIMENSION: u32 = 32;

#[cfg(feature = "bevy_wgpu")]
const TREE_SIZE: u32 = 128;

#[cfg(feature = "bevy_wgpu")]
fn main() {
    App::new()
        .insert_resource(ClearColor(Color::BLACK))
        .add_plugins((
            DefaultPlugins.set(WindowPlugin::default()),
            voxelhex::raytracing::RenderBevyPlugin::<u32>::new(),
            bevy::diagnostic::FrameTimeDiagnosticsPlugin,
            PanOrbitCameraPlugin,
            PerfUiPlugin,
        ))
        .add_systems(Startup, setup)
        .add_systems(Update, set_viewport_for_camera)
        .add_systems(Update, handle_zoom)
        .run();
}

#[cfg(feature = "bevy_wgpu")]
fn setup(mut commands: Commands, images: ResMut<Assets<Image>>) {
    let origin = V3c::new(
        TREE_SIZE as f32 * 2.,
        TREE_SIZE as f32 / 2.,
        TREE_SIZE as f32 * -2.,
    );

    // fill boxtree with data
    let mut tree: BoxTree = voxelhex::boxtree::BoxTree::new(TREE_SIZE, BRICK_DIMENSION)
        .ok()
        .unwrap();

    for x in 0..TREE_SIZE {
        for y in 0..TREE_SIZE {
            for z in 0..TREE_SIZE {
                if ((x < (TREE_SIZE / 4) || y < (TREE_SIZE / 4) || z < (TREE_SIZE / 4))
                    && (0 == x % 2 && 0 == y % 4 && 0 == z % 2))
                    || ((TREE_SIZE / 2) <= x && (TREE_SIZE / 2) <= y && (TREE_SIZE / 2) <= z)
                {
                    let r = if 0 == x % (TREE_SIZE / 4) {
                        (x as f32 / TREE_SIZE as f32 * 255.) as u32
                    } else {
                        128
                    };
                    let g = if 0 == y % (TREE_SIZE / 4) {
                        (y as f32 / TREE_SIZE as f32 * 255.) as u32
                    } else {
                        128
                    };
                    let b = if 0 == z % (TREE_SIZE / 4) {
                        (z as f32 / TREE_SIZE as f32 * 255.) as u32
                    } else {
                        128
                    };
                    tree.insert(
                        &V3c::new(x, y, z),
                        &Albedo::default()
                            .with_red(r as u8)
                            .with_green(g as u8)
                            .with_blue(b as u8)
                            .with_alpha(255),
                    )
                    .ok()
                    .unwrap();
                    assert_eq!(
                        tree.get(&V3c::new(x, y, z)),
                        BoxTreeEntry::Visual(
                            &Albedo::default()
                                .with_red(r as u8)
                                .with_green(g as u8)
                                .with_blue(b as u8)
                                .with_alpha(255)
                        )
                    );
                }
            }
        }
    }

    let mut host = BoxTreeGPUHost { tree };
    let mut views = VhxViewSet::default();
    let view_index = host.create_new_view(
        &mut views,
        50,
        Viewport::new(
            origin,
            (V3c::new(0., 0., 0.) - origin).normalized(),
            V3c::new(10., 10., 200.),
            3.,
        ),
        DISPLAY_RESOLUTION,
        images,
    );
    commands.insert_resource(host);

    let mut display = Sprite::from_image(
        views.views[view_index]
            .lock()
            .unwrap()
            .output_texture()
            .clone(),
    );
    display.custom_size = Some(Vec2::new(1024., 768.));
    commands.spawn(display);
    commands.insert_resource(views);
    commands.spawn((
        Camera {
            is_active: false,
            ..default()
        },
        PanOrbitCamera {
            focus: Vec3::new(0., TREE_SIZE as f32 * 1.2, 0.),
            ..default()
        },
    ));
    commands.spawn(Camera2d::default());
    commands.spawn((
        PerfUiRoot::default(),
        PerfUiEntryFPS {
            label: "Frame Rate (current)".into(),
            threshold_highlight: Some(60.0),
            digits: 5,
            precision: 2,
            ..default()
        },
        PerfUiEntryFPSWorst {
            label: "Frame Rate (worst)".into(),
            threshold_highlight: Some(60.0),
            digits: 5,
            precision: 2,
            ..default()
        },
    ));
}

#[cfg(feature = "bevy_wgpu")]
fn direction_from_cam(cam: &PanOrbitCamera) -> Option<V3cf32> {
    if let Some(radius) = cam.radius {
        Some(
            V3c::new(
                radius / 2. + cam.yaw.unwrap().sin() * radius,
                radius + cam.pitch.unwrap().sin() * radius * 2.,
                radius / 2. + cam.yaw.unwrap().cos() * radius,
            )
            .normalized(),
        )
    } else {
        None
    }
}

#[cfg(feature = "bevy_wgpu")]
fn set_viewport_for_camera(camera_query: Query<&mut PanOrbitCamera>, view_set: ResMut<VhxViewSet>) {
    let cam = camera_query.single();
    if let Some(_) = cam.radius {
        let mut tree_view = view_set.views[0].lock().unwrap();
        tree_view.spyglass.viewport_mut().origin = V3c::new(cam.focus.x, cam.focus.y, cam.focus.z);
        tree_view.spyglass.viewport_mut().direction = direction_from_cam(cam).unwrap();
    }
}

#[cfg(feature = "bevy_wgpu")]
fn handle_zoom(
    keys: Res<ButtonInput<KeyCode>>,
    tree: ResMut<BoxTreeGPUHost>,
    view_set: ResMut<VhxViewSet>,
    mut camera_query: Query<&mut PanOrbitCamera>,
) {
    let mut tree_view = view_set.views[0].lock().unwrap();

    if keys.pressed(KeyCode::Tab) {
        // Render the current view with CPU
        const CPU_DISPLAY_RESOLUTION: [u32; 2] = [64, 64];
        let viewport_up_direction = V3c::new(0., 1., 0.);
        let viewport_right_direction = viewport_up_direction
            .cross(tree_view.spyglass.viewport().direction)
            .normalized();
        let pixel_width =
            tree_view.spyglass.view_frustum().x as f32 / CPU_DISPLAY_RESOLUTION[0] as f32;
        let pixel_height =
            tree_view.spyglass.view_frustum().y as f32 / CPU_DISPLAY_RESOLUTION[1] as f32;
        let viewport_bottom_left = tree_view.spyglass.viewport().origin
            + (tree_view.spyglass.viewport().direction * tree_view.spyglass.view_fov())
            - (viewport_up_direction * (tree_view.spyglass.view_frustum().y / 2.))
            - (viewport_right_direction * (tree_view.spyglass.view_frustum().x / 2.));

        // define light
        let diffuse_light_normal = V3c::new(0., -1., 1.).normalized();

        use image::ImageBuffer;
        use image::Rgb;
        let mut img = ImageBuffer::new(CPU_DISPLAY_RESOLUTION[0], CPU_DISPLAY_RESOLUTION[1]);

        // cast each ray for a hit
        for x in 0..CPU_DISPLAY_RESOLUTION[0] {
            for y in 0..CPU_DISPLAY_RESOLUTION[1] {
                let actual_y_in_image = CPU_DISPLAY_RESOLUTION[1] - y - 1;
                //from the origin of the camera to the current point of the viewport
                let glass_point = viewport_bottom_left
                    + viewport_right_direction * x as f32 * pixel_width
                    + viewport_up_direction * y as f32 * pixel_height;
                let ray = Ray {
                    origin: tree_view.spyglass.viewport().origin,
                    direction: (glass_point - tree_view.spyglass.viewport().origin).normalized(),
                };

                use std::io::Write;
                std::io::stdout().flush().ok().unwrap();

                if let Some(hit) = tree.tree.get_by_ray(&ray) {
                    let (data, _, normal) = hit;
                    //Because both vector should be normalized, the dot product should be 1*1*cos(angle)
                    //That means it is in range -1, +1, which should be accounted for
                    let diffuse_light_strength =
                        1. - (normal.dot(&diffuse_light_normal) / 2. + 0.5);
                    img.put_pixel(
                        x,
                        actual_y_in_image,
                        Rgb([
                            (data.albedo().unwrap().r as f32 * diffuse_light_strength) as u8,
                            (data.albedo().unwrap().g as f32 * diffuse_light_strength) as u8,
                            (data.albedo().unwrap().b as f32 * diffuse_light_strength) as u8,
                        ]),
                    );
                } else {
                    img.put_pixel(x, actual_y_in_image, Rgb([128, 128, 128]));
                }
            }
        }

        img.save("example_junk_cpu_render.png").ok().unwrap();
    }

    if keys.pressed(KeyCode::Home) {
        tree_view.spyglass.viewport_mut().fov *= 1. + 0.00009;
    }
    if keys.pressed(KeyCode::End) {
        tree_view.spyglass.viewport_mut().fov *= 1. - 0.00009;
    }

    const MOVEMENT_MODIF: f32 = 0.75;
    let mut cam = camera_query.single_mut();
    if keys.pressed(KeyCode::ShiftLeft) {
        cam.target_focus.y += MOVEMENT_MODIF;
    }
    if keys.pressed(KeyCode::ControlLeft) {
        cam.target_focus.y -= MOVEMENT_MODIF;
    }
    if keys.pressed(KeyCode::NumpadAdd) {
        tree_view.spyglass.viewport_mut().frustum.z *= 1.01;
    }
    if keys.pressed(KeyCode::NumpadSubtract) {
        tree_view.spyglass.viewport_mut().frustum.z *= 0.99;
    }

    if let Some(_) = cam.radius {
        let dir = direction_from_cam(&cam).unwrap();
        let dir = Vec3::new(dir.x, dir.y, dir.z);
        let right = dir.cross(Vec3::new(0., 1., 0.)) * 0.1;
        if keys.pressed(KeyCode::KeyW) {
            cam.target_focus += dir * MOVEMENT_MODIF;
        }
        if keys.pressed(KeyCode::KeyS) {
            cam.target_focus -= dir * MOVEMENT_MODIF;
        }
        if keys.pressed(KeyCode::KeyA) {
            cam.target_focus += right * MOVEMENT_MODIF;
        }
        if keys.pressed(KeyCode::KeyD) {
            cam.target_focus -= right * MOVEMENT_MODIF;
        }
    }
}

#[cfg(not(feature = "bevy_wgpu"))]
fn main() {
    println!("You probably forgot to enable the bevy_wgpu feature!");
    //nothing to do when the feature is not enabled
}
