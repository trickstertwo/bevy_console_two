//! Console registry for ConVars and ConCommands.
//!
//! Central storage with trie-based lookup for fast autocomplete.

use std::collections::HashMap;

use bevy::prelude::*;

use super::{
    ConCommand, ConCommandMeta, ConVar, ConVarDyn, ConVarFlags, ConVarValue,
    Trie, subsequence_match, matcher::MatchResult,
    CommandHandler, concommand::AutocompleteProvider,
    PermissionLevel,
};

/// Entry type in the console registry.
pub enum ConEntry {
    /// A console variable.
    Var(ConVarMeta),
    /// A console command (metadata only, handler stored separately).
    Cmd(ConCommandMeta),
}

impl ConEntry {
    /// Get the name of this entry.
    pub fn name(&self) -> &str {
        match self {
            ConEntry::Var(meta) => &meta.name,
            ConEntry::Cmd(meta) => meta.name(),
        }
    }

    /// Get the description of this entry.
    pub fn description(&self) -> &str {
        match self {
            ConEntry::Var(meta) => meta.description,
            ConEntry::Cmd(meta) => meta.get_description(),
        }
    }

    /// Get the flags of this entry.
    pub fn flags(&self) -> ConVarFlags {
        match self {
            ConEntry::Var(meta) => meta.flags,
            ConEntry::Cmd(meta) => meta.get_flags(),
        }
    }

    /// Check if this is a variable.
    pub fn is_var(&self) -> bool {
        matches!(self, ConEntry::Var(_))
    }

    /// Check if this is a command.
    pub fn is_cmd(&self) -> bool {
        matches!(self, ConEntry::Cmd(_))
    }

    /// Get the required permission level of this entry.
    pub fn required_permission(&self) -> PermissionLevel {
        match self {
            ConEntry::Var(meta) => meta.required_permission,
            ConEntry::Cmd(meta) => meta.get_required_permission(),
        }
    }
}

/// Stores command handlers separately from metadata.
///
/// This separation allows command handlers to access `World` (including `ConsoleRegistry`)
/// without borrow conflicts.
#[derive(Resource, Default)]
pub struct CommandHandlers {
    handlers: HashMap<Box<str>, CommandHandler>,
    autocomplete: HashMap<Box<str>, AutocompleteProvider>,
}

impl CommandHandlers {
    /// Create a new empty handler storage.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a handler for a command.
    pub fn register(&mut self, name: Box<str>, handler: CommandHandler, autocomplete: Option<AutocompleteProvider>) {
        self.handlers.insert(name.clone(), handler);
        if let Some(ac) = autocomplete {
            self.autocomplete.insert(name, ac);
        }
    }

    /// Get a handler by name.
    pub fn get(&self, name: &str) -> Option<&CommandHandler> {
        self.handlers.get(name)
    }

    /// Take a handler temporarily for execution.
    ///
    /// Use `put` to return the handler after execution.
    pub fn take(&mut self, name: &str) -> Option<CommandHandler> {
        self.handlers.remove(name)
    }

    /// Put a handler back after temporary removal.
    pub fn put(&mut self, name: &str, handler: CommandHandler) {
        self.handlers.insert(name.into(), handler);
    }

    /// Get autocomplete suggestions for a command.
    pub fn get_completions(&self, name: &str, partial: &str) -> Vec<String> {
        self.autocomplete
            .get(name)
            .map(|f| f(partial))
            .unwrap_or_default()
    }

    /// Check if a command has an autocomplete provider.
    pub fn has_autocomplete(&self, name: &str) -> bool {
        self.autocomplete.contains_key(name)
    }
}

/// Metadata for a type-erased ConVar.
pub struct ConVarMeta {
    /// The variable name.
    pub name: Box<str>,
    /// Description.
    pub description: &'static str,
    /// Flags.
    pub flags: ConVarFlags,
    /// Required permission level.
    pub required_permission: PermissionLevel,
    /// Type-erased value storage.
    value: Box<dyn ConVarDyn>,
}

impl ConVarMeta {
    /// Create from a typed ConVar.
    pub fn from_convar<T: ConVarValue + PartialEq>(cvar: ConVar<T>) -> Self {
        Self {
            name: cvar.name().into(),
            description: cvar.get_description(),
            flags: cvar.get_flags(),
            required_permission: cvar.get_required_permission(),
            value: Box::new(cvar),
        }
    }

