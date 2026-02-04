//! Console events for communication between layers.
//!
//! Events are the primary mechanism for:
//! - UI -> Core: Command input
//! - Core -> UI: Output/logging
//! - Core -> Systems: ConVar changes

use bevy::prelude::*;

/// Event sent when a command is submitted to the console.
///
/// The console system will parse and execute this command.
///
/// # Examples
///
/// ```ignore
/// fn submit_command(mut events: EventWriter<ConsoleInputEvent>) {
///     events.send(ConsoleInputEvent {
///         command: "sv_cheats 1".to_string(),
///     });
/// }
/// ```
#[derive(Message, Debug, Clone)]
pub struct ConsoleInputEvent {
    /// The raw command string to execute.
    pub command: String,
}

impl ConsoleInputEvent {
    /// Create a new input event.
    pub fn new(command: impl Into<String>) -> Self {
        Self {
            command: command.into(),
        }
    }
}

/// Event sent when output should be displayed in the console.
///
/// # Examples
///
/// ```ignore
/// fn log_to_console(mut events: EventWriter<ConsoleOutputEvent>) {
///     events.send(ConsoleOutputEvent::info("Game started"));
///     events.send(ConsoleOutputEvent::error("Failed to load config"));
/// }
/// ```
#[derive(Message, Debug, Clone)]
pub struct ConsoleOutputEvent {
    /// The message text.
    pub message: String,
    /// The log level/type.
    pub level: ConsoleOutputLevel,
}

/// Log level for console output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ConsoleOutputLevel {
    /// Debug information (gray).
    Debug,
    /// General information (white).
    #[default]
    Info,
    /// Warning (yellow).
    Warn,
    /// Error (red).
    Error,
    /// Command echo (shows the command that was executed).
    Command,
    /// Command result/response.
    Result,
}

impl ConsoleOutputEvent {
    /// Create a new output event.
    pub fn new(level: ConsoleOutputLevel, message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            level,
        }
    }

    /// Create a debug message.
    pub fn debug(message: impl Into<String>) -> Self {
        Self::new(ConsoleOutputLevel::Debug, message)
    }

    /// Create an info message.
    pub fn info(message: impl Into<String>) -> Self {
        Self::new(ConsoleOutputLevel::Info, message)
    }

    /// Create a warning message.
    pub fn warn(message: impl Into<String>) -> Self {
        Self::new(ConsoleOutputLevel::Warn, message)
    }

    /// Create an error message.
    pub fn error(message: impl Into<String>) -> Self {
        Self::new(ConsoleOutputLevel::Error, message)
    }

    /// Create a command echo message.
    pub fn command(message: impl Into<String>) -> Self {
        Self::new(ConsoleOutputLevel::Command, message)
    }

    /// Create a result message.
    pub fn result(message: impl Into<String>) -> Self {
        Self::new(ConsoleOutputLevel::Result, message)
    }
}

/// Event sent when a ConVar value changes.
///
/// Subscribe to this event to react to configuration changes.
///
/// # Examples
///
/// ```ignore
/// fn on_gravity_change(mut events: EventReader<ConVarChangedEvent>) {
///     for event in events.read() {
///         if event.name == "sv_gravity" {
///             info!("Gravity changed to {}", event.new_value);
///         }
///     }
/// }
/// ```
#[derive(Message, Debug, Clone)]
pub struct ConVarChangedEvent {
    /// The name of the ConVar that changed.
    pub name: Box<str>,
    /// The old value as a string.
    pub old_value: String,
    /// The new value as a string.
    pub new_value: String,
}

impl ConVarChangedEvent {
    /// Create a new change event.
    pub fn new(
        name: impl Into<Box<str>>,
        old_value: impl Into<String>,
        new_value: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            old_value: old_value.into(),
            new_value: new_value.into(),
        }
    }
}

/// Event sent when the console is opened or closed.
#[derive(Message, Debug, Clone, Copy)]
pub struct ConsoleToggleEvent {
    /// Whether the console is now open.
    pub open: bool,
}

impl ConsoleToggleEvent {
    /// Create an event for opening the console.
    pub fn opened() -> Self {
        Self { open: true }
    }

    /// Create an event for closing the console.
    pub fn closed() -> Self {
        Self { open: false }
    }
}

/// Event requesting the console to clear its output buffer.
#[derive(Message, Debug, Clone, Copy, Default)]
pub struct ConsoleClearEvent;

/// Plugin that registers all console events.
pub struct ConsoleEventsPlugin;

impl Plugin for ConsoleEventsPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<ConsoleInputEvent>()
            .add_message::<ConsoleOutputEvent>()
            .add_message::<ConVarChangedEvent>()
            .add_message::<ConsoleToggleEvent>()
            .add_message::<ConsoleClearEvent>();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_console_input_event() {
        let event = ConsoleInputEvent::new("sv_cheats 1");
        assert_eq!(event.command, "sv_cheats 1");
    }

    #[test]
    fn test_console_output_event() {
        let event = ConsoleOutputEvent::error("Something went wrong");
        assert_eq!(event.level, ConsoleOutputLevel::Error);
        assert_eq!(event.message, "Something went wrong");
    }

    #[test]
    fn test_convar_changed_event() {
        let event = ConVarChangedEvent::new("sv_gravity", "800", "1000");
        assert_eq!(&*event.name, "sv_gravity");
        assert_eq!(event.old_value, "800");
        assert_eq!(event.new_value, "1000");
    }
}
