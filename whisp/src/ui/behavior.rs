use crate::{ui::components::*, ui::UiState};
use bevy::{input::mouse::MouseMotion, prelude::*};
use bevy_lunex::prelude::*;
use bevy_pkv::PkvStore;

enum ResolutionUpdated {
    OutputWidth,
    OutputHeight,
    ViewportWidth,
    ViewportHeight,
}

#[derive(Event)]
#[event(auto_propagate)]
pub(crate) struct OutputResolutionUpdated {
    by: ResolutionUpdated,
    from: u32,
    to: u32,
}

pub(crate) fn resolution_changed_observer(
    update: Trigger<OutputResolutionUpdated>,
    mut ui_state: ResMut<UiState>,
    mut output_width_button: Query<(
        Entity,
        &Output,
        &Width,
        &crate::ui::components::Button,
        &mut Text2d,
    )>,
    mut output_height_button: Query<
        (
            Entity,
            &Output,
            &Height,
            &crate::ui::components::Button,
            &mut Text2d,
        ),
        Without<Width>,
    >,
    mut viewport_width_button: Query<
        (
            Entity,
            &crate::ui::components::Camera,
            &Width,
            &crate::ui::components::Button,
            &mut Text2d,
        ),
        Without<Output>,
    >,
    mut viewport_height_button: Query<
        (
            Entity,
            &crate::ui::components::Camera,
            &Height,
            &crate::ui::components::Button,
            &mut Text2d,
        ),
        (Without<Width>, Without<Output>),
    >,
) {
    match update.by {
        ResolutionUpdated::OutputWidth => {
            let (_, _, _, _, mut height_text) = output_height_button
                .single_mut()
                .expect("Expected Output Height Button to be available in UI");
            let ratio = height_text
                .0
                .parse::<i32>()
                .expect("Expected Output Height to be a parsable integer!")
                as f32
                / update.from as f32;
            let new_height = (update.to as f32 * ratio) as u32;
            height_text.0 = new_height.to_string();
            ui_state.output_resolution[1] = new_height;
        }
        ResolutionUpdated::OutputHeight => {
            let (_, _, _, _, mut width_text) = output_width_button
                .single_mut()
                .expect("Expected Output Width Button to be available in UI");
            let ratio = width_text
                .0
                .parse::<i32>()
                .expect("Expected Output Width to be a parsable integer!")
                as f32
                / update.from as f32;
            let new_width = (update.to as f32 * ratio) as u32;
            width_text.0 = new_width.to_string();
            ui_state.output_resolution[0] = new_width;
        }
        ResolutionUpdated::ViewportWidth => {
            let (_, _, _, _, mut height_text) = viewport_height_button
                .single_mut()
                .expect("Expected Viewport Height Button to be available in UI");
            let ratio = height_text
                .0
                .parse::<i32>()
                .expect("Expected Viewport Height to be a parsable integer!")
                as f32
                / update.from as f32;
            let new_height = (update.to as f32 * ratio) as u32;
            height_text.0 = new_height.to_string();
            ui_state.viewport_resolution[1] = new_height;
        }
        ResolutionUpdated::ViewportHeight => {
            let (_, _, _, _, mut width_text) = viewport_width_button
                .single_mut()
                .expect("Expected Viewport Width Button to be available in UI");
            let ratio = width_text
                .0
                .parse::<i32>()
                .expect("Expected Viewport Height to be a parsable integer!")
                as f32
                / update.from as f32;
            let new_width = (update.to as f32 * ratio) as u32;
            width_text.0 = new_width.to_string();
            ui_state.viewport_resolution[0] = new_width;
        }
    }
}

