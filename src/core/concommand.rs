//! Console command (ConCommand) implementation.
//!
//! ConCommands are named commands that execute functions when invoked.

use bevy::prelude::*;

use super::{ConVarFlags, PermissionLevel};

/// Arguments passed to a command handler.
#[derive(Debug, Clone)]
pub struct CommandArgs<'a> {
    /// The raw command string.
    raw: &'a str,
    /// Parsed arguments (excluding command name).
    args: Vec<&'a str>,
}

impl<'a> CommandArgs<'a> {
    /// Create new command args from a raw string and parsed arguments.
    pub fn new(raw: &'a str, args: Vec<&'a str>) -> Self {
        Self { raw, args }
    }

    /// Get the raw command string.
    #[inline]
    pub fn raw(&self) -> &str {
        self.raw
    }

    /// Get the number of arguments.
    #[inline]
    pub fn len(&self) -> usize {
        self.args.len()
    }

    /// Check if there are no arguments.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.args.is_empty()
    }

    /// Get an argument by index.
    #[inline]
    pub fn get(&self, index: usize) -> Option<&str> {
        self.args.get(index).copied()
    }

    /// Get an argument or a default value.
    #[inline]
    pub fn get_or(&self, index: usize, default: &'a str) -> &str {
        self.args.get(index).copied().unwrap_or(default)
    }

    /// Try to parse an argument as a specific type.
    pub fn parse<T: std::str::FromStr>(&self, index: usize) -> Option<T> {
        self.get(index).and_then(|s| s.parse().ok())
    }

    /// Parse an argument with a default value.
    pub fn parse_or<T: std::str::FromStr>(&self, index: usize, default: T) -> T {
        self.parse(index).unwrap_or(default)
    }

    /// Get all arguments as a slice.
    #[inline]
    pub fn as_slice(&self) -> &[&str] {
        &self.args
    }

    /// Iterate over arguments.
    #[inline]
    pub fn iter(&self) -> impl Iterator<Item = &str> {
        self.args.iter().copied()
    }

    /// Join all arguments with a space (useful for string arguments).
    pub fn join(&self, separator: &str) -> String {
        self.args.join(separator)
    }

    /// Join arguments starting from an index.
    pub fn join_from(&self, start: usize, separator: &str) -> String {
        self.args.get(start..).unwrap_or(&[]).join(separator)
    }
}

impl<'a> std::ops::Index<usize> for CommandArgs<'a> {
    type Output = str;

    fn index(&self, index: usize) -> &Self::Output {
        self.args[index]
    }
}

/// Type alias for command handler functions.
///
/// Handlers receive:
/// - `args`: The parsed command arguments
/// - `world`: Mutable access to the Bevy world
pub type CommandHandler = Box<dyn Fn(&CommandArgs, &mut World) + Send + Sync>;

/// Type alias for autocomplete provider functions.
///
/// Receives the partial input and returns a list of suggestions.
pub type AutocompleteProvider = Box<dyn Fn(&str) -> Vec<String> + Send + Sync>;

/// Metadata for a console command (stored in registry).
///
/// The handler is stored separately in `CommandHandlers` to avoid borrow conflicts.
#[derive(Debug)]
pub struct ConCommandMeta {
    /// The command name.
    pub name: Box<str>,
    /// Description.
    pub description: &'static str,
    /// Flags.
    pub flags: ConVarFlags,
    /// Required permission level.
    pub required_permission: PermissionLevel,
}

impl ConCommandMeta {
    /// Get the command name.
    #[inline]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get the description.
    #[inline]
    pub fn get_description(&self) -> &'static str {
        self.description
    }

    /// Get the flags.
    #[inline]
    pub fn get_flags(&self) -> ConVarFlags {
        self.flags
    }

    /// Get the required permission level.
    #[inline]
    pub fn get_required_permission(&self) -> PermissionLevel {
        self.required_permission
    }
}

/// A console command with a handler function.
///
/// # Examples
///
/// ```ignore
/// let quit_cmd = ConCommand::new("quit", |_args, world| {
///     world.send_event(AppExit::default());
/// }).description("Exit the game");
///
/// let echo_cmd = ConCommand::new("echo", |args, _world| {
///     println!("{}", args.join(" "));
/// }).description("Print text to console");
/// ```
pub struct ConCommand {
    name: Box<str>,
    description: &'static str,
    flags: ConVarFlags,
    required_permission: PermissionLevel,
    handler: CommandHandler,
    autocomplete: Option<AutocompleteProvider>,
}

