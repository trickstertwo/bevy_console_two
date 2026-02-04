//! Persistence layer for console configuration.
//!
//! Provides RON-based save/load for ARCHIVE convars and command aliases.

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::core::ConsoleRegistry;

/// Default config file name.
pub const DEFAULT_CONFIG_FILE: &str = "console.ron";

/// Serializable console configuration.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ConsoleConfigFile {
    /// ConVar values (name -> string value).
    #[serde(default)]
    pub convars: HashMap<String, String>,
    /// Command aliases (alias -> command).
    #[serde(default)]
    pub aliases: HashMap<String, String>,
}

impl ConsoleConfigFile {
    /// Create a new empty config.
    pub fn new() -> Self {
        Self::default()
    }

    /// Load config from a RON file.
    pub fn load(path: impl AsRef<Path>) -> Result<Self, ConfigError> {
        let path = path.as_ref();
        let contents = fs::read_to_string(path)
            .map_err(|e| ConfigError::Io(path.display().to_string(), e.to_string()))?;

        ron::from_str(&contents)
            .map_err(|e| ConfigError::Parse(path.display().to_string(), e.to_string()))
    }

    /// Save config to a RON file.
    pub fn save(&self, path: impl AsRef<Path>) -> Result<(), ConfigError> {
        let path = path.as_ref();

        // Create parent directories if needed
        if let Some(parent) = path.parent() {
            if !parent.exists() {
                fs::create_dir_all(parent)
                    .map_err(|e| ConfigError::Io(parent.display().to_string(), e.to_string()))?;
            }
        }

        let pretty = ron::ser::PrettyConfig::new()
            .depth_limit(2)
            .separate_tuple_members(true)
            .enumerate_arrays(false);

        let contents = ron::ser::to_string_pretty(self, pretty)
            .map_err(|e| ConfigError::Serialize(e.to_string()))?;

        fs::write(path, contents)
            .map_err(|e| ConfigError::Io(path.display().to_string(), e.to_string()))
    }

    /// Load config from file, returning default if file doesn't exist.
    pub fn load_or_default(path: impl AsRef<Path>) -> Self {
        Self::load(path).unwrap_or_default()
    }
}

/// Errors that can occur during config operations.
#[derive(Debug, Clone)]
pub enum ConfigError {
    /// IO error (path, message).
    Io(String, String),
    /// Parse error (path, message).
    Parse(String, String),
    /// Serialization error.
    Serialize(String),
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigError::Io(path, msg) => write!(f, "IO error for '{}': {}", path, msg),
            ConfigError::Parse(path, msg) => write!(f, "Parse error for '{}': {}", path, msg),
            ConfigError::Serialize(msg) => write!(f, "Serialization error: {}", msg),
        }
    }
}

impl std::error::Error for ConfigError {}

/// Resource storing command aliases.
#[derive(Resource, Default, Debug)]
pub struct CommandAliases {
    aliases: HashMap<String, String>,
}

impl CommandAliases {
    /// Create new empty aliases.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add an alias.
    pub fn add(&mut self, name: impl Into<String>, command: impl Into<String>) {
        self.aliases.insert(name.into(), command.into());
    }

    /// Remove an alias.
    pub fn remove(&mut self, name: &str) -> Option<String> {
        self.aliases.remove(name)
    }

    /// Get an alias.
    pub fn get(&self, name: &str) -> Option<&str> {
        self.aliases.get(name).map(|s| s.as_str())
    }

    /// Check if an alias exists.
    pub fn contains(&self, name: &str) -> bool {
        self.aliases.contains_key(name)
    }

    /// Iterate over all aliases.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.aliases.iter().map(|(k, v)| (k.as_str(), v.as_str()))
    }

    /// Get the number of aliases.
    pub fn len(&self) -> usize {
        self.aliases.len()
    }

    /// Check if there are no aliases.
    pub fn is_empty(&self) -> bool {
        self.aliases.is_empty()
    }
}

/// Resource tracking the config file path.
#[derive(Resource, Debug, Clone)]
pub struct ConfigPath(pub String);

impl Default for ConfigPath {
    fn default() -> Self {
        Self(DEFAULT_CONFIG_FILE.to_string())
    }
}

/// Extract ARCHIVE convars from registry into a config.
pub fn extract_archive_convars(registry: &ConsoleRegistry) -> ConsoleConfigFile {
    let mut config = ConsoleConfigFile::new();

    for (name, meta) in registry.archive_vars() {
        config.convars.insert(name.to_string(), meta.get_string());
    }

    config
}

