use bevy::app::{App, PluginGroup};
use bevy::diagnostic::FrameTimeDiagnosticsPlugin;
use bevy::utils::default;
use bevy::window::{PresentMode, Window, WindowPlugin};
use bevy::DefaultPlugins;

fn main() {
    let mut app = App::new();

    app.add_plugins((
        DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: concat!("GDClone ", env!("VERSION")).into(),
                present_mode: PresentMode::AutoNoVsync,
                ..default()
            }),
            ..default()
        }),
        FrameTimeDiagnosticsPlugin,
    ));

    app.run()
}