pub(crate) fn setup(
    mut commands: Commands,
    output_resolution_linked: Query<(Entity, &Output, &Link, &crate::ui::components::Button)>,
    viewport_resolution_linked: Query<(
        Entity,
        &crate::ui::components::Camera,
        &Link,
        &crate::ui::components::Button,
    )>,
    info_panel_button_mini: Query<
        (Entity, &Info, &crate::ui::components::Button),
        (Without<Expanded>, Without<crate::ui::components::Camera>),
    >,
    info_panel_button_expanded: Query<
        (Entity, &Info, &crate::ui::components::Button),
        With<Expanded>,
    >,
    fov_slider: Query<(
        Entity,
        &UiAction,
        &crate::ui::components::Camera,
        &Depth,
        &Slider,
        &Container,
    )>,
    fov_slider_bar: Query<
        (Entity, &crate::ui::components::Camera, &Depth, &Slider),
        Without<Container>,
    >,
    camera_locked_button: Query<(Entity, &crate::ui::components::Camera, &Info)>,
) {
    // Camera locked button
    let (camera_locked_button, _, _) = camera_locked_button
        .single()
        .expect("Expected Camera Locked Button to be available in UI");

    commands.entity(camera_locked_button).observe(
        move |_: Trigger<Pointer<Click>>,
              mut ui_state: ResMut<UiState>,
              asset_server: Res<AssetServer>,
              mut camera_locked_icon: Query<(
            &mut Sprite,
            &crate::ui::components::Camera,
            &Info,
        )>| {
            let (mut camera_locked_icon, _, _) = camera_locked_icon
                .single_mut()
                .expect("Expected Camera Locked icon to be available in UI!");
            ui_state.camera_locked = !ui_state.camera_locked;
            if ui_state.camera_locked {
                *camera_locked_icon =
                    Sprite::from_image(asset_server.load("ui/lock_closed_icon.png"));
            } else {
                *camera_locked_icon =
                    Sprite::from_image(asset_server.load("ui/lock_open_icon.png"));
            }
        },
    );

    // Viewport resolution linked
    let (viewport_resolution_link_button, _, _, _) = viewport_resolution_linked
        .single()
        .expect("Expected Output Resolution Linked Button to be available in UI");
    commands.entity(viewport_resolution_link_button).observe(
        move |_: Trigger<Pointer<Click>>,
              mut ui_state: ResMut<UiState>,
              asset_server: Res<AssetServer>,
              mut viewport_resolution_linked: Query<(
            &mut Sprite,
            &crate::ui::components::Camera,
            &Link,
            &crate::ui::components::Button,
        )>| {
            let (mut link_sprite, _, _, _) = viewport_resolution_linked
                .single_mut()
                .expect("Expected Output Resolution Linked Button to be available in UI");

            ui_state.viewport_resolution_linked = !ui_state.viewport_resolution_linked;
            *link_sprite = if ui_state.viewport_resolution_linked {
                Sprite::from_image(asset_server.load("ui/linked_icon.png"))
            } else {
                Sprite::from_image(asset_server.load("ui/not_linked_icon.png"))
            };
        },
    );

    // Output resolution linked
    let (output_resolution_link_button, _, _, _) = output_resolution_linked
        .single()
        .expect("Expected Output Resolution Linked Button to be available in UI");
    commands.entity(output_resolution_link_button).observe(
        move |_: Trigger<Pointer<Click>>,
              mut ui_state: ResMut<UiState>,
              asset_server: Res<AssetServer>,
              mut output_resolution_linked: Query<(
            &mut Sprite,
            &crate::ui::components::Output,
            &Link,
            &crate::ui::components::Button,
        )>| {
            let (mut link_sprite, _, _, _) = output_resolution_linked
                .single_mut()
                .expect("Expected Output Resolution Linked Button to be available in UI");

            ui_state.output_resolution_linked = !ui_state.output_resolution_linked;
            *link_sprite = if ui_state.output_resolution_linked {
                Sprite::from_image(asset_server.load("ui/linked_icon.png"))
            } else {
                Sprite::from_image(asset_server.load("ui/not_linked_icon.png"))
            };
        },
    );

    // Shortcuts panel
    let (info_panel_button_mini, _, _) = info_panel_button_mini
        .single()
        .expect("Expected Open Shortcuts Panel Button to be available in UI");
    commands.entity(info_panel_button_mini).observe(
        move |_: Trigger<Pointer<Click>>,
              mut commands: Commands,
              mut pkv: ResMut<PkvStore>,
              mut ui_state: ResMut<UiState>,
              info_panel_mini: Query<(Entity, &Info, &Container), Without<Expanded>>,
              info_panel_expaned: Query<(Entity, &Info, &Container), With<Expanded>>| {
            let (maxi_info_panel, _, _) = info_panel_expaned
                .single()
                .expect("Expected Expanded Shortcuts Panel to be available in UI");
            let (mini_info_panel, _, _) = info_panel_mini
                .single()
                .expect("Expected Mini Shortcuts Button to be available in UI");
            commands.entity(maxi_info_panel).insert(Visibility::Visible);
            commands.entity(mini_info_panel).insert(Visibility::Hidden);
            ui_state.hide_shortcuts = false;
            pkv.set("shortcuts_hidden", &ui_state.hide_shortcuts.to_string())
                .expect("Expected to be able to store setting shortcuts_hidden");
        },
    );
    let (info_panel_button_expanded, _, _) = info_panel_button_expanded
        .single()
        .expect("Expected Close Shortcuts Panel Button to be available in UI");
    commands.entity(info_panel_button_expanded).observe(
        move |_: Trigger<Pointer<Click>>,
              mut commands: Commands,
              mut pkv: ResMut<PkvStore>,
              mut ui_state: ResMut<UiState>,
              info_panel_mini: Query<(Entity, &Info, &Container), Without<Expanded>>,
              info_panel_expaned: Query<(Entity, &Info, &Container), With<Expanded>>| {
            let (maxi_info_panel, _, _) = info_panel_expaned
                .single()
                .expect("Expected Expanded Shortcuts Panel to be available in UI");
            let (mini_info_panel, _, _) = info_panel_mini
                .single()
                .expect("Expected Mini Shortcuts Button to be available in UI");
            commands.entity(maxi_info_panel).insert(Visibility::Hidden);
            commands.entity(mini_info_panel).insert(Visibility::Visible);
            ui_state.hide_shortcuts = true;
            pkv.set("shortcuts_hidden", &ui_state.hide_shortcuts.to_string())
                .expect("Expected to be able to store setting shortcuts_hidden");
        },
    );

    // FOV slider
    let fov_slider_observer = |mut mouse_move: Trigger<Pointer<Move>>,
                               mut ui_state: ResMut<UiState>,
                               fov_slider: Query<(
        &UiAction,
        &Dimension,
        &GlobalTransform,
        &crate::ui::components::Camera,
        &Depth,
        &Slider,
        &Container,
    )>,
                               mut fov_slider_bar: Query<
        (
            &mut Dimension,
            &mut Transform,
            &crate::ui::components::Camera,
            &Depth,
            &Slider,
        ),
        Without<Container>,
    >| {
        let (ui_action, container_size, container_transform, _, _, _, _) = fov_slider
            .single()
            .expect("Expected FOV Slider to be available in UI");
        if ui_action.is_active {
            let (mut bar_size, mut bar_transform, _, _, _) = fov_slider_bar
                .single_mut()
                .expect("Expected FOV Slider bar to be available in UI");
            bar_size.x = mouse_move.hit.position.unwrap().x - container_transform.translation().x
                + container_size.x / 2.;
            bar_transform.translation.x = (bar_size.x - container_size.x) / 2.;
            let fov_bar_value_extent = (ui_action.boundaries[1] - ui_action.boundaries[0]) as f32;
            let fov_bar_percentage = (bar_size.x / container_size.x) as f32;
            ui_state.fov_value =
                (ui_action.boundaries[0] as f32 + fov_bar_value_extent * fov_bar_percentage) as u32;
        }
        mouse_move.propagate(false);
    };

    let (fov_slider, _, _, _, _, _) = fov_slider
        .single()
        .expect("Expected FOV Slider to be available in UI");

    let (fov_slider_bar, _, _, _) = fov_slider_bar
        .single()
        .expect("Expected FOV Slider bar to be available in UI");

    commands.entity(fov_slider).observe(fov_slider_observer);
    commands.entity(fov_slider_bar).observe(fov_slider_observer);
}

