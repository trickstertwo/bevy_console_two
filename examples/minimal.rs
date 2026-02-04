//! Minimal headless console example.
//!
//! Demonstrates using bevy_console_two programmatically without any UI.
//! Useful for testing or custom UI implementations.
//!
//! Run with: `cargo run --example minimal --no-default-features`

use bevy::prelude::*;
use bevy_console_two::prelude::*;

fn main() {
    App::new()
        .add_plugins(MinimalPlugins)
        .add_plugins(bevy_console_two::ConsolePlugin)
        .add_systems(Startup, setup)
        .add_systems(Update, process_outputs)
        .add_systems(Update, send_test_commands.run_if(run_once))
        .run();
}

fn setup(mut console: Console) {
    // Register a simple convar
    console.register_var(
        ConVar::new("sv_gravity", 800.0f32)
            .description("World gravity"),
    );

    // Register a command
    console.register_cmd(
        ConCommand::new("greet", |args, _world| {
            let name = args.get(0).unwrap_or("World");
            println!("Hello, {}!", name);
        })
        .description("Greet someone"),
    );

    println!("Console initialized. Registered: sv_gravity, greet");
}

/// Send some test commands programmatically.
fn send_test_commands(mut events: MessageWriter<ConsoleInputEvent>) {
    println!("\n--- Sending test commands ---");

    // Query a variable
    events.write(ConsoleInputEvent::new("sv_gravity"));

    // Set a variable
    events.write(ConsoleInputEvent::new("sv_gravity 1000"));

    // Run a command
    events.write(ConsoleInputEvent::new("greet Developer"));

    // Multiple commands with semicolons
    events.write(ConsoleInputEvent::new("echo First; echo Second; echo Third"));
}

/// Process and print console output events.
fn process_outputs(mut events: MessageReader<ConsoleOutputEvent>) {
    for event in events.read() {
        let prefix = match event.level {
            ConsoleOutputLevel::Debug => "[DEBUG]",
            ConsoleOutputLevel::Info => "[INFO]",
            ConsoleOutputLevel::Warn => "[WARN]",
            ConsoleOutputLevel::Error => "[ERROR]",
            ConsoleOutputLevel::Command => "[$]",
            ConsoleOutputLevel::Result => "[>]",
        };
        println!("{} {}", prefix, event.message);
    }
}
