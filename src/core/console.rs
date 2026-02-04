//! Unified console API for convenient access.
//!
//! The [`Console`] system parameter provides a simplified interface for working
//! with console variables and commands, combining [`ConsoleRegistry`] and
//! [`CommandHandlers`] into a single ergonomic API.

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;

use super::{
    ConCommand, ConVar, ConVarValue, ConsoleRegistry, CommandHandlers,
    ConEntry, ConVarMeta, ConCommandMeta,
};

/// Unified console system parameter for convenient access.
///
/// This combines [`ConsoleRegistry`] and [`CommandHandlers`] into a single
/// interface, providing a simpler API for common operations.
///
/// # Examples
///
/// ```ignore
/// fn setup_console(mut console: Console) {
///     // Register a variable
///     console.register_var(ConVar::new("sv_gravity", 800.0f32)
///         .description("World gravity"));
///
///     // Register a command
///     console.register_cmd(ConCommand::new("noclip", |_, world| {
///         info!("Noclip toggled!");
///     }).description("Toggle noclip mode"));
///
///     // Get/set values
///     let gravity: f32 = console.get("sv_gravity").unwrap();
///     console.set("sv_gravity", 1000.0f32);
/// }
/// ```
#[derive(SystemParam)]
pub struct Console<'w> {
    registry: ResMut<'w, ConsoleRegistry>,
    handlers: ResMut<'w, CommandHandlers>,
}

impl Console<'_> {
    /// Register a console variable.
    ///
    /// Returns `true` if newly registered, `false` if it replaced an existing entry.
    pub fn register_var<T: ConVarValue + PartialEq>(&mut self, cvar: ConVar<T>) -> bool {
        self.registry.register_var(cvar)
    }

    /// Register a console command.
    ///
    /// This handles both the metadata (in registry) and handler (in handlers) registration.
    /// Returns `true` if newly registered, `false` if it replaced an existing entry.
    pub fn register_cmd(&mut self, cmd: ConCommand) -> bool {
        let (name, handler, autocomplete, is_new) = self.registry.register_cmd(cmd);
        self.handlers.register(name, handler, autocomplete);
        is_new
    }

    /// Get a ConVar's typed value by name.
    pub fn get<T: ConVarValue + PartialEq + 'static>(&self, name: &str) -> Option<T> {
        self.registry.get(name)
    }

    /// Get a ConVar's value as a string.
    pub fn get_string(&self, name: &str) -> Option<String> {
        self.registry.get_string(name)
    }

    /// Set a ConVar's typed value.
    ///
    /// Returns `true` if successful, `false` if the variable doesn't exist or type mismatch.
    pub fn set<T: ConVarValue + PartialEq + 'static>(&mut self, name: &str, value: T) -> bool {
        self.registry.set(name, value)
    }

    /// Set a ConVar's value from a string.
    ///
    /// Returns `true` if successful, `false` if the variable doesn't exist or parse failed.
    pub fn set_string(&mut self, name: &str, value: &str) -> bool {
        self.registry.set_string(name, value)
    }

    /// Check if an entry (variable or command) exists.
    pub fn contains(&self, name: &str) -> bool {
        self.registry.contains(name)
    }

    /// Get an entry by name.
    pub fn get_entry(&self, name: &str) -> Option<&ConEntry> {
        self.registry.get_entry(name)
    }

    /// Get the number of registered entries.
    pub fn len(&self) -> usize {
        self.registry.len()
    }

    /// Check if the console registry is empty.
    pub fn is_empty(&self) -> bool {
        self.registry.is_empty()
    }

    /// Iterate over all variables.
    pub fn vars(&self) -> impl Iterator<Item = (&str, &ConVarMeta)> {
        self.registry.vars()
    }

    /// Iterate over all commands.
    pub fn cmds(&self) -> impl Iterator<Item = (&str, &ConCommandMeta)> {
        self.registry.cmds()
    }

    /// Iterate over variables with non-default values.
    pub fn modified_vars(&self) -> impl Iterator<Item = (&str, &ConVarMeta)> {
        self.registry.modified_vars()
    }

    /// Iterate over variables with the ARCHIVE flag.
    pub fn archive_vars(&self) -> impl Iterator<Item = (&str, &ConVarMeta)> {
        self.registry.archive_vars()
    }

    /// Find entries with a given prefix.
    pub fn prefix_iter(&self, prefix: &str) -> impl Iterator<Item = (&str, &ConEntry)> {
        self.registry.prefix_iter(prefix)
    }

    /// Search entries by name or description.
    pub fn search(&self, query: &str) -> Vec<(&str, &ConEntry)> {
        self.registry.search(query)
    }

    /// Get autocomplete suggestions for a command's arguments.
    pub fn get_completions(&self, cmd_name: &str, partial: &str) -> Vec<String> {
        self.handlers.get_completions(cmd_name, partial)
    }

    /// Get read-only access to the underlying registry.
    ///
    /// Use this for advanced operations not covered by the Console API.
    pub fn registry(&self) -> &ConsoleRegistry {
        &self.registry
    }

    /// Get read-only access to the underlying handlers.
    ///
    /// Use this for advanced operations not covered by the Console API.
    pub fn handlers(&self) -> &CommandHandlers {
        &self.handlers
    }
}

/// Read-only console system parameter.
///
/// Use this when you only need to read console values, not modify them.
/// This allows for better parallelism in Bevy's scheduler.
#[derive(SystemParam)]
pub struct ConsoleRef<'w> {
    registry: Res<'w, ConsoleRegistry>,
    handlers: Res<'w, CommandHandlers>,
}

impl ConsoleRef<'_> {
    /// Get a ConVar's typed value by name.
    pub fn get<T: ConVarValue + PartialEq + 'static>(&self, name: &str) -> Option<T> {
        self.registry.get(name)
    }

    /// Get a ConVar's value as a string.
    pub fn get_string(&self, name: &str) -> Option<String> {
        self.registry.get_string(name)
    }

    /// Check if an entry exists.
    pub fn contains(&self, name: &str) -> bool {
        self.registry.contains(name)
    }

    /// Get an entry by name.
    pub fn get_entry(&self, name: &str) -> Option<&ConEntry> {
        self.registry.get_entry(name)
    }

    /// Iterate over all variables.
    pub fn vars(&self) -> impl Iterator<Item = (&str, &ConVarMeta)> {
        self.registry.vars()
    }

    /// Iterate over all commands.
    pub fn cmds(&self) -> impl Iterator<Item = (&str, &ConCommandMeta)> {
        self.registry.cmds()
    }

    /// Search entries by name or description.
    pub fn search(&self, query: &str) -> Vec<(&str, &ConEntry)> {
        self.registry.search(query)
    }

    /// Get autocomplete suggestions for a command's arguments.
    pub fn get_completions(&self, cmd_name: &str, partial: &str) -> Vec<String> {
        self.handlers.get_completions(cmd_name, partial)
    }

    /// Get read-only access to the underlying registry.
    pub fn registry(&self) -> &ConsoleRegistry {
        &self.registry
    }
}
