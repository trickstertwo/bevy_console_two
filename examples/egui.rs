//! Egui console example.
//!
//! Demonstrates the egui-based developer console with convars and commands.
//!
//! Run with: `cargo run --example egui`
//!
//! Controls:
//! - Press ` (grave/tilde) to toggle console
//! - Press Enter to submit commands
//! - Press Tab or ArrowRight to accept autocomplete
//! - Press ArrowUp/ArrowDown to navigate history
//!
//! Try these commands:
//! - `help` - List all commands
//! - `cvarlist` - List all convars
//! - `sv_gravity 1200` - Change gravity
//! - `spawn` / `despawn` - Spawn/despawn entities
//! - `status` - Show current settings

use bevy::log::LogPlugin;
use bevy::prelude::*;
use bevy_console_two::prelude::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(LogPlugin {
            custom_layer: bevy_console_two::logging::custom_log_layer,
            ..default()
        }))
        .add_plugins(bevy_egui::EguiPlugin::default())
        .add_plugins(bevy_console_two::ConsolePlugin)
        .add_systems(Startup, setup)
        .run();
}

/// Marker for demo entities.
#[derive(Component)]
struct DemoEntity;

fn setup(mut commands: Commands, mut console: Console) {
    commands.spawn(Camera2d::default());

    // Server convars
    console.register_var(
        ConVar::new("sv_gravity", 800.0f32)
            .description("World gravity (units/s^2)")
            .flags(ConVarFlags::ARCHIVE)
            .min(0.0)
            .max(2000.0),
    );

    console.register_var(
        ConVar::new("sv_maxplayers", 32i32)
            .description("Maximum players")
            .flags(ConVarFlags::ARCHIVE | ConVarFlags::READ_ONLY)
            .min(1)
            .max(64),
    );

    // Client convars
    console.register_var(
        ConVar::new("cl_showfps", true)
            .description("Show FPS counter")
            .flags(ConVarFlags::ARCHIVE),
    );

    console.register_var(
        ConVar::new("player_speed", 200.0f32)
            .description("Player movement speed")
            .min(50.0)
            .max(1000.0),
    );

    // Commands
    console.register_cmd(
        ConCommand::new("spawn", |_args, world| {
            world.spawn((
                Sprite {
                    color: Color::srgb(0.3, 0.7, 0.3),
                    custom_size: Some(Vec2::new(50.0, 50.0)),
                    ..default()
                },
                DemoEntity,
            ));
            info!("Spawned entity");
        })
        .description("Spawn a demo entity"),
    );

    console.register_cmd(
        ConCommand::new("despawn", |_args, world| {
            let entities: Vec<Entity> = world
                .query_filtered::<Entity, With<DemoEntity>>()
                .iter(world)
                .collect();
            let count = entities.len();
            for entity in entities {
                world.despawn(entity);
            }
            info!("Despawned {} entities", count);
        })
        .description("Despawn all demo entities"),
    );

    console.register_cmd(
        ConCommand::new("status", |_args, world| {
            let registry = world.resource::<ConsoleRegistry>();
            info!("=== Status ===");
            info!("  sv_gravity: {}", registry.get::<f32>("sv_gravity").unwrap_or(0.0));
            info!("  sv_maxplayers: {}", registry.get::<i32>("sv_maxplayers").unwrap_or(0));
            info!("  player_speed: {}", registry.get::<f32>("player_speed").unwrap_or(0.0));
        })
        .description("Show current settings"),
    );

    console.register_cmd(
        ConCommand::new("noclip", |_args, world| {
            let registry = world.resource::<ConsoleRegistry>();
            if registry.get::<i32>("sv_cheats").unwrap_or(0) == 0 {
                warn!("sv_cheats must be enabled to use noclip");
                return;
            }
            info!("Noclip toggled");
        })
        .description("Toggle noclip (requires sv_cheats)")
        .flags(ConVarFlags::CHEAT),
    );

    info!("Press ` to open console. Try: help, spawn, status, cvarlist");
}
