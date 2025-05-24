mod ui;

use bevy::{
    prelude::*,
    render::view::RenderLayers,
    tasks::{block_on, futures_lite::future, AsyncComputeTaskPool, Task},
    window::WindowPlugin,
};
use bevy_lunex::prelude::*;
use bevy_panorbit_camera::{PanOrbitCamera, PanOrbitCameraPlugin};
use bevy_pkv::PkvStore;
use std::{ffi::OsStr, path::Path};
use ui::{components::*, UiState};
use voxelhex::{
    boxtree::{BoxTree, V3c},
    raytracing::{BoxTreeGPUHost, VhxViewSet},
};

const BRICK_DIMENSION: u32 = 32;

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
            bevy::diagnostic::FrameTimeDiagnosticsPlugin::default(),
            PanOrbitCameraPlugin,
            UiLunexPlugins,
            //UiLunexDebugPlugin::<1, 2>,
        ))
        .add_systems(Startup, (ui::layout::setup, setup))
        .add_systems(
            Startup,
            ((
                ui::input::setup_mouse_action,
                ui::behavior::setup,
                ui::behavior::messages,
                load_last_loaded_model,
            )
                .after(crate::ui::layout::setup),),
        )
        .add_systems(
            Update,
            (
                ui::behavior::update,
                ui::input::mouse_action_cleanup,
                ui::input::handle_settings_update,
                ui::input::handle_camera_update,
                ui::input::handle_world_interaction_block_by_ui,
                observe_file_drop,
                handle_model_load,
            ),
        )
        .insert_resource(ui_state)
        .insert_resource(preferences)
        .insert_resource(VhxViewSet::default())
        .add_observer(ui::behavior::resolution_changed_observer)
        .run();
}

#[derive(Resource)]
struct TreeLoadingTask {
    task: Task<BoxTree<u32>>,
    tmp_file_path: String,
    target_cache_file_path: String,
    confirmed: bool,
}

fn observe_file_drop(
    mut commands: Commands,
    mut evr_dnd: EventReader<FileDragAndDrop>,
    mut tree_factory: Option<ResMut<TreeLoadingTask>>,
) {
    for ev in evr_dnd.read() {
        match ev {
            FileDragAndDrop::HoveredFile {
                window: _,
                path_buf,
            } => {
                let thread_pool = AsyncComputeTaskPool::get();
                let tree_file_name = path_buf
                    .file_stem()
                    .unwrap_or_else(|| OsStr::new("unknwon"))
                    .to_str()
                    .unwrap_or_else(|| "name_conversion_failed");
                let model_path = path_buf
                    .to_str()
                    .unwrap_or_else(|| "name_conversion_failed")
                    .to_string();
                let tmp_file_path_ = ".tmp_cache_".to_string() + tree_file_name;
                let tmp_file_path = tmp_file_path_.to_string();
                let target_cache_file_path = ".cache_".to_string() + tree_file_name;
                let task = thread_pool.spawn(async move {
                    //TODO: signal, loading progress in async task
                    let tree: BoxTree;
                    if Path::new(&tmp_file_path_).exists() {
                        tree = BoxTree::load(&tmp_file_path_).ok().unwrap();
                    } else {
                        tree = BoxTree::load_vox_file(model_path.as_str(), BRICK_DIMENSION)
                            .expect("Parsing model file failed: ");
                        tree.save(&tmp_file_path_).ok().unwrap();
                    }
                    tree
                });
                commands.insert_resource(TreeLoadingTask {
                    task,
                    confirmed: false,
                    tmp_file_path,
                    target_cache_file_path,
                });
            }
            FileDragAndDrop::DroppedFile {
                window: _,
                path_buf,
            } => {
                let tree_facory = tree_factory
                    .as_mut()
                    .expect("Expected available tree loading task upon model load cancellation");
                #[cfg(debug_assertions)]
                {
                    let tree_file_name = path_buf
                        .file_stem()
                        .unwrap_or_else(|| OsStr::new("unknwon"))
                        .to_str()
                        .unwrap_or_else(|| "name_conversion_failed");
                    let tmp_file_path_ = ".tmp_cache_".to_string() + tree_file_name;
                    debug_assert!(tmp_file_path_ == tree_facory.tmp_file_path);
                }
                tree_facory.confirmed = true;
            }
            FileDragAndDrop::HoveredFileCanceled { window: _ } => {
                debug_assert!(tree_factory.is_some() && !tree_factory.as_ref().unwrap().confirmed);
                let tmp_file_path = Path::new(&tree_factory.as_ref().unwrap().tmp_file_path);
                if tmp_file_path.exists() {
                    std::fs::remove_file(tmp_file_path)
                        .expect("Expected to be able to remove temporary file at {tmp_file_path}");
                }
                commands.remove_resource::<TreeLoadingTask>();
            }
        }
    }
}

