use bevy::prelude::Component;

#[derive(Component)]
pub(crate) struct UiAction {
    pub(crate) is_active: bool,
    pub(crate) change_sensitivity: f32,
    pub(crate) boundaries: [u32; 2],
}

#[derive(Component)]
pub(crate) struct Width;

#[derive(Component)]
pub(crate) struct Height;

#[derive(Component)]
pub(crate) struct Depth;

#[derive(Component)]
pub(crate) struct Slider;

#[derive(Component)]
pub(crate) struct Button;

#[derive(Component)]
pub(crate) struct Container;

#[derive(Component)]
pub(crate) struct UserInterface;

#[derive(Component)]
pub(crate) struct Model;

#[derive(Component)]
pub(crate) struct Output;

#[derive(Component)]
pub(crate) struct Status;

#[derive(Component)]
pub(crate) struct Performance;

#[derive(Component)]
pub(crate) struct Loading;

#[derive(Component)]
pub(crate) struct Camera;

#[derive(Component)]
pub(crate) struct Info;

#[derive(Component)]
pub(crate) struct Expanded;

#[derive(Component)]
pub(crate) struct Link;
