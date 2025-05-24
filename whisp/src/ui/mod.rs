use bevy::prelude::*;
use bevy_pkv::PkvStore;

pub(crate) mod behavior;
pub(crate) mod components;
pub(crate) mod input;
pub(crate) mod layout;

#[derive(Resource)]
pub(crate) struct UiState {
    pub(crate) camera_locked: bool,
    pub(crate) menu_interaction: bool,
    pub(crate) model_loaded: bool,
    pub(crate) hide_ui: bool,
    pub(crate) hide_shortcuts: bool,
    pub(crate) output_resolution_linked: bool,
    pub(crate) viewport_resolution_linked: bool,
    pub(crate) fov_value: u32,
    pub(crate) view_distance: u32,
    pub(crate) output_resolution: [u32; 2],
    pub(crate) viewport_resolution: [u32; 2],
}

impl UiState {
    pub(crate) fn new(pkv: &PkvStore) -> Self {
        Self {
            model_loaded: false,
            menu_interaction: false,
            camera_locked: if let Ok(link) = pkv.get::<String>("camera_locked") {
                link.parse::<bool>()
                    .expect("Expected camera_locked setting to be either 'true' or 'false'")
            } else {
                true
            },
            hide_ui: if let Ok(link) = pkv.get::<String>("ui_hidden") {
                link.parse::<bool>()
                    .expect("Expected ui_hidden setting to be either 'true' or 'false'")
            } else {
                true
            },
            hide_shortcuts: if let Ok(link) = pkv.get::<String>("shortcuts_hidden") {
                link.parse::<bool>()
                    .expect("Expected stored shortcuts_hidden to be either 'true' or 'false'")
            } else {
                true
            },
            output_resolution_linked: if let Ok(link) =
                pkv.get::<String>("output_resolution_linked")
            {
                link.parse::<bool>().expect(
                    "Expected output_resolution_linked setting to be either 'true' or 'false'",
                )
            } else {
                true
            },
            viewport_resolution_linked: if let Ok(link) =
                pkv.get::<String>("viewport_resolution_linked")
            {
                link.parse::<bool>().expect(
                    "Expected viewport_resolution_linked setting to be either 'true' or 'false'",
                )
            } else {
                true
            },
            fov_value: if let Ok(fov) = pkv.get::<String>("fov") {
                fov.parse::<u32>()
                    .expect("Expected fov setting to be a parsable number")
            } else {
                50
            },
            view_distance: if let Ok(vdist) = pkv.get::<String>("view_distance") {
                vdist
                    .parse::<u32>()
                    .expect("Expected view_distance setting to be a parsable number")
            } else {
                1024
            },
            output_resolution: [
                if let Ok(res) = pkv.get::<String>("output_resolution_width") {
                    res.parse::<u32>()
                        .expect("Expected output_resolution_width setting to be a parsable number")
                } else {
                    1920
                },
                if let Ok(res) = pkv.get::<String>("output_resolution_height") {
                    res.parse::<u32>()
                        .expect("Expected output_resolution_height setting to be a parsable number")
                } else {
                    1080
                },
            ],
            viewport_resolution: [
                if let Ok(res) = pkv.get::<String>("viewport_resolution_width") {
                    res.parse::<u32>().expect(
                        "Expected viewport_resolution_width setting to be a parsable number",
                    )
                } else {
                    100
                },
                if let Ok(res) = pkv.get::<String>("viewport_resolution_height") {
                    res.parse::<u32>().expect(
                        "Expected viewport_resolution_height setting to be a parsable number",
                    )
                } else {
                    100
                },
            ],
        }
    }
}