fn handle_model_load(
    mut commands: Commands,
    mut pkv: ResMut<PkvStore>,
    images: ResMut<Assets<Image>>,
    mut viewset: ResMut<VhxViewSet>,
    mut ui_state: ResMut<ui::UiState>,
    tree_factory: Option<ResMut<TreeLoadingTask>>,
    mut view_output: Query<(&mut Sprite, &Model, &Output, &Container)>,
    mut status_text: Query<(&mut Text2d, &Model, &Status)>,
) {
    if let Some(mut tree_factory) = tree_factory {
        if tree_factory.confirmed {
            if let Some(tree) = block_on(future::poll_once(&mut tree_factory.task)) {
                debug_assert!(
                    Path::new(&tree_factory.tmp_file_path).exists(),
                    "Expected {:?} to exist after tree load is completed!",
                    tree_factory.tmp_file_path
                );
                if tree_factory.tmp_file_path != tree_factory.target_cache_file_path {
                    std::fs::rename(
                        &tree_factory.tmp_file_path,
                        &tree_factory.target_cache_file_path,
                    )
                    .expect("Expected to be able to remove temporary file at {tmp_file_path}");
                }

                let mut host = BoxTreeGPUHost { tree };
                let view_index = host.create_new_view(
                    &mut viewset,
                    50,
                    voxelhex::raytracing::Viewport::new(
                        V3c::new(0., 10., 0.),
                        V3c::new(0., 0., 1.),
                        V3c::new(
                            ui_state.viewport_resolution[0] as f32,
                            ui_state.viewport_resolution[1] as f32,
                            ui_state.view_distance as f32,
                        ),
                        ui_state.fov_value as f32,
                    ),
                    ui_state.output_resolution,
                    images,
                );

                // Set output render as tree view output
                let (mut output_sprite, _, _, _) = view_output
                    .single_mut()
                    .expect("Expected to have model output image available in UI!");
                *output_sprite = Sprite::from_image(
                    viewset.views[view_index]
                        .lock()
                        .unwrap()
                        .output_texture()
                        .clone(),
                );

                // Insert the tree resource
                ui_state.model_loaded = true;
                commands.insert_resource(host);
                commands.remove_resource::<TreeLoadingTask>();
                pkv.set("last_loaded_model", &tree_factory.target_cache_file_path)
                    .expect("Expected to be able to store last_loaded_model setting");
            }
        }
    }
}

fn load_last_loaded_model(
    pkv: Res<PkvStore>,
    mut commands: Commands,
    mut ui_state: ResMut<UiState>,
    tree_factory: Option<Res<TreeLoadingTask>>,
) {
    if !ui_state.model_loaded && tree_factory.is_none() {
        if let Ok(file_path) = pkv.get::<String>("last_loaded_model") {
            let thread_pool = AsyncComputeTaskPool::get();
            let file_path_ = file_path.to_string();
            let task = thread_pool.spawn(async move { BoxTree::load(&file_path).ok().unwrap() });
            commands.insert_resource(TreeLoadingTask {
                task,
                confirmed: true,
                tmp_file_path: file_path_.to_string(),
                target_cache_file_path: file_path_,
            });
            ui_state.model_loaded = true;
        }
    }
}

fn init_preferences_cache() -> PkvStore {
    let mut pkv = PkvStore::new("MinistryOfVoxelAffairs", "Whisp");
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
        pkv.set("view_distance", &"1024")
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
            is_active: false,
            ..default()
        },
        PanOrbitCamera {
            focus: Vec3::new(0., 300., 0.),
            ..default()
        },
    ));
    commands.spawn((
        Camera2d,
        UiSourceCamera::<0>,
        Transform::from_translation(Vec3::Z * 1000.0),
        RenderLayers::from_layers(&[0, 1]),
    ));
}