fn update_number(number: u32, update_motion: &Vec2, ui_action: &UiAction) -> u32 {
    let update_count = update_motion.x - update_motion.y;
    let mut new_number =
        (number as f32 * (1. + ui_action.change_sensitivity * (update_count as f32 / 4.))) as u32;
    if new_number == number && 0. != update_count {
        new_number += update_count.signum() as u32;
    }
    new_number.clamp(ui_action.boundaries[0], ui_action.boundaries[1])
}

pub(crate) fn update(
    mut commands: Commands,
    mut ui_state: ResMut<UiState>,
    keys: Res<ButtonInput<KeyCode>>,
    mut motion: EventReader<MouseMotion>,
    mut output_resolution_width_update_button: Query<(
        &UiAction,
        &mut Text2d,
        &Output,
        &Width,
        &crate::ui::components::Button,
    )>,
    mut output_resolution_height_update_button: Query<
        (
            &UiAction,
            &mut Text2d,
            &Output,
            &Height,
            &crate::ui::components::Button,
        ),
        Without<Width>,
    >,
    mut viewport_resolution_width_update_button: Query<
        (
            &UiAction,
            &mut Text2d,
            &crate::ui::components::Camera,
            &Width,
            &crate::ui::components::Button,
        ),
        Without<Output>,
    >,
    mut viewport_resolution_height_update_button: Query<
        (
            &UiAction,
            &mut Text2d,
            &crate::ui::components::Camera,
            &Height,
            &crate::ui::components::Button,
        ),
        (Without<Width>, Without<Output>),
    >,
    mut view_distance_button: Query<
        (
            &UiAction,
            &mut Text2d,
            &crate::ui::components::Camera,
            &Depth,
            &crate::ui::components::Button,
        ),
        (Without<Width>, Without<Height>, Without<Output>),
    >,
    loading_panel: Query<(&Dimension, &Model, &Loading, &Slider), With<Container>>,
    mut loading_panel_bar: Query<
        (&mut Dimension, &mut Transform, &Model, &Loading, &Slider),
        Without<Container>,
    >,
) {
    // Get mouse update motion:
    let mouse_update_motion = motion.read().map(|ev| ev.delta).sum();

    // View distance button
    let (ui_action, mut text, _, _, _) = view_distance_button
        .single_mut()
        .expect("Expected View distance Button to be available in UI");
    if ui_action.is_active {
        let distance = text
            .0
            .parse::<u32>()
            .expect("Expected viewport width text to be parsable as number");
        let new_distance = update_number(distance, &mouse_update_motion, ui_action);
        text.0 = new_distance.to_string();
        ui_state.view_distance = new_distance;
    }

    // Viewport resolution Width update button
    let (ui_action, mut text, _, _, _) = viewport_resolution_width_update_button
        .single_mut()
        .expect("Expected Viewport Resolution Width Button to be available in UI");
    if ui_action.is_active {
        let width = text
            .0
            .parse::<u32>()
            .expect("Expected viewport width text to be parsable as number");
        let new_width = update_number(width, &mouse_update_motion, ui_action);
        text.0 = new_width.to_string();
        ui_state.output_resolution[0] = new_width;
        if ui_state.viewport_resolution_linked && width != new_width {
            commands.trigger(OutputResolutionUpdated {
                by: ResolutionUpdated::ViewportWidth,
                from: width,
                to: new_width,
            });
        }
    }

    // Viewport resolution Height update button
    let (ui_action, mut text, _, _, _) = viewport_resolution_height_update_button
        .single_mut()
        .expect("Expected Viewport Resolution Height Button to be available in UI");
    if ui_action.is_active {
        let height = text
            .0
            .parse::<u32>()
            .expect("Expected viewport height text to be parsable as number");
        let new_height = update_number(height, &mouse_update_motion, ui_action);
        text.0 = new_height.to_string();
        ui_state.output_resolution[1] = new_height;
        if ui_state.viewport_resolution_linked && height != new_height {
            commands.trigger(OutputResolutionUpdated {
                by: ResolutionUpdated::ViewportHeight,
                from: height,
                to: new_height,
            });
        }
    }

    // Output resolution Width update button
    let (ui_action, mut text, _, _, _) = output_resolution_width_update_button
        .single_mut()
        .expect("Expected Output Resolution Width Button to be available in UI");
    if ui_action.is_active {
        let width = text
            .0
            .parse::<u32>()
            .expect("Expected text to be parsable as number");
        let new_width = update_number(width, &mouse_update_motion, ui_action);
        ui_state.viewport_resolution[0] = new_width;

        text.0 = new_width.to_string();
        if ui_state.output_resolution_linked && width != new_width {
            commands.trigger(OutputResolutionUpdated {
                by: ResolutionUpdated::OutputWidth,
                from: width,
                to: new_width,
            });
        }
    }

    // Output resolution Height update button
    let (ui_action, mut text, _, _, _) = output_resolution_height_update_button
        .single_mut()
        .expect("Expected Output Resolution Height Button to be available in UI");
    if ui_action.is_active {
        let height = text
            .0
            .parse::<u32>()
            .expect("Expected text to be parsable as number");
        let new_height = update_number(height, &mouse_update_motion, ui_action);
        text.0 = new_height.to_string();
        ui_state.viewport_resolution[1] = new_height;

        if ui_state.output_resolution_linked && height != new_height {
            commands.trigger(OutputResolutionUpdated {
                by: ResolutionUpdated::OutputHeight,
                from: height,
                to: new_height,
            });
        }
    }

    // Progress bar
    let (loading_panel_size, _, _, _) = loading_panel
        .single()
        .expect("Expected Model Progress panel to be available in UI");
    let (mut progressbar_size, mut progressbar_transform, _, _, _) = loading_panel_bar
        .single_mut()
        .expect("Expected Model Progressbar to be available in UI");

    let progressbar_xtrinsics_fn = |progress: f32, container_size: &Dimension| -> (f32, f32) {
        let size_x = container_size.x * progress;
        let transform_x = (size_x - container_size.x) / 2.;
        (size_x, transform_x)
    };
    if keys.just_pressed(KeyCode::Digit0) {
        (progressbar_size.x, progressbar_transform.translation.x) =
            progressbar_xtrinsics_fn(0., loading_panel_size);
    }
}

pub(crate) fn messages(mut status_text: Query<(&mut Text2d, &Model, &Status)>) {
    #[cfg(debug_assertions)]
    {
        let (mut message_text, _, _) = status_text
            .single_mut()
            .expect("Expected Status message to be available in UI");
        message_text.0 = "WARNING! Running in Debug mode! Perfromance will be bad!".to_string();
    }
}
