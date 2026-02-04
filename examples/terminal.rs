//! Terminal console example.
//!
//! Demonstrates using bevy_console_two with stdin/stdout for headless applications
//! like dedicated servers.
//!
//! Run with: `cargo run --example terminal --no-default-features --features terminal`
//!
//! Commands:
//! - `help` - List available commands
//! - `sv_gravity` - Query the gravity value
//! - `sv_gravity 1000` - Set gravity to 1000
//! - `status` - Show server status
//! - `quit` - Exit the application

use bevy::prelude::*;
use bevy_console_two::{Console, ConVar, ConVarFlags, ConCommand, ConsoleRegistry};

fn main() {
    println!("=== Terminal Console Example ===");
    println!("Type commands and press Enter. Type 'quit' to exit.");
    println!();

    // Ensure output is flushed before starting the app
    use std::io::Write;
    let _ = std::io::stdout().flush();

    App::new()
        .add_plugins(MinimalPlugins)
        .add_plugins(bevy_console_two::ConsolePlugin)
        .add_systems(Startup, setup)
        .run();
}

fn setup(mut console: Console) {
    // Register convars
    console.register_var(
        ConVar::new("sv_gravity", 800.0f32)
            .description("World gravity")
            .flags(ConVarFlags::ARCHIVE),
    );

    console.register_var(
        ConVar::new("sv_maxplayers", 32i32)
            .description("Maximum number of players")
            .min(1)
            .max(64),
    );

    console.register_var(
        ConVar::new("sv_hostname", "My Server".to_string())
            .description("Server name")
            .flags(ConVarFlags::ARCHIVE),
    );

    // Register commands
    console.register_cmd(
        ConCommand::new("status", |_args, world| {
            let registry = world.resource::<ConsoleRegistry>();

            let hostname: String = registry.get("sv_hostname").unwrap_or_default();
            let maxplayers: i32 = registry.get("sv_maxplayers").unwrap_or(0);
            let gravity: f32 = registry.get("sv_gravity").unwrap_or(0.0);

            println!("=== Status ===");
            println!("Hostname: {}", hostname);
            println!("Max Players: {}", maxplayers);
            println!("Gravity: {}", gravity);
        })
        .description("Show server status"),
    );

    console.register_cmd(
        ConCommand::new("say", |args, _world| {
            if args.is_empty() {
                println!("Usage: say <message>");
            } else {
                println!("[SERVER] {}", args.join(" "));
            }
        })
        .description("Broadcast a message"),
    );
}