impl ConCommand {
    /// Create a new command with the given name and handler.
    pub fn new<F>(name: impl Into<Box<str>>, handler: F) -> Self
    where
        F: Fn(&CommandArgs, &mut World) + Send + Sync + 'static,
    {
        Self {
            name: name.into(),
            description: "",
            flags: ConVarFlags::NONE,
            required_permission: PermissionLevel::User,
            handler: Box::new(handler),
            autocomplete: None,
        }
    }

    /// Set the description.
    pub fn description(mut self, desc: &'static str) -> Self {
        self.description = desc;
        self
    }

    /// Set the flags.
    pub fn flags(mut self, flags: ConVarFlags) -> Self {
        self.flags = flags;
        self
    }

    /// Set the autocomplete provider.
    pub fn autocomplete<F>(mut self, provider: F) -> Self
    where
        F: Fn(&str) -> Vec<String> + Send + Sync + 'static,
    {
        self.autocomplete = Some(Box::new(provider));
        self
    }

    /// Set the required permission level.
    pub fn permission(mut self, level: PermissionLevel) -> Self {
        self.required_permission = level;
        self
    }

    /// Get the command name.
    #[inline]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get the description.
    #[inline]
    pub fn get_description(&self) -> &'static str {
        self.description
    }

    /// Get the flags.
    #[inline]
    pub fn get_flags(&self) -> ConVarFlags {
        self.flags
    }

    /// Get the required permission level.
    #[inline]
    pub fn get_required_permission(&self) -> PermissionLevel {
        self.required_permission
    }

    /// Execute the command with the given arguments.
    pub fn execute(&self, args: &CommandArgs, world: &mut World) {
        (self.handler)(args, world);
    }

    /// Get autocomplete suggestions for the given partial input.
    pub fn get_completions(&self, partial: &str) -> Vec<String> {
        self.autocomplete
            .as_ref()
            .map(|f| f(partial))
            .unwrap_or_default()
    }

    /// Check if this command has an autocomplete provider.
    #[inline]
    pub fn has_autocomplete(&self) -> bool {
        self.autocomplete.is_some()
    }

    /// Split the command into metadata and handler.
    ///
    /// This is used internally to store metadata and handler separately.
    pub fn split(self) -> (ConCommandMeta, CommandHandler, Option<AutocompleteProvider>) {
        (
            ConCommandMeta {
                name: self.name,
                description: self.description,
                flags: self.flags,
                required_permission: self.required_permission,
            },
            self.handler,
            self.autocomplete,
        )
    }
}

impl std::fmt::Debug for ConCommand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConCommand")
            .field("name", &self.name)
            .field("description", &self.description)
            .field("flags", &self.flags)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_args_basic() {
        let args = CommandArgs::new("echo hello world", vec!["hello", "world"]);
        assert_eq!(args.len(), 2);
        assert_eq!(args.get(0), Some("hello"));
        assert_eq!(args.get(1), Some("world"));
        assert_eq!(args.get(2), None);
    }

    #[test]
    fn test_command_args_parse() {
        let args = CommandArgs::new("set 42", vec!["42"]);
        assert_eq!(args.parse::<i32>(0), Some(42));
        assert_eq!(args.parse::<i32>(1), None);
        assert_eq!(args.parse_or::<i32>(1, 0), 0);
    }

    #[test]
    fn test_command_args_join() {
        let args = CommandArgs::new("echo hello world", vec!["hello", "world"]);
        assert_eq!(args.join(" "), "hello world");
        assert_eq!(args.join_from(1, " "), "world");
    }

    #[test]
    fn test_concommand_creation() {
        let cmd = ConCommand::new("test", |_args, _world| {})
            .description("A test command")
            .flags(ConVarFlags::CHEAT);

        assert_eq!(cmd.name(), "test");
        assert_eq!(cmd.get_description(), "A test command");
        assert!(cmd.get_flags().contains(ConVarFlags::CHEAT));
    }
}