    /// Get the current value as a string.
    pub fn get_string(&self) -> String {
        self.value.get_string()
    }

    /// Set the value from a string.
    pub fn set_string(&mut self, s: &str) -> bool {
        self.value.set_string(s)
    }

    /// Get the default value as a string.
    pub fn default_string(&self) -> String {
        self.value.default_string()
    }

    /// Reset to default.
    pub fn reset(&mut self) {
        self.value.reset();
    }

    /// Check if modified from default.
    pub fn is_modified(&self) -> bool {
        self.value.is_modified()
    }

    /// Try to downcast to a specific ConVar type.
    pub fn downcast_ref<T: ConVarValue + PartialEq + 'static>(&self) -> Option<&ConVar<T>> {
        self.value.as_any().downcast_ref()
    }

    /// Try to downcast to a specific ConVar type (mutable).
    pub fn downcast_mut<T: ConVarValue + PartialEq + 'static>(&mut self) -> Option<&mut ConVar<T>> {
        self.value.as_any_mut().downcast_mut()
    }
}

/// Central registry for console variables and commands.
///
/// Uses a trie for O(k) lookup and fast prefix iteration for autocomplete.
///
/// # Examples
///
/// ```ignore
/// let mut registry = ConsoleRegistry::new();
///
/// // Register a variable
/// registry.register_var(ConVar::new("sv_gravity", 800.0f32)
///     .description("World gravity")
///     .flags(ConVarFlags::ARCHIVE));
///
/// // Register a command
/// registry.register_cmd(ConCommand::new("quit", |_, world| {
///     world.send_event(AppExit::default());
/// }));
///
/// // Lookup
/// let gravity: f32 = registry.get("sv_gravity").unwrap();
/// ```
#[derive(Resource, Default)]
pub struct ConsoleRegistry {
    /// Trie for fast prefix lookup.
    trie: Trie<()>,
    /// Actual storage (trie stores () to save memory, we lookup here).
    entries: HashMap<Box<str>, ConEntry>,
}

impl ConsoleRegistry {
    /// Create a new empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a console variable.
    ///
    /// Returns `true` if the variable was newly registered, `false` if it replaced an existing entry.
    /// A warning is logged if a duplicate is detected.
    pub fn register_var<T: ConVarValue + PartialEq>(&mut self, cvar: ConVar<T>) -> bool {
        let name: Box<str> = cvar.name().into();
        let is_duplicate = self.entries.contains_key(&name);

        if is_duplicate {
            bevy::log::warn!(
                "Console: Overwriting existing entry '{}' with new variable",
                name
            );
        }

        self.trie.insert(&name, ());
        self.entries.insert(name, ConEntry::Var(ConVarMeta::from_convar(cvar)));
        !is_duplicate
    }

    /// Register a console command's metadata.
    ///
    /// Note: The handler must be registered separately in `CommandHandlers`.
    /// Use `register_cmd_full` for a complete registration when you have access to `CommandHandlers`.
    ///
    /// Returns `true` if newly registered, `false` if it replaced an existing entry.
    pub fn register_cmd_meta(&mut self, meta: ConCommandMeta) -> bool {
        let name: Box<str> = meta.name.clone();
        let is_duplicate = self.entries.contains_key(&name);

        if is_duplicate {
            bevy::log::warn!(
                "Console: Overwriting existing entry '{}' with new command",
                name
            );
        }

        self.trie.insert(&name, ());
        self.entries.insert(name, ConEntry::Cmd(meta));
        !is_duplicate
    }

    /// Register a console command, returning the handler for separate storage.
    ///
    /// The returned tuple contains (name, handler, autocomplete, is_new) which should be
    /// stored in `CommandHandlers`. `is_new` is `false` if an existing entry was overwritten.
    pub fn register_cmd(&mut self, cmd: ConCommand) -> (Box<str>, CommandHandler, Option<AutocompleteProvider>, bool) {
        let (meta, handler, autocomplete) = cmd.split();
        let name = meta.name.clone();
        let is_duplicate = self.entries.contains_key(&name);

        if is_duplicate {
            bevy::log::warn!(
                "Console: Overwriting existing entry '{}' with new command",
                name
            );
        }

        self.trie.insert(&name, ());
        self.entries.insert(name.clone(), ConEntry::Cmd(meta));
        (name, handler, autocomplete, !is_duplicate)
    }

    /// Get an entry by name.
    pub fn get_entry(&self, name: &str) -> Option<&ConEntry> {
        self.entries.get(name)
    }

