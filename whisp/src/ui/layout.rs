use crate::PkvStore;
use bevy::prelude::*;
use bevy_lunex::prelude::*;

fn icon() -> UiLayout {
    UiLayout::window().pos(Ab(4.)).width(24.).height(24.).pack()
}

pub(crate) fn fov_to_x(fov: u32, size_x: f32, boundaries: [u32; 2]) -> f32 {
    debug_assert!(
        boundaries[0] <= boundaries[1],
        "Expected valid fov boundaries instead of {:?}",
        boundaries
    );
    (size_x * fov as f32) / (boundaries[1] - boundaries[0]) as f32
}

pub(crate) fn setup(mut commands: Commands, asset_server: Res<AssetServer>, pkv: ResMut<PkvStore>) {
    let hide_ui = if let Ok(link) = pkv.get::<String>("ui_hidden") {
        link.parse::<bool>()
            .expect("Expected ui_hidden setting to be either 'true' or 'false'")
    } else {
        true
    };
    let hide_shortcuts = if let Ok(link) = pkv.get::<String>("shortcuts_hidden") {
        link.parse::<bool>()
            .expect("Expected stored shortcuts_hidden to be either 'true' or 'false'")
    } else {
        true
    };

    commands
        .spawn((UiLayoutRoot::new_2d(), UiFetchFromCamera::<0>))
        .with_children(|ui_root| {
            ui_root
                .spawn((
                    if hide_ui {
                        Visibility::Hidden
                    } else {
                        Visibility::Visible
                    },
                    crate::ui::components::UserInterface,
                    UiLayout::solid()
                        .scaling(Scaling::Fill)
                        .align_x(-1.)
                        .align_y(-1.)
                        .pack(),
                ))
                .with_children(|ui_hidable| {
                    // Model and properties
                    ui_hidable
                        .spawn((UiLayout::solid().align_x(-1.).align_y(-1.)).pack())
                        .with_children(|ui_model_panel| {
                            // model name panel
                            ui_model_panel
                                .spawn((
                                    crate::ui::components::Model,
                                    crate::ui::components::Container,
                                    UiLayout::window()
                                        .anchor(Anchor::TopLeft)
                                        .pos(Ab((5., 5.)))
                                        .size(Ab((768.0, 32.0)))
                                        .pack(),
                                    UiColor::from(Color::srgb(0.2, 0.1, 0.25)),
                                    Sprite::from_color(
                                        Color::srgba(1., 1., 1., 0.7),
                                        Vec2::new(1., 1.),
                                    ),
                                    OnHoverSetCursor::new(SystemCursorIcon::Pointer),
                                ))
                                .with_children(|ui_modelname_field| {
                                    ui_modelname_field.spawn((
                                        icon(),
                                        Sprite::from_image(asset_server.load("ui/open_icon.png")),
                                    ));
                                    ui_modelname_field.spawn((
                                        crate::ui::components::Model,
                                        UiLayout::window()
                                            .anchor(Anchor::TopLeft)
                                            .pos(Ab((32., 4.)))
                                            .width(Rl(100.) - Ab(37.))
                                            .height(Ab(25.))
                                            .pack(),
                                        UiColor::from(Color::srgb(0.88, 0.62, 0.49)),
                                        Text2d::new(
                                            pkv.get::<String>("model_name").ok().unwrap_or_else(
                                                || {
                                                    "gingerbread_house_by_kirra_luan(1024^3)"
                                                        .to_string()
                                                },
                                            ),
                                        ),
                                        OnHoverSetCursor::new(SystemCursorIcon::Pointer),
                                    ));
                                });

                            // resolution panel
                            ui_model_panel
                                .spawn((
                                    UiLayout::window()
                                        .anchor(Anchor::TopLeft)
                                        .pos(Ab((5., 40.)))
                                        .size(Ab((200.0, 32.0)))
                                        .pack(),
                                    Sprite::from_color(
                                        Color::srgba(1., 1., 1., 0.7),
                                        Vec2::new(1., 1.),
                                    ),
                                    UiColor::from(Color::srgb(0.2, 0.1, 0.25)),
                                ))
                                .with_children(|ui_resolution_panel| {
                                    ui_resolution_panel.spawn((
                                        crate::ui::components::Output,
                                        crate::ui::components::Link,
                                        crate::ui::components::Button,
                                        icon(),
                                        UiColor::from(Color::srgb(1., 1., 1.)),
                                        if let Ok(link) =
                                            pkv.get::<String>("output_resolution_linked")
                                        {
                                            if link.parse::<bool>()
                                                .expect("Expected output_resolution_linked setting to be either 'true' or 'false'")
                                            {
                                                Sprite::from_image(asset_server.load("ui/linked_icon.png"))
                                            } else {
                                                Sprite::from_image(asset_server.load("ui/not_linked_icon.png"))
                                            }
                                        } else {
                                            Sprite::from_image(
                                                asset_server.load("ui/open_icon.png"),
                                            )
                                        },
                                    ));

                                    ui_resolution_panel.spawn((
                                        crate::ui::components::Output,
                                        crate::ui::components::Width,
                                        crate::ui::components::Button,
                                        crate::ui::components::UiAction {
                                            is_active: false,
                                            change_sensitivity: 0.005,
                                            boundaries: [128, 7680],
                                        },
                                        UiLayout::window()
                                            .anchor(Anchor::TopLeft)
                                            .pos(Ab((35., 8.)))
                                            .size(Ab((55.0, 18.0)))
                                            .pack(),
                                        Sprite::from_color(
                                            Color::srgba(0.62, 0.1, 0.4, 0.7),
                                            Vec2::new(1., 1.),
                                        ),
                                        Text2d::new(
                                            pkv.get::<String>("output_resolution_width")
                                                .ok()
                                                .unwrap_or_else(|| "1920".to_string()),
                                        ),
                                        OnHoverSetCursor::new(SystemCursorIcon::Pointer),
                                    ));
                                    ui_resolution_panel.spawn((
                                        UiLayout::window()
                                            .anchor(Anchor::TopLeft)
                                            .pos(Ab((100., 7.)))
                                            .size(Ab((15.0, 18.0)))
                                            .pack(),
                                        UiColor::from(Color::srgb(0.88, 0.62, 0.49)),
                                        Text2d::new("x"),
                                    ));

                                    ui_resolution_panel.spawn((
                                        crate::ui::components::Output,
                                        crate::ui::components::Height,
                                        crate::ui::components::Button,
                                        crate::ui::components::UiAction {
                                            is_active: false,
                                            change_sensitivity: 0.005,
                                            boundaries: [72, 4320],
                                        },
                                        UiLayout::window()
                                            .anchor(Anchor::TopLeft)
                                            .pos(Ab((125., 8.)))
                                            .size(Ab((55.0, 20.0)))
                                            .pack(),
                                        Sprite::from_color(
                                            Color::srgba(0.62, 0.1, 0.4, 0.7),
                                            Vec2::new(1., 1.),
                                        ),
                                        Text2d::new(
                                            pkv.get::<String>("output_resolution_height")
                                                .ok()
                                                .unwrap_or_else(|| "1080".to_string()),
                                        ),
                                        OnHoverSetCursor::new(SystemCursorIcon::Pointer),
                                    ));
                                });

                            // Performance panel
                            ui_model_panel
                                .spawn((
                                    UiLayout::window()
                                        .anchor(Anchor::TopLeft)
                                        .pos(Ab((210., 40.)))
                                        .size(Ab((200.0, 32.0)))
                                        .pack(),
                                    Sprite::from_color(
                                        Color::srgba(1., 1., 1., 0.7),
                                        Vec2::new(1., 1.),
                                    ),
                                    UiColor::from(Color::srgb(0.2, 0.1, 0.25)),
                                ))
                                .with_children(|ui_performance_panel| {
                                    ui_performance_panel.spawn((
                                        crate::ui::components::Performance,
                                        UiLayout::window()
                                            .anchor(Anchor::TopLeft)
                                            .pos(Ab((25., 5.)))
                                            .size(Ab((150.0, 30.0)))
                                            .pack(),
                                        UiColor::from(Color::srgb(0.88, 0.62, 0.49)),
                                        Text2d::new("120fps / 5ms"),
                                    ));
                                });

                            // versions panel
                            ui_model_panel
                                .spawn((
                                    UiLayout::window()
                                        .anchor(Anchor::TopLeft)
                                        .pos(Ab((415., 40.)))
                                        .size(Ab((200.0, 32.0)))
                                        .pack(),
                                    Sprite::from_color(
                                        Color::srgba(1., 1., 1., 0.7),
                                        Vec2::new(1., 1.),
                                    ),
                                    UiColor::from(Color::srgb(0.2, 0.1, 0.25)),
                                ))
                                .with_children(|ui_versions_panel| {
                                    ui_versions_panel.spawn((
                                        crate::ui::components::Model,
                                        crate::ui::components::Info,
                                        UiLayout::window()
                                            .anchor(Anchor::TopLeft)
                                            .pos(Ab((15., 5.)))
                                            .size(Ab((165.0, 20.0)))
                                            .pack(),
                                        UiColor::from(Color::srgb(0.88, 0.62, 0.49)),
                                        Text2d::new("model: v0.1  / app: v0.1"),
                                    ));
                                });

                            // G to hide text
                            ui_model_panel.spawn((
                                UiLayout::window()
                                    .anchor(Anchor::TopLeft)
                                    .pos(Ab((620., 45.)))
                                    .size(Ab((150.0, 25.0)))
                                    .pack(),
                                UiColor::from(Color::srgb(0.88, 0.62, 0.49)),
                                Text2d::new("Press 'G' to hide UI"),
                            ));

                            // Loading bar
                            ui_model_panel
                                .spawn((
                                    crate::ui::components::Model,
                                    crate::ui::components::Loading,
                                    crate::ui::components::Slider,
                                    crate::ui::components::Container,
                                    UiLayout::window()
                                        .anchor(Anchor::TopLeft)
                                        .pos(Ab((5., 75.)))
                                        .size(Ab((768.0, 25.0)))
                                        .pack(),
                                    Sprite::from_color(
                                        Color::srgba(1., 1., 1., 0.7),
                                        Vec2::new(1., 1.),
                                    ),
                                    UiColor::from(Color::srgb(0.3, 0.0, 0.23)),
                                ))
                                .with_children(|ui_loading_bar| {
                                    ui_loading_bar.spawn((
                                        crate::ui::components::Model,
                                        crate::ui::components::Loading,
                                        crate::ui::components::Slider,
                                        UiLayout::window()
                                            .anchor(Anchor::TopLeft)
                                            .pos(Ab((0., 0.)))
                                            .size(Ab((0.0, 25.0)))
                                            .pack(),
                                        Sprite::from_color(
                                            Color::srgba(1., 1., 1., 0.7),
                                            Vec2::new(1., 1.),
                                        ),
                                        UiColor::from(Color::srgb(0.96, 0.0, 0.72)),
                                    ));

                                    ui_loading_bar
                                        .spawn(
                                            UiLayout::window()
                                                .pos(Ab(2.))
                                                .size(Rl(100.) - Ab(2.))
                                                .anchor(Anchor::TopLeft)
                                                .pack(),
                                        )
                                        .with_children(|ui_loading_bar_padding| {
                                            ui_loading_bar_padding.spawn((
                                                crate::ui::components::Model,
                                                crate::ui::components::Loading,
                                                UiLayout::solid()
                                                    .scaling(Scaling::VerFill)
                                                    .align_x(-1.)
                                                    .size((400.0, 20.0))
                                                    .pack(),
                                                UiColor::from(Color::srgb(0., 0., 0.)),
                                                UiTextSize::from(Rh(95.0)),
                                                Text2d::new(""),
                                            ));

                                            ui_loading_bar_padding.spawn((
                                                crate::ui::components::Model,
                                                crate::ui::components::Status,
                                                UiLayout::solid()
                                                    .scaling(Scaling::VerFill)
                                                    .align_x(1.)
                                                    .size((400.0, 20.0))
                                                    .pack(),
                                                UiColor::from(Color::srgb(0.88, 0.62, 0.49)),
                                                UiTextSize::from(Rh(95.0)),
                                                Text2d::new(""),
                                            ));
                                        });
                                });
                        });

                    // Camera Intrinsics panel
                    ui_hidable
                        .spawn((
                            crate::ui::components::Camera,
                            crate::ui::components::Container,
                            UiLayout::window()
                                .anchor(Anchor::TopRight)
                                .x(Rl(100.) - Ab(5.))
                                .y(Ab(5.))
                                .size(Ab((400.0, 100.0)))
                                .pack(),
                            Sprite::from_color(Color::srgba(1., 1., 1., 0.7), Vec2::new(1., 1.)),
                            UiColor::from(Color::srgb(0.2, 0.1, 0.25)),
                        ))
                        .with_children(|ui_camera_intrinsics_panel| {
                            ui_camera_intrinsics_panel.spawn((
                                UiLayout::window()
                                    .anchor(Anchor::TopLeft)
                                    .pos(Ab((5., 5.)))
                                    .size(Ab((100., 25.)))
                                    .pack(),
                                UiColor::from(Color::srgb(0.88, 0.62, 0.49)),
                                UiTextSize::from(Ab(15.0)),
                                Text2d::new("FOV:"),
                            ));

                            let fov_value_bounds = [1, 100];
                            ui_camera_intrinsics_panel
                                .spawn((
                                    crate::ui::components::Camera,
                                    crate::ui::components::Depth,
                                    crate::ui::components::Slider,
                                    crate::ui::components::Container,
                                    crate::ui::components::UiAction {
                                        is_active: false,
                                        change_sensitivity: 0.01,
                                        boundaries: fov_value_bounds,
                                    },
                                    UiLayout::window()
                                        .anchor(Anchor::TopLeft)
                                        .pos(Ab((105., 5.)))
                                        .width(Rl(100.) - Ab(105.))
                                        .height(Ab(25.))
                                        .pack(),
                                    Sprite::from_color(
                                        Color::srgba(1., 1., 1., 0.7),
                                        Vec2::new(1., 1.),
                                    ),
                                    UiColor::from(Color::srgb(0.3, 0.0, 0.23)),
                                    OnHoverSetCursor::new(SystemCursorIcon::Pointer),
                                ))
                                .with_children(|ui_fov_panel| {
                                    ui_fov_panel.spawn((
                                        crate::ui::components::Camera,
                                        crate::ui::components::Depth,
                                        crate::ui::components::Slider,
                                        UiLayout::window()
                                            .anchor(Anchor::TopLeft)
                                            .pos(Ab((0., 0.)))
                                            .width(
                                                Ab(
                                                    fov_to_x(
                                                        pkv.get::<String>("fov").ok()
                                                            .unwrap_or_else(|| "50".to_string()).parse::<u32>()
                                                            .expect("Expected to be able to parse fov setting"),
                                                        295.0,
                                                        fov_value_bounds
                                                    )
                                                )
                                            )
                                            .height(Rl(100.))
                                            .pack(),
                                        Sprite::from_color(
                                            Color::srgba(1., 1., 1., 0.7),
                                            Vec2::new(1., 1.),
                                        ),
                                        UiColor::from(Color::srgb(0.96, 0.0, 0.72)),
                                        OnHoverSetCursor::new(SystemCursorIcon::Pointer),
                                    ));
                                });

                            ui_camera_intrinsics_panel.spawn((
                                UiLayout::window()
                                    .anchor(Anchor::TopLeft)
                                    .pos(Ab((5., 35.)))
                                    .size(Ab((100., 25.)))
                                    .pack(),
                                UiColor::from(Color::srgb(0.88, 0.62, 0.49)),
                                UiTextSize::from(Ab(15.0)),
                                Text2d::new("Viewport:"),
                            ));

                            ui_camera_intrinsics_panel
                                .spawn(
                                    UiLayout::window()
                                        .anchor(Anchor::TopLeft)
                                        .pos(Ab((105., 35.)))
                                        .width(Rl(100.) - Ab(100.))
                                        .height(Ab(25.))
                                        .pack(),
                                )
                                .with_children(|ui_camera_intrinsics_viewport_row| {
                                    ui_camera_intrinsics_viewport_row.spawn((
                                        crate::ui::components::Camera,
                                        crate::ui::components::Link,
                                        crate::ui::components::Button,
                                        icon(),
                                        UiColor::from(Color::srgb(1., 1., 1.)),
                                        if let Ok(link) =
                                            pkv.get::<String>("viewport_resolution_linked")
                                        {
                                            if link.parse::<bool>()
                                                .expect("Expected viewport_resolution_linked setting to be either 'true' or 'false'")
                                            {
                                                Sprite::from_image(asset_server.load("ui/linked_icon.png"))
                                            } else {
                                                Sprite::from_image(asset_server.load("ui/not_linked_icon.png"))
                                            }
                                        } else {
                                            Sprite::from_image(asset_server.load("ui/open_icon.png"))
                                        },
                                    ));

                                    ui_camera_intrinsics_viewport_row.spawn((
                                        crate::ui::components::Camera,
                                        crate::ui::components::Width,
                                        crate::ui::components::Button,
                                        crate::ui::components::UiAction {
                                            is_active: false,
                                            change_sensitivity: 0.005,
                                            boundaries: [5, 250],
                                        },
                                        UiLayout::window()
                                            .anchor(Anchor::TopLeft)
                                            .pos(Ab((40., 3.)))
                                            .size(Ab((45.0, 18.0)))
                                            .pack(),
                                        Sprite::from_color(
                                            Color::srgba(0.62, 0.1, 0.4, 0.7),
                                            Vec2::new(1., 1.),
                                        ),
                                        Text2d::new(
                                            pkv.get::<String>("viewport_resolution_width")
                                                .ok()
                                                .unwrap_or_else(|| "100".to_string()),
                                        ),
                                        OnHoverSetCursor::new(SystemCursorIcon::Pointer),
                                    ));

                                    ui_camera_intrinsics_viewport_row.spawn((
                                        UiLayout::window()
                                            .anchor(Anchor::TopLeft)
                                            .pos(Ab((100., 0.)))
                                            .size(Ab((15.0, 25.0)))
                                            .pack(),
                                        UiColor::from(Color::srgb(0.88, 0.62, 0.49)),
                                        Text2d::new("x"),
                                    ));

                                    ui_camera_intrinsics_viewport_row.spawn((
                                        crate::ui::components::Camera,
                                        crate::ui::components::Height,
                                        crate::ui::components::Button,
                                        crate::ui::components::UiAction {
                                            is_active: false,
                                            change_sensitivity: 0.005,
                                            boundaries: [5, 250],
                                        },
                                        UiLayout::window()
                                            .anchor(Anchor::TopLeft)
                                            .pos(Ab((130., 3.)))
                                            .size(Ab((45.0, 18.0)))
                                            .pack(),
                                        Sprite::from_color(
                                            Color::srgba(0.62, 0.1, 0.4, 0.7),
                                            Vec2::new(1., 1.),
                                        ),
                                        Text2d::new(
                                            pkv.get::<String>("viewport_resolution_height")
                                                .ok()
                                                .unwrap_or_else(|| "100".to_string()),
                                        ),
                                        OnHoverSetCursor::new(SystemCursorIcon::Pointer),
                                    ));
                                });

                            ui_camera_intrinsics_panel.spawn((
                                UiLayout::window()
                                    .anchor(Anchor::TopLeft)
                                    .pos(Ab((5., 65.)))
                                    .size(Ab((100., 25.)))
                                    .pack(),
                                UiColor::from(Color::srgb(0.88, 0.62, 0.49)),
                                UiTextSize::from(Ab(15.0)),
                                Text2d::new("View dist.:"),
                            ));

                            ui_camera_intrinsics_panel.spawn((
                                crate::ui::components::Camera,
                                crate::ui::components::Depth,
                                crate::ui::components::Button,
                                crate::ui::components::UiAction {
                                    is_active: false,
                                    change_sensitivity: 0.005,
                                    boundaries: [10, 500000],
                                },
                                UiLayout::window()
                                    .anchor(Anchor::TopLeft)
                                    .pos(Ab((105., 65.)))
                                    .size(Ab((55.0, 18.0)))
                                    .pack(),
                                Sprite::from_color(
                                    Color::srgba(0.62, 0.1, 0.4, 0.7),
                                    Vec2::new(1., 1.),
                                ),
                                Text2d::new(
                                    pkv.get::<String>("view_distance")
                                        .ok()
                                        .unwrap_or_else(|| "1024".to_string()),
                                ),
                                OnHoverSetCursor::new(SystemCursorIcon::Pointer),
                            ));
                        });

                    // Shortcuts button
                    ui_hidable
                        .spawn((
                            if hide_ui || !hide_shortcuts {
                                Visibility::Hidden
                            } else {
                                Visibility::Visible
                            },
                            crate::ui::components::Info,
                            crate::ui::components::Container,
                            UiLayout::window()
                                .anchor(Anchor::TopRight)
                                .x(Rl(100.) - Ab(5.))
                                .y(Ab(200.))
                                .size(Ab((32.0, 32.0)))
                                .pack(),
                            Sprite::from_color(Color::srgba(1., 1., 1., 0.7), Vec2::new(1., 1.)),
                            UiColor::from(Color::srgb(0.2, 0.1, 0.25)),
                            OnHoverSetCursor::new(SystemCursorIcon::Pointer),
                        ))
                        .with_children(|ui_shortcuts_panel| {
                            ui_shortcuts_panel.spawn((
                                crate::ui::components::Info,
                                crate::ui::components::Button,
                                icon(),
                                Sprite::from_image(asset_server.load("ui/info_active.png")),
                            ));
                        });

                    // Shortcuts panel
                    ui_hidable
                        .spawn((
                            if hide_ui || hide_shortcuts {
                                Visibility::Hidden
                            } else {
                                Visibility::Visible
                            },
                            crate::ui::components::Info,
                            crate::ui::components::Container,
                            crate::ui::components::Expanded,
                            UiLayout::window()
                                .anchor(Anchor::TopRight)
                                .x(Rl(100.) - Ab(5.))
                                .y(Ab(200.))
                                .size(Ab((250.0, 512.0)))
                                .pack(),
                            Sprite::from_color(Color::srgba(1., 1., 1., 0.7), Vec2::new(1., 1.)),
                            UiColor::from(Color::srgb(0.2, 0.1, 0.25)),
                        ))
                        .with_children(|ui_shortcuts_panel| {
                            ui_shortcuts_panel.spawn((
                                crate::ui::components::Info,
                                crate::ui::components::Button,
                                crate::ui::components::Expanded,
                                UiLayout::window()
                                    .anchor(Anchor::TopRight)
                                    .size(Ab((24., 24.)))
                                    .x(Rl(100.) - Ab(4.))
                                    .y(Ab(4.))
                                    .pack(),
                                Sprite::from_image(asset_server.load("ui/info_active.png")),
                                OnHoverSetCursor::new(SystemCursorIcon::Pointer),
                            ));

                            ui_shortcuts_panel.spawn((
                                UiLayout::solid().scaling(Scaling::Fit).align_x(1.).pack(),
                                UiColor::from(Color::srgb(0.88, 0.62, 0.49)),
                                Text2d::new(
                                    "
      G     - hide UI
      F4    - Lock Camera
      F5    - Save Settings
(ctrl)F5    - Restore defaults
      F6    - Save camera position
(ctrl)F6    - Restore default
      F9    - Load Settings
      F10   - Load Camera position
      WS    - Forward/Backward
      AD    - Move Sideways
      Shift - Move Y up
      Ctrl  - Move Y Down
---------------------------------
Drag mouse to look around!
---------------------------------
Drag Buttons to update values!
---------------------------------
Drag&Drop files to open them!
(Magicavoxel format)
---------------------------------
",
                                ),
                            ));
                        });

                        //Camera movements locked icon
                        ui_hidable.spawn((
                            if hide_ui {
                                Visibility::Hidden
                            } else {
                                Visibility::Visible
                            },
                            crate::ui::components::Camera,
                            crate::ui::components::Info,
                            crate::ui::components::Button,
                            UiLayout::window()
                                .anchor(Anchor::TopRight)
                                .x(Rl(100.) - Ab(5.))
                                .y(Ab(110.))
                                .size(Ab((32.0, 32.0)))
                                .pack(),
                                OnHoverSetCursor::new(SystemCursorIcon::Pointer),
                                if let Ok(link) =
                                    pkv.get::<String>("camera_locked")
                                {
                                    if link.parse::<bool>()
                                        .expect("Expected camera_locked setting to be either 'true' or 'false'")
                                    {
                                        Sprite::from_image(asset_server.load("ui/lock_closed_icon.png"))
                                    } else {
                                        Sprite::from_image(asset_server.load("ui/lock_open_icon.png"))
                                    }
                                } else {
                                    Sprite::from_image(
                                        asset_server.load("ui/lock_open_icon.png"),
                                    )
                                },
                        ));
                });

            // render texture
            let mut sprite = Sprite::from_color(Color::srgb(1., 0., 0.), Vec2::new(1920.0, 1080.0));
            sprite.custom_size = Some(Vec2::new(
                1920.0,
                1080.0,
            ));
            ui_root.spawn((
                crate::ui::components::Model,
                crate::ui::components::Output,
                crate::ui::components::Container,
                UiLayout::solid()
                    .size((2., 1.))
                    .align_x(0.)
                    .align_y(0.)
                    .pack(),
                sprite,
            ));
        });
}