/// Apply config values to registry.
pub fn apply_config_to_registry(config: &ConsoleConfigFile, registry: &mut ConsoleRegistry) {
    for (name, value) in &config.convars {
        if registry.set_string(name, value) {
            debug!("Loaded convar: {} = \"{}\"", name, value);
        } else {
            warn!("Failed to set convar '{}' to '{}'", name, value);
        }
    }
}

/// System to load config on startup.
pub fn load_config_on_startup(
    mut registry: ResMut<ConsoleRegistry>,
    mut aliases: ResMut<CommandAliases>,
    config_path: Res<ConfigPath>,
) {
    let path = &config_path.0;

    if !Path::new(path).exists() {
        info!("No config file found at '{}', using defaults", path);
        return;
    }

    match ConsoleConfigFile::load(path) {
        Ok(config) => {
            info!("Loading config from '{}'", path);
            apply_config_to_registry(&config, &mut registry);

            // Load aliases
            for (name, command) in &config.aliases {
                aliases.add(name.clone(), command.clone());
                debug!("Loaded alias: {} -> {}", name, command);
            }

            info!("Loaded {} convars and {} aliases",
                config.convars.len(), config.aliases.len());
        }
        Err(e) => {
            error!("Failed to load config: {}", e);
        }
    }
}

/// Save current ARCHIVE convars to file.
pub fn save_config(
    registry: &ConsoleRegistry,
    aliases: &CommandAliases,
    path: impl AsRef<Path>,
) -> Result<(), ConfigError> {
    let mut config = extract_archive_convars(registry);

    // Add aliases
    for (name, command) in aliases.iter() {
        config.aliases.insert(name.to_string(), command.to_string());
    }

    config.save(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_config_file_roundtrip() {
        let mut config = ConsoleConfigFile::new();
        config.convars.insert("sv_gravity".to_string(), "800".to_string());
        config.convars.insert("sv_cheats".to_string(), "0".to_string());
        config.aliases.insert("quit".to_string(), "exit".to_string());

        let temp = NamedTempFile::new().unwrap();
        config.save(temp.path()).unwrap();

        let loaded = ConsoleConfigFile::load(temp.path()).unwrap();
        assert_eq!(loaded.convars.get("sv_gravity"), Some(&"800".to_string()));
        assert_eq!(loaded.convars.get("sv_cheats"), Some(&"0".to_string()));
        assert_eq!(loaded.aliases.get("quit"), Some(&"exit".to_string()));
    }

    #[test]
    fn test_config_file_load_missing() {
        let result = ConsoleConfigFile::load("nonexistent_file.ron");
        assert!(result.is_err());
    }

    #[test]
    fn test_config_file_load_or_default() {
        let config = ConsoleConfigFile::load_or_default("nonexistent_file.ron");
        assert!(config.convars.is_empty());
        assert!(config.aliases.is_empty());
    }

    #[test]
    fn test_command_aliases() {
        let mut aliases = CommandAliases::new();

        aliases.add("q", "quit");
        aliases.add("nc", "noclip");

        assert_eq!(aliases.get("q"), Some("quit"));
        assert_eq!(aliases.get("nc"), Some("noclip"));
        assert_eq!(aliases.get("unknown"), None);
        assert!(aliases.contains("q"));
        assert!(!aliases.contains("unknown"));
        assert_eq!(aliases.len(), 2);

        aliases.remove("q");
        assert_eq!(aliases.get("q"), None);
        assert_eq!(aliases.len(), 1);
    }

    #[test]
    fn test_config_parse_ron() {
        let ron_content = r#"(
    convars: {
        "sv_gravity": "800",
        "cl_fov": "90",
    },
    aliases: {
        "q": "quit",
    },
)"#;

        let mut temp = NamedTempFile::new().unwrap();
        temp.write_all(ron_content.as_bytes()).unwrap();
        temp.flush().unwrap();

        let config = ConsoleConfigFile::load(temp.path()).unwrap();
        assert_eq!(config.convars.get("sv_gravity"), Some(&"800".to_string()));
        assert_eq!(config.convars.get("cl_fov"), Some(&"90".to_string()));
        assert_eq!(config.aliases.get("q"), Some(&"quit".to_string()));
    }
}