    /// Get a mutable entry by name.
    pub fn get_entry_mut(&mut self, name: &str) -> Option<&mut ConEntry> {
        self.entries.get_mut(name)
    }

    /// Get a ConVar's value by name.
    pub fn get<T: ConVarValue + PartialEq + 'static>(&self, name: &str) -> Option<T> {
        match self.entries.get(name)? {
            ConEntry::Var(meta) => meta.downcast_ref::<T>().map(|cvar| cvar.get()),
            ConEntry::Cmd(_) => None,
        }
    }

    /// Get a ConVar's value as a string.
    pub fn get_string(&self, name: &str) -> Option<String> {
        match self.entries.get(name)? {
            ConEntry::Var(meta) => Some(meta.get_string()),
            ConEntry::Cmd(_) => None,
        }
    }

    /// Set a ConVar's value.
    pub fn set<T: ConVarValue + PartialEq + 'static>(&mut self, name: &str, value: T) -> bool {
        match self.entries.get_mut(name) {
            Some(ConEntry::Var(meta)) => {
                if let Some(cvar) = meta.downcast_mut::<T>() {
                    cvar.set(value)
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    /// Set a ConVar's value from a string.
    pub fn set_string(&mut self, name: &str, value: &str) -> bool {
        match self.entries.get_mut(name) {
            Some(ConEntry::Var(meta)) => meta.set_string(value),
            _ => false,
        }
    }

    /// Check if an entry exists.
    pub fn contains(&self, name: &str) -> bool {
        self.entries.contains_key(name)
    }

    /// Get the number of entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Iterate over all entries with a given prefix.
    pub fn prefix_iter(&self, prefix: &str) -> impl Iterator<Item = (&str, &ConEntry)> {
        self.trie
            .prefix_iter(prefix)
            .filter_map(|(name, _)| self.entries.get(name).map(|e| (name, e)))
    }

    /// Iterate over all entries.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &ConEntry)> {
        self.entries.iter().map(|(k, v)| (k.as_ref(), v))
    }

    /// Iterate over all variables.
    pub fn vars(&self) -> impl Iterator<Item = (&str, &ConVarMeta)> {
        self.entries.iter().filter_map(|(k, v)| match v {
            ConEntry::Var(meta) => Some((k.as_ref(), meta)),
            ConEntry::Cmd(_) => None,
        })
    }

    /// Iterate over all commands.
    pub fn cmds(&self) -> impl Iterator<Item = (&str, &ConCommandMeta)> {
        self.entries.iter().filter_map(|(k, v)| match v {
            ConEntry::Var(_) => None,
            ConEntry::Cmd(meta) => Some((k.as_ref(), meta)),
        })
    }

    /// Iterate over all variables with non-default values.
    pub fn modified_vars(&self) -> impl Iterator<Item = (&str, &ConVarMeta)> {
        self.vars().filter(|(_, meta)| meta.is_modified())
    }

    /// Iterate over all variables with ARCHIVE flag.
    pub fn archive_vars(&self) -> impl Iterator<Item = (&str, &ConVarMeta)> {
        self.vars().filter(|(_, meta)| meta.flags.contains(ConVarFlags::ARCHIVE))
    }

    /// Find entries matching a fuzzy pattern.
    ///
    /// Returns entries sorted by match score (best first).
    pub fn fuzzy_find(&self, pattern: &str) -> Vec<(&str, &ConEntry, MatchResult)> {
        let mut matches: Vec<_> = self
            .entries
            .iter()
            .filter(|(_, entry)| !entry.flags().contains(ConVarFlags::HIDDEN))
            .filter_map(|(name, entry)| {
                subsequence_match(pattern, name).map(|result| (name.as_ref(), entry, result))
            })
            .collect();

        matches.sort_by(|a, b| b.2.score.cmp(&a.2.score).then_with(|| a.0.cmp(b.0)));
        matches
    }

    /// Find entries by searching both name and description.
    pub fn search(&self, query: &str) -> Vec<(&str, &ConEntry)> {
        let query_lower = query.to_lowercase();

        let mut matches: Vec<_> = self
            .entries
            .iter()
            .filter(|(_, entry)| !entry.flags().contains(ConVarFlags::HIDDEN))
            .filter(|(name, entry)| {
                name.to_lowercase().contains(&query_lower)
                    || entry.description().to_lowercase().contains(&query_lower)
            })
            .map(|(name, entry)| (name.as_ref(), entry))
            .collect();

        matches.sort_by(|a, b| a.0.cmp(b.0));
        matches
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_var() {
        let mut registry = ConsoleRegistry::new();

        registry.register_var(ConVar::new("sv_gravity", 800.0f32)
            .description("World gravity"));

        assert!(registry.contains("sv_gravity"));
        assert_eq!(registry.get::<f32>("sv_gravity"), Some(800.0));
        assert_eq!(registry.get_string("sv_gravity"), Some("800".to_string()));
    }

    #[test]
    fn test_registry_set() {
        let mut registry = ConsoleRegistry::new();

        registry.register_var(ConVar::new("sv_gravity", 800.0f32));

        assert!(registry.set("sv_gravity", 1000.0f32));
        assert_eq!(registry.get::<f32>("sv_gravity"), Some(1000.0));

        assert!(registry.set_string("sv_gravity", "500"));
        assert_eq!(registry.get::<f32>("sv_gravity"), Some(500.0));
    }

    #[test]
    fn test_registry_cmd() {
        let mut registry = ConsoleRegistry::new();

        // Note: We only test that metadata was registered, handler is intentionally ignored
        let (_, _, _, is_new) = registry.register_cmd(ConCommand::new("test", |_, _| {})
            .description("Test command"));

        assert!(is_new);
        assert!(registry.contains("test"));
        assert!(registry.get_entry("test").unwrap().is_cmd());
    }

    #[test]
    fn test_registry_prefix_iter() {
        let mut registry = ConsoleRegistry::new();

        registry.register_var(ConVar::new("sv_gravity", 800.0f32));
        registry.register_var(ConVar::new("sv_cheats", 0i32));
        registry.register_var(ConVar::new("cl_fov", 90i32));

        let sv_entries: Vec<_> = registry.prefix_iter("sv_").collect();
        assert_eq!(sv_entries.len(), 2);

        let cl_entries: Vec<_> = registry.prefix_iter("cl_").collect();
        assert_eq!(cl_entries.len(), 1);
    }

    #[test]
    fn test_registry_fuzzy_find() {
        let mut registry = ConsoleRegistry::new();

        registry.register_var(ConVar::new("sv_gravity", 800.0f32));
        registry.register_var(ConVar::new("sv_cheats", 0i32));
        registry.register_var(ConVar::new("cl_showfps", 0i32));

        let matches = registry.fuzzy_find("svg");
        assert!(!matches.is_empty());
        // sv_gravity should match "svg"
        assert!(matches.iter().any(|(name, _, _)| *name == "sv_gravity"));
    }

    #[test]
    fn test_registry_search() {
        let mut registry = ConsoleRegistry::new();

        registry.register_var(ConVar::new("sv_gravity", 800.0f32)
            .description("World gravity force"));
        registry.register_var(ConVar::new("sv_cheats", 0i32)
            .description("Enable cheats"));

        // Search by name
        let matches = registry.search("gravity");
        assert_eq!(matches.len(), 1);

        // Search by description
        let matches = registry.search("cheats");
        assert_eq!(matches.len(), 1);
    }

    #[test]
    fn test_registry_modified_vars() {
        let mut registry = ConsoleRegistry::new();

        registry.register_var(ConVar::new("sv_gravity", 800.0f32));
        registry.register_var(ConVar::new("sv_cheats", 0i32));

        // Initially no modified vars
        assert_eq!(registry.modified_vars().count(), 0);

        // Modify one
        registry.set("sv_gravity", 1000.0f32);
        assert_eq!(registry.modified_vars().count(), 1);
    }

    #[test]
    fn test_duplicate_detection() {
        let mut registry = ConsoleRegistry::new();

        // First registration should succeed
        let is_new = registry.register_var(ConVar::new("test_var", 42i32));
        assert!(is_new);

        // Duplicate var registration should return false
        let is_new = registry.register_var(ConVar::new("test_var", 100i32));
        assert!(!is_new);

        // Value should be updated to new value
        assert_eq!(registry.get::<i32>("test_var"), Some(100));

        // Command registration
        let (_, _, _, is_new) = registry.register_cmd(ConCommand::new("test_cmd", |_, _| {}));
        assert!(is_new);

        // Duplicate command should return false
        let (_, _, _, is_new) = registry.register_cmd(ConCommand::new("test_cmd", |_, _| {}));
        assert!(!is_new);
    }
}
