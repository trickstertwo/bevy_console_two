//! Terminal backend for headless/dedicated server console.
//!
//! This module provides stdin/stdout integration for running the console
//! without a graphical UI, useful for dedicated servers.

use std::io::{self, BufRead, Write};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Mutex;
use std::thread::{self, JoinHandle};

use bevy::prelude::*;

use crate::core::{ConsoleInputEvent, ConsoleOutputEvent, ConsoleOutputLevel};

/// Plugin that adds terminal (stdin/stdout) console support.
pub struct TerminalPlugin;

impl Plugin for TerminalPlugin {
    fn build(&self, app: &mut App) {
        let (sender, receiver) = mpsc::channel();
        let _handle = spawn_stdin_reader(sender);

        app.insert_resource(StdinReceiver(Mutex::new(receiver)))
            .insert_resource(TerminalConfig::default())
            .add_systems(Update, (read_stdin, write_stdout));
    }
}

/// Configuration for terminal behavior.
#[derive(Resource)]
pub struct TerminalConfig {
    /// Whether to use colored output (ANSI escape codes).
    pub colored: bool,
}

impl Default for TerminalConfig {
    fn default() -> Self {
        Self { colored: false }  // Disabled by default - causes issues on some terminals
    }
}

#[derive(Resource)]
struct StdinReceiver(Mutex<Receiver<String>>);

fn spawn_stdin_reader(sender: Sender<String>) -> JoinHandle<()> {
    thread::spawn(move || {
        let stdin = io::stdin();
        let handle = stdin.lock();

        for line in handle.lines().flatten() {
            let text = line.trim().to_string();
            if !text.is_empty() {
                if sender.send(text).is_err() {
                    break;
                }
            }
        }
    })
}

fn read_stdin(receiver: Res<StdinReceiver>, mut events: MessageWriter<ConsoleInputEvent>) {
    let rx = receiver.0.lock().unwrap();
    while let Ok(line) = rx.try_recv() {
        events.write(ConsoleInputEvent::new(line));
    }
}

fn write_stdout(mut events: MessageReader<ConsoleOutputEvent>, config: Res<TerminalConfig>) {
    for event in events.read() {
        if config.colored {
            print_colored(&event.message, event.level);
        } else {
            println!("{}", event.message);
        }
        let _ = io::stdout().flush();
    }
}

fn print_colored(message: &str, level: ConsoleOutputLevel) {
    let color = match level {
        ConsoleOutputLevel::Debug => "\x1b[90m",
        ConsoleOutputLevel::Info => "\x1b[0m",
        ConsoleOutputLevel::Warn => "\x1b[33m",
        ConsoleOutputLevel::Error => "\x1b[31m",
        ConsoleOutputLevel::Command => "\x1b[36m",
        ConsoleOutputLevel::Result => "\x1b[32m",
    };
    println!("{}{}\x1b[0m", color, message);
}
