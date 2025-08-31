mod loader;
mod ui;

use crate::ui::input::CameraPosition;
use bevy::{
    diagnostic::FrameTimeDiagnosticsPlugin, prelude::*, render::view::RenderLayers,
    window::WindowPlugin,
};
use bevy_lunex::prelude::*;
use bevy_panorbit_camera::{PanOrbitCamera, PanOrbitCameraPlugin};
use bevy_pkv::PkvStore;
use voxelhex::raytracing::VhxViewSet;

fn main() {
    let preferences = init_preferences_cache();
    let ui_state = ui::UiState::new(&preferences);

    App::new()
        .insert_resource(ClearColor(Color::BLACK))
        .add_plugins((
            DefaultPlugins.set(WindowPlugin {
                primary_window: Some(Window {
                    // uncomment for unthrottled FPS
                    present_mode: bevy::window::PresentMode::AutoNoVsync,
                    title: "Whisp - Press g to hide/show UI".to_string(),
                    ..default()
                }),
                ..default()
            }),
            voxelhex::raytracing::RenderBevyPlugin::<u32>::new(),
            FrameTimeDiagnosticsPlugin::new(300),
            PanOrbitCameraPlugin,
            UiLunexPlugins,
            // UiLunexDebugPlugin::<1, 2>,
        ))
        .add_systems(Startup, (ui::layout::setup, setup))
        .add_systems(
            Startup,
            (ui::behavior::setup, ui::input::setup_mouse_action).after(crate::ui::layout::setup),
        )
        .add_systems(
            Startup,
            loader::load_last_loaded_model.after(ui::behavior::setup),
        )
        .add_systems(
            Update,
            (
                ui::behavior::update,
                ui::input::mouse_action_cleanup,
                ui::input::handle_settings_update,
                ui::input::handle_camera_update,
                ui::input::handle_world_interaction_block_by_ui,
            ),
        )
        .add_systems(
            FixedUpdate,
            (
                ui::input::init_camera.run_if(run_once),
                ui::behavior::handle_model_load_animation,
                ui::behavior::update_performance_stats,
                ui::behavior::update_output_resolution_and_view_dist,
                ui::behavior::handle_ui_hidden,
                loader::observe_file_drop,
                loader::handle_model_load_finished,
            ),
        )
        .add_systems(EguiPrimaryContextPass, ui::behavior::fps_graph_system)
        .insert_resource(ui_state)
        .insert_resource(preferences)
        .insert_resource(VhxViewSet::new())
        .add_observer(ui::behavior::resolution_changed_observer)
        .add_observer(ui::behavior::settings_changed_observer)
        .run();
}

fn init_preferences_cache() -> PkvStore {
    let mut pkv = PkvStore::new("MinistryOfVoxelAffairs", "Whisp");

    if pkv.get::<CameraPosition>("camera_position").is_err() {
        pkv.set("CameraPosition", &CameraPosition::baked_model_pose())
            .expect("Failed to store default value: camera_position");
    }
    if pkv.get::<String>("camera_locked").is_err() {
        pkv.set("camera_locked", &"false")
            .expect("Failed to store default value: camera_locked");
    }
    if pkv.get::<String>("output_resolution_width").is_err() {
        pkv.set("output_resolution_width", &"1920")
            .expect("Failed to store default value: output_resolution_width");
    }
    if pkv.get::<String>("output_resolution_height").is_err() {
        pkv.set("output_resolution_height", &"1080")
            .expect("Failed to store default value: output_resolution_height");
    }
    if pkv.get::<String>("fov").is_err() {
        pkv.set("fov", &"50")
            .expect("Failed to store default value: fov");
    }
    if pkv.get::<String>("viewport_resolution_width").is_err() {
        pkv.set("viewport_resolution_width", &"100")
            .expect("Failed to store default value: viewport_resolution_width");
    }
    if pkv.get::<String>("viewport_resolution_height").is_err() {
        pkv.set("viewport_resolution_height", &"100")
            .expect("Failed to store default value: viewport_resolution_height");
    }
    if pkv.get::<String>("view_distance").is_err() {
        pkv.set("view_distance", &"512")
            .expect("Failed to store default value: view_distance");
    }
    if pkv.get::<String>("ui_hidden").is_err() {
        pkv.set("ui_hidden", &"false")
            .expect("Failed to store default value: ui_hidden");
    }
    if pkv.get::<String>("shortcuts_hidden").is_err() {
        pkv.set("shortcuts_hidden", &"false")
            .expect("Failed to store default value: shortcuts_hidden");
    }
    if pkv.get::<String>("output_resolution_linked").is_err() {
        pkv.set("output_resolution_linked", &"false")
            .expect("Failed to store default value: output_resolution_linked");
    }
    if pkv.get::<String>("viewport_resolution_linked").is_err() {
        pkv.set("viewport_resolution_linked", &"false")
            .expect("Failed to store default value: viewport_resolution_linked");
    }
    pkv
}

fn setup(mut commands: Commands) {
    commands.spawn((
        bevy::prelude::Camera {
            is_active: true,
            ..default()
        },
        PanOrbitCamera {
            focus: Vec3::new(0., 100., 0.),
            ..default()
        },
    ));

    commands.spawn((
        Camera {
            order: 1,
            ..default()
        },
        Camera2d::default(),
        UiSourceCamera::<0>,
        RenderLayers::from_layers(&[0, 1]),
    ));

    commands.spawn((
        Camera {
            order: 2,
            ..default()
        },
        Camera2d::default(),
        EguiContext::default(),
    ));  // separate camera for egui to fix warning from bevy(multiple contexts cannot write to single camera)

