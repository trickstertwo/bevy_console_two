//! A minimal, extensible developer console for Bevy.
//!
//! Inspired by the Source Engine ConVar system, bevy_console provides:
//!
//! - **ConVar**: Typed console variables with constraints and flags
//! - **ConCommand**: Console commands with handlers
//! - **Console**: Unified system parameter for convenient access
//! - **Fuzzy matching**: Zero-dependency autocomplete
//!
//! # Features
//!
//! - `egui` (default): egui-based UI with log capture
//! - `terminal`: stdin/stdout backend for dedicated servers
//! - `persist`: RON configuration persistence (exec, host_writeconfig, alias)
//! - `full`: Enable egui + persist
//!
//! # Quick Start
//!
//! ```ignore
//! use bevy::prelude::*;
//! use bevy_console::prelude::*;
//!
//! fn main() {
//!     App::new()
//!         .add_plugins(DefaultPlugins)
//!         .add_plugins(ConsolePlugin::default())
//!         .add_systems(Startup, setup_console)
//!         .run();
//! }
//!
//! fn setup_console(mut console: Console) {
//!     // Register a variable
//!     console.register_var(ConVar::new("sv_gravity", 800.0f32)
//!         .description("World gravity")
//!         .flags(ConVarFlags::ARCHIVE));
//!
//!     // Register a command
//!     console.register_cmd(ConCommand::new("noclip", |_, world| {
//!         info!("Noclip toggled!");
//!     }).description("Toggle noclip mode"));
//!
//!     // Get and set values
//!     let gravity: f32 = console.get("sv_gravity").unwrap();
//!     console.set("sv_gravity", 1000.0f32);
//! }
//! ```

use bevy::prelude::*;

// Core module (always available, zero optional deps)
pub mod core;

// Re-export core types at crate root for convenience
pub use core::{
    Console, ConsoleRef,
    ConVar, ConVarFlags, ConVarValue, ConVarDyn,
    ConCommand, CommandHandler, CommandArgs,
    ConsoleRegistry, ConEntry, ConVarMeta, CommandHandlers,
    Trie,
    subsequence_match, match_and_sort, MatchResult,
    tokenize, tokenize_string, split_commands, TokenizedCommand, TokenizeError,
    ConsoleInputEvent, ConsoleOutputEvent, ConsoleOutputLevel,
    ConVarChangedEvent, ConsoleToggleEvent, ConsoleClearEvent,
    ConsoleEventsPlugin,
    PermissionLevel, ConsolePermissions,
};


// UI modules (feature-gated)
#[cfg(feature = "egui")]
pub mod config;
#[cfg(feature = "egui")]
pub mod logging;
#[cfg(feature = "egui")]
pub mod ui;

// Terminal backend (feature-gated)
#[cfg(feature = "terminal")]
pub mod terminal;

// Persistence module (feature-gated)
#[cfg(feature = "persist")]
pub mod persist;

// Re-exports
#[cfg(feature = "egui")]
pub use config::{ConsoleConfig, ConsoleTheme};

#[cfg(feature = "persist")]
pub use persist::{ConsoleConfigFile, CommandAliases, ConfigPath, ConfigError};

#[cfg(feature = "terminal")]
pub use terminal::{TerminalPlugin, TerminalConfig};

/// Prelude for convenient imports.
pub mod prelude {
    pub use crate::core::{
        Console, ConsoleRef,
        ConVar, ConVarFlags, ConVarValue,
        ConCommand, CommandArgs,
        ConsoleRegistry, ConEntry,
        ConsoleInputEvent, ConsoleOutputEvent, ConsoleOutputLevel, ConVarChangedEvent,
        tokenize, split_commands,
        PermissionLevel, ConsolePermissions,
    };
    pub use crate::ConsolePlugin;
}

/// Main console plugin.
///
/// # Configuration
///
/// ```ignore
/// ConsolePlugin::default()
/// ```
#[derive(Default)]
pub struct ConsolePlugin;

impl Plugin for ConsolePlugin {
    fn build(&self, app: &mut App) {
        // Core: Always register the registry, handlers, and events
        app.init_resource::<ConsoleRegistry>()
            .init_resource::<CommandHandlers>()
            .init_resource::<PendingCommands>()
            .init_resource::<ConsolePermissions>()
            .add_plugins(core::ConsoleEventsPlugin);

        // Register built-in commands
        app.add_systems(Startup, register_builtin_commands);

        // Process console input events (three-stage pipeline)
        // 1. parse_console_input: Read input events, tokenize, queue commands
        // 2. execute_pending_commands: Execute commands with exclusive World access
        // 3. send_pending_outputs: Send output events
        app.add_systems(Update, (
            parse_console_input,
            execute_pending_commands,
            send_pending_outputs,
        ).chain());

        // Persistence (feature-gated)
        #[cfg(feature = "persist")]
        {
            app.init_resource::<persist::CommandAliases>()
                .init_resource::<persist::ConfigPath>()
                .add_systems(Startup, persist::load_config_on_startup.after(register_builtin_commands));
        }

        // egui UI (feature-gated)
        #[cfg(feature = "egui")]
        {
            use bevy_egui::EguiPrimaryContextPass;
            use config::ConsoleConfig;
            use ui::ConsoleUiState;

            app.init_resource::<ConsoleUiState>()
                .init_resource::<ConsoleConfig>()
                .init_resource::<ui::AutoCompletions>()
                .register_type::<ConsoleConfig>()
                .add_systems(
                    Update,
                    (
                        ui::read_logs,
                        ui::open_close_ui,
                        ui::update_completions,
                        ui::handle_clear,
                    ),
                )
                .add_systems(
                    EguiPrimaryContextPass,
                    ui::render_ui_system.run_if(|s: Res<ConsoleUiState>| s.open),
                );
        }

        // Terminal backend (feature-gated)
        #[cfg(feature = "terminal")]
        {
            app.add_plugins(terminal::TerminalPlugin);
        }
    }
}

/// Helper to register a command in both registry and handlers.
fn register_cmd(
    registry: &mut ConsoleRegistry,
    handlers: &mut CommandHandlers,
    cmd: ConCommand,
) {
    let (name, handler, autocomplete, _is_new) = registry.register_cmd(cmd);
    handlers.register(name, handler, autocomplete);
}

/// Register built-in console commands.
fn register_builtin_commands(
    mut registry: ResMut<ConsoleRegistry>,
    mut handlers: ResMut<CommandHandlers>,
) {
    // sv_cheats - Enable cheat-protected commands and variables
    registry.register_var(
        ConVar::new("sv_cheats", 0i32)
            .description("Enable cheat-protected commands and variables")
            .min(0)
            .max(1)
            .permission(PermissionLevel::Admin)
    );

    // help - Show help for a command or list all commands
    register_cmd(&mut registry, &mut handlers, ConCommand::new("help", |args, world| {
        let registry = world.resource::<ConsoleRegistry>();

        if let Some(name) = args.get(0) {
            // Show help for specific command/var
            if let Some(entry) = registry.get_entry(name) {
                let desc = entry.description();
                let desc = if desc.is_empty() { "No description" } else { desc };
                info!("{} - {}", name, desc);

                if let ConEntry::Var(meta) = entry {
                    info!("  Current: {}", meta.get_string());
                    info!("  Default: {}", meta.default_string());
                }
            } else {
                warn!("Unknown command or variable: {}", name);
            }
        } else {
            // List all commands
            info!("Commands:");
            for (name, _) in registry.cmds() {
                info!("  {}", name);
            }
            info!("Use 'help <name>' for details, 'cvarlist' for variables");
        }
    }).description("Show help for a command or list all commands"));

    // cvarlist - List all console variables
    register_cmd(&mut registry, &mut handlers, ConCommand::new("cvarlist", |args, world| {
        let registry = world.resource::<ConsoleRegistry>();
        let prefix = args.get(0).unwrap_or("");

        let mut count = 0;
        for (name, meta) in registry.vars() {
            if name.starts_with(prefix) && !meta.flags.contains(ConVarFlags::HIDDEN) {
                let modified = if meta.is_modified() { "*" } else { "" };
                info!("{}{} = \"{}\"", name, modified, meta.get_string());
                count += 1;
            }
        }
        info!("{} convars", count);
    }).description("List console variables"));

    // find - Search commands and variables
    register_cmd(&mut registry, &mut handlers, ConCommand::new("find", |args, world| {
        let registry = world.resource::<ConsoleRegistry>();

        if let Some(query) = args.get(0) {
            let results = registry.search(query);
            let count = results.len();
            for (name, entry) in results {
                let kind = if entry.is_var() { "var" } else { "cmd" };
                let desc = entry.description();
                if desc.is_empty() {
                    info!("[{}] {}", kind, name);
                } else {
                    info!("[{}] {} - {}", kind, name, desc);
                }
            }
            info!("{} results", count);
        } else {
            warn!("Usage: find <search term>");
        }
    }).description("Search commands and variables by name or description"));

    // echo - Print text to console
    register_cmd(&mut registry, &mut handlers, ConCommand::new("echo", |args, _world| {
        info!("{}", args.join(" "));
    }).description("Print text to console"));

    // clear - Clear console output
    register_cmd(&mut registry, &mut handlers, ConCommand::new("clear", |_args, world| {
        let mut pending = world.resource_mut::<PendingCommands>();
        pending.clear_console = true;
    }).description("Clear console output"));

    // quit - Exit the application immediately
    register_cmd(&mut registry, &mut handlers, ConCommand::new("quit", |_args, _world| {
        std::process::exit(0);
    }).description("Exit the application"));

    // toggle - Toggle a boolean convar
    register_cmd(&mut registry, &mut handlers, ConCommand::new("toggle", |args, world| {
        if let Some(name) = args.get(0) {
            let mut registry = world.resource_mut::<ConsoleRegistry>();

            if let Some(current) = registry.get::<bool>(name) {
                registry.set(name, !current);
                info!("{} = {}", name, if !current { "1" } else { "0" });
            } else if let Some(current) = registry.get::<i32>(name) {
                let new_val = if current == 0 { 1 } else { 0 };
                registry.set(name, new_val);
                info!("{} = {}", name, new_val);
            } else {
                warn!("Cannot toggle '{}': not a boolean or integer", name);
            }
        } else {
            warn!("Usage: toggle <convar>");
        }
    }).description("Toggle a boolean convar"));

    // reset - Reset a convar to default
    register_cmd(&mut registry, &mut handlers, ConCommand::new("reset", |args, world| {
        if let Some(name) = args.get(0) {
            let mut registry = world.resource_mut::<ConsoleRegistry>();

            if let Some(ConEntry::Var(meta)) = registry.get_entry_mut(name) {
                meta.reset();
                info!("{} reset to \"{}\"", name, meta.get_string());
            } else {
                warn!("Unknown variable: {}", name);
            }
        } else {
            warn!("Usage: reset <convar>");
        }
    }).description("Reset a convar to its default value"));

    // differences - Show convars that differ from default
    register_cmd(&mut registry, &mut handlers, ConCommand::new("differences", |_args, world| {
        let registry = world.resource::<ConsoleRegistry>();

        let mut count = 0;
        for (name, meta) in registry.modified_vars() {
            info!("{} = \"{}\" (default: \"{}\")",
                name, meta.get_string(), meta.default_string());
            count += 1;
        }

        if count == 0 {
            info!("No modified convars");
        } else {
            info!("{} modified convars", count);
        }
    }).description("Show convars with non-default values"));

    // Persistence commands (only with persist feature)
    #[cfg(feature = "persist")]
    register_persist_commands(&mut registry, &mut handlers);
}

/// Register persistence-related commands.
#[cfg(feature = "persist")]
fn register_persist_commands(
    registry: &mut ConsoleRegistry,
    handlers: &mut CommandHandlers,
) {
    // exec - Execute commands from a file
    register_cmd(registry, handlers, ConCommand::new("exec", |args, world| {
        if let Some(filename) = args.get(0) {
            // We need to queue the commands, not execute them directly
            // So we'll read the file and send input events
            let path = std::path::Path::new(filename);

            match std::fs::read_to_string(path) {
                Ok(contents) => {
                    info!("Executing '{}'...", filename);
                    let mut count = 0;

                    // Queue each line as a command
                    for line in contents.lines() {
                        let line = line.trim();
                        if line.is_empty() || line.starts_with("//") || line.starts_with('#') {
                            continue;
                        }

                        // Queue the command through pending
                        let mut pending = world.resource_mut::<PendingCommands>();
                        pending.outputs.push(ConsoleOutputEvent::command(format!("$ {}", line)));

                        if let Ok(tokens) = tokenize(line) {
                            pending.queue.push(QueuedCommand {
                                raw: line.to_string(),
                                name: tokens.command.to_string(),
                                args: tokens.args.iter().map(|s| s.to_string()).collect(),
                            });
                            count += 1;
                        }
                    }

                    info!("Queued {} commands from '{}'", count, filename);
                }
                Err(e) => {
                    error!("Failed to read '{}': {}", filename, e);
                }
            }
        } else {
            warn!("Usage: exec <filename>");
        }
    }).description("Execute commands from a file"));

    // host_writeconfig - Save ARCHIVE convars to file
    register_cmd(registry, handlers, ConCommand::new("host_writeconfig", |args, world| {
        let config_path = world.resource::<persist::ConfigPath>();
        let filename = args.get(0).unwrap_or(&config_path.0);

        let registry = world.resource::<ConsoleRegistry>();
        let aliases = world.resource::<persist::CommandAliases>();

        match persist::save_config(&registry, &aliases, filename) {
            Ok(()) => {
                info!("Saved config to '{}'", filename);
            }
            Err(e) => {
                error!("Failed to save config: {}", e);
            }
        }
    }).description("Save ARCHIVE convars to config file"));

    // alias - Create or list command aliases
    register_cmd(registry, handlers, ConCommand::new("alias", |args, world| {
        let mut aliases = world.resource_mut::<persist::CommandAliases>();

        match (args.get(0), args.get(1)) {
            (None, None) => {
                // List all aliases
                if aliases.is_empty() {
                    info!("No aliases defined");
                } else {
                    info!("Aliases:");
                    for (name, command) in aliases.iter() {
                        info!("  {} -> {}", name, command);
                    }
                }
            }
            (Some(name), None) => {
                // Show specific alias
                if let Some(command) = aliases.get(name) {
                    info!("{} -> {}", name, command);
                } else {
                    warn!("Alias '{}' not found", name);
                }
            }
            (Some(name), Some(_)) => {
                // Create alias (join remaining args as the command)
                let command = args.join_from(1, " ");
                aliases.add(name.to_string(), command.clone());
                info!("Alias '{}' set to '{}'", name, command);
            }
            (None, Some(_)) => unreachable!(),
        }
    }).description("Create or list command aliases"));

    // unalias - Remove a command alias
    register_cmd(registry, handlers, ConCommand::new("unalias", |args, world| {
        if let Some(name) = args.get(0) {
            let mut aliases = world.resource_mut::<persist::CommandAliases>();

            if aliases.remove(name).is_some() {
                info!("Removed alias '{}'", name);
            } else {
                warn!("Alias '{}' not found", name);
            }
        } else {
            warn!("Usage: unalias <name>");
        }
    }).description("Remove a command alias"));
}

/// Queued command for execution.
#[derive(Debug, Clone)]
struct QueuedCommand {
    /// Raw command string for display.
    raw: String,
    /// Command/variable name.
    name: String,
    /// Arguments.
    args: Vec<String>,
}

/// Resource that holds pending command executions.
#[derive(Resource, Default)]
struct PendingCommands {
    queue: Vec<QueuedCommand>,
    outputs: Vec<ConsoleOutputEvent>,
    changes: Vec<ConVarChangedEvent>,
    clear_console: bool,
}

/// System that parses console input and queues commands for execution.
fn parse_console_input(
    mut input_events: MessageReader<ConsoleInputEvent>,
    mut pending: ResMut<PendingCommands>,
) {
    for event in input_events.read() {
        // Split by semicolons for multiple commands
        let commands = split_commands(&event.command);

        for cmd_str in commands {
            // Echo the command
            pending.outputs.push(ConsoleOutputEvent::command(format!("$ {}", cmd_str)));

            // Tokenize
            let tokens = match tokenize(cmd_str) {
                Ok(t) => t,
                Err(e) => {
                    pending.outputs.push(ConsoleOutputEvent::error(format!("Parse error: {}", e)));
                    continue;
                }
            };

            pending.queue.push(QueuedCommand {
                raw: cmd_str.to_string(),
                name: tokens.command.to_string(),
                args: tokens.args.iter().map(|s| s.to_string()).collect(),
            });
        }
    }
}

/// Check if access is permitted based on flags and permission level.
///
/// Checks:
/// 1. If CHEAT flag is set, `sv_cheats` must be enabled
/// 2. Current permission level must be >= required level
fn check_access(
    world: &World,
    flags: ConVarFlags,
    required_permission: PermissionLevel,
) -> Result<(), String> {
    // Check CHEAT flag
    if flags.contains(ConVarFlags::CHEAT) {
        let registry = world.resource::<ConsoleRegistry>();
        if registry.get::<i32>("sv_cheats").unwrap_or(0) == 0 {
            return Err("Requires sv_cheats to be enabled".into());
        }
    }

    // Check permission level
    let perms = world.resource::<ConsolePermissions>();
    if !perms.has_permission(required_permission) {
        return Err(format!(
            "Insufficient permission (requires {}, have {})",
            required_permission.name(),
            perms.current_level.name()
        ));
    }

    Ok(())
}

/// Exclusive system that executes queued commands with full World access.
fn execute_pending_commands(world: &mut World) {
    // Take the pending commands
    let mut pending = world.resource_mut::<PendingCommands>();
    let queue = std::mem::take(&mut pending.queue);
    let mut outputs = std::mem::take(&mut pending.outputs);
    let mut changes = std::mem::take(&mut pending.changes);
    drop(pending);

    if queue.is_empty() && outputs.is_empty() {
        return;
    }

    for cmd in queue {
        // First, check what type of entry this is and get access info (borrow registry briefly)
        let entry_info = {
            let registry = world.resource::<ConsoleRegistry>();
            match registry.get_entry(&cmd.name) {
                Some(ConEntry::Cmd(meta)) => Some((
                    true,  // is_command
                    meta.flags,
                    meta.required_permission,
                )),
                Some(ConEntry::Var(meta)) => Some((
                    false, // is_command
                    meta.flags,
                    meta.required_permission,
                )),
                None => None,
            }
        };

        match entry_info {
            Some((true, flags, required_permission)) => {
                // It's a command - check access first
                if let Err(msg) = check_access(world, flags, required_permission) {
                    outputs.push(ConsoleOutputEvent::error(
                        format!("Cannot execute '{}': {}", cmd.name, msg)
                    ));
                    continue;
                }

                // Get handler from CommandHandlers and execute
                // Use resource_scope to take CommandHandlers temporarily
                let cmd_name_for_panic = cmd.name.clone();
                let panic_result = world.resource_scope(|world, mut handlers: Mut<CommandHandlers>| {
                    // Take the handler out temporarily
                    if let Some(handler) = handlers.take(&cmd.name) {
                        let args_refs: Vec<&str> = cmd.args.iter().map(|s| s.as_str()).collect();
                        let cmd_args = CommandArgs::new(&cmd.raw, args_refs);

                        // Execute with panic safety - always restore handler even if panic occurs
                        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                            handler(&cmd_args, world);
                        }));

                        // Always put the handler back, regardless of panic
                        handlers.put(&cmd.name, handler);

                        // Return panic info if one occurred
                        if let Err(panic_info) = result {
                            let panic_msg = if let Some(s) = panic_info.downcast_ref::<&str>() {
                                s.to_string()
                            } else if let Some(s) = panic_info.downcast_ref::<String>() {
                                s.clone()
                            } else {
                                "Unknown panic".to_string()
                            };
                            return Some(panic_msg);
                        }
                    }
                    None
                });

                // Log panic outside resource_scope so we can add to outputs
                if let Some(panic_msg) = panic_result {
                    outputs.push(ConsoleOutputEvent::error(
                        format!("Command '{}' panicked: {}", cmd_name_for_panic, panic_msg)
                    ));
                }
            }
            Some((false, flags, required_permission)) => {
                // It's a variable - handle get/set
                if cmd.args.is_empty() {
                    // Get variable (no access check needed for reading)
                    let registry = world.resource::<ConsoleRegistry>();
                    if let Some(ConEntry::Var(meta)) = registry.get_entry(&cmd.name) {
                        let value = meta.get_string();
                        let desc = meta.description;
                        outputs.push(ConsoleOutputEvent::result(
                            format!("\"{}\" = \"{}\"", cmd.name, value)
                        ));
                        if !desc.is_empty() {
                            outputs.push(ConsoleOutputEvent::info(
                                format!(" - {}", desc)
                            ));
                        }
                    }
                } else {
                    // Set variable - check access first
                    if let Err(msg) = check_access(world, flags, required_permission) {
                        outputs.push(ConsoleOutputEvent::error(
                            format!("Cannot set '{}': {}", cmd.name, msg)
                        ));
                        continue;
                    }

                    // Re-borrow registry for the actual set
                    let mut registry = world.resource_mut::<ConsoleRegistry>();
                    let old_value = registry.get_string(&cmd.name).unwrap_or_default();
                    let new_value = cmd.args.join(" ");

                    if let Some(ConEntry::Var(meta)) = registry.get_entry_mut(&cmd.name) {
                        if meta.set_string(&new_value) {
                            let actual_new = meta.get_string();
                            outputs.push(ConsoleOutputEvent::result(
                                format!("\"{}\" = \"{}\"", cmd.name, actual_new)
                            ));

                            // Queue change event
                            changes.push(ConVarChangedEvent::new(
                                cmd.name.clone(),
                                old_value,
                                actual_new,
                            ));
                        } else {
                            outputs.push(ConsoleOutputEvent::error(
                                format!("Cannot set '{}': invalid value or read-only", cmd.name)
                            ));
                        }
                    }
                }
            }
            None => {
                // Check if it's an alias (only with persist feature)
                #[cfg(feature = "persist")]
                {
                    let alias_cmd = {
                        let aliases = world.resource::<persist::CommandAliases>();
                        aliases.get(&cmd.name).map(|s| s.to_string())
                    };

                    if let Some(alias_expansion) = alias_cmd {
                        // Expand the alias: replace the alias name with its expansion
                        // and append any additional arguments
                        let expanded = if cmd.args.is_empty() {
                            alias_expansion
                        } else {
                            format!("{} {}", alias_expansion, cmd.args.join(" "))
                        };

                        // Queue the expanded command
                        if let Ok(tokens) = tokenize(&expanded) {
                            let mut pending = world.resource_mut::<PendingCommands>();
                            pending.queue.push(QueuedCommand {
                                raw: expanded.clone(),
                                name: tokens.command.to_string(),
                                args: tokens.args.iter().map(|s| s.to_string()).collect(),
                            });
                        }
                        continue;
                    }
                }

                outputs.push(ConsoleOutputEvent::error(
                    format!("Unknown command or variable: '{}'", cmd.name)
                ));
            }
        }
    }

    // Store outputs and changes back for the next system to send
    let mut pending = world.resource_mut::<PendingCommands>();
    pending.outputs = outputs;
    pending.changes = changes;
}

/// System that sends queued output events.
fn send_pending_outputs(
    mut pending: ResMut<PendingCommands>,
    mut output_events: MessageWriter<ConsoleOutputEvent>,
    mut change_events: MessageWriter<ConVarChangedEvent>,
    mut clear_events: MessageWriter<ConsoleClearEvent>,
) {
    for output in pending.outputs.drain(..) {
        output_events.write(output);
    }
    for change in pending.changes.drain(..) {
        change_events.write(change);
    }
    if pending.clear_console {
        pending.clear_console = false;
        clear_events.write(ConsoleClearEvent);
    }
}


// Integration tests run without egui feature since MinimalPlugins doesn't provide
// the resources that egui UI systems require (ButtonInput, etc.)
// Run with: cargo test --no-default-features
#[cfg(all(test, not(feature = "egui")))]
mod tests {
    use super::*;

    /// Test resource to track command execution.
    #[derive(Resource, Default)]
    struct TestCommandExecuted {
        count: usize,
        last_args: Vec<String>,
    }

    /// Helper to queue a command directly for testing.
    fn queue_command(app: &mut App, cmd: &str) {
        // Parse the command and add to pending queue
        let commands = split_commands(cmd);
        for cmd_str in commands {
            let tokens = tokenize(cmd_str).expect("Failed to tokenize test command");
            let mut pending = app.world_mut().resource_mut::<PendingCommands>();
            pending.outputs.push(ConsoleOutputEvent::command(format!("$ {}", cmd_str)));
            pending.queue.push(QueuedCommand {
                raw: cmd_str.to_string(),
                name: tokens.command.to_string(),
                args: tokens.args.iter().map(|s| s.to_string()).collect(),
            });
        }
    }

    #[test]
    fn test_command_execution() {
        let mut app = App::new();

        // Add minimal plugins needed for the test
        app.add_plugins(MinimalPlugins);
        app.add_plugins(ConsolePlugin);

        // Register a test command that increments a counter
        app.init_resource::<TestCommandExecuted>();

        // Register the command in a startup system
        app.add_systems(Startup, |mut registry: ResMut<ConsoleRegistry>, mut handlers: ResMut<CommandHandlers>| {
            register_cmd(&mut registry, &mut handlers,
                ConCommand::new("test_cmd", |args, world| {
                    let mut tracker = world.resource_mut::<TestCommandExecuted>();
                    tracker.count += 1;
                    tracker.last_args = args.iter().map(|s| s.to_string()).collect();
                })
                .description("Test command")
            );
        });

        // Run startup
        app.update();

        // Queue a command directly
        queue_command(&mut app, "test_cmd arg1 arg2");

        // Run the update loop to process the command
        app.update();

        // Verify the command was executed
        let tracker = app.world().resource::<TestCommandExecuted>();
        assert_eq!(tracker.count, 1, "Command should have been executed once");
        assert_eq!(tracker.last_args, vec!["arg1", "arg2"]);
    }

    #[test]
    fn test_convar_get_set_via_input() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_plugins(ConsolePlugin);

        // Register a test variable
        app.add_systems(Startup, |mut registry: ResMut<ConsoleRegistry>| {
            registry.register_var(
                ConVar::new("test_var", 42i32)
                    .description("Test variable")
            );
        });

        // Run startup
        app.update();

        // Verify initial value
        {
            let registry = app.world().resource::<ConsoleRegistry>();
            assert_eq!(registry.get::<i32>("test_var"), Some(42));
        }

        // Set variable via queued command
        queue_command(&mut app, "test_var 100");

        // Run update to process the command
        app.update();

        // Verify the value changed
        {
            let registry = app.world().resource::<ConsoleRegistry>();
            assert_eq!(registry.get::<i32>("test_var"), Some(100));
        }
    }

    #[test]
    fn test_builtin_echo_command() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_plugins(ConsolePlugin);

        // Run startup (registers built-in commands)
        app.update();

        // Queue echo command
        queue_command(&mut app, "echo hello world");

        // Run update to process the command
        app.update();

        // The echo command uses info!() macro, which we can't easily test
        // but if we got here without panicking, the command executed
    }

    #[test]
    fn test_multiple_commands_semicolon() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_plugins(ConsolePlugin);
        app.init_resource::<TestCommandExecuted>();

        app.add_systems(Startup, |mut registry: ResMut<ConsoleRegistry>, mut handlers: ResMut<CommandHandlers>| {
            register_cmd(&mut registry, &mut handlers,
                ConCommand::new("inc", |_args, world| {
                    let mut tracker = world.resource_mut::<TestCommandExecuted>();
                    tracker.count += 1;
                })
            );
        });

        // Run startup
        app.update();

        // Queue multiple commands separated by semicolons
        queue_command(&mut app, "inc; inc; inc");

        // Run update
        app.update();

        // Verify all three commands executed
        let tracker = app.world().resource::<TestCommandExecuted>();
        assert_eq!(tracker.count, 3, "All three commands should have executed");
    }

    #[test]
    fn test_convar_changed_event() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_plugins(ConsolePlugin);

        app.add_systems(Startup, |mut registry: ResMut<ConsoleRegistry>| {
            registry.register_var(ConVar::new("test_var", 10i32));
        });

        // Run startup
        app.update();

        // Set variable via queued command
        queue_command(&mut app, "test_var 20");

        // Run update
        app.update();

        // Check the pending changes (they're queued in outputs)
        let pending = app.world().resource::<PendingCommands>();
        // Changes should have been sent and cleared by send_pending_outputs
        assert!(pending.changes.is_empty(), "Changes should have been sent");
    }

    #[test]
    fn test_builtin_help_command() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_plugins(ConsolePlugin);

        // Run startup (registers built-in commands)
        app.update();

        // Queue help command
        queue_command(&mut app, "help");

        // Run update to process the command
        app.update();

        // The help command uses info!() macro, which we can't easily test
        // but if we got here without panicking, the command executed
    }

    #[test]
    fn test_builtin_toggle_command() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_plugins(ConsolePlugin);

        app.add_systems(Startup, |mut registry: ResMut<ConsoleRegistry>| {
            registry.register_var(ConVar::new("test_bool", false));
        });

        // Run startup
        app.update();

        // Verify initial value
        {
            let registry = app.world().resource::<ConsoleRegistry>();
            assert_eq!(registry.get::<bool>("test_bool"), Some(false));
        }

        // Toggle the variable
        queue_command(&mut app, "toggle test_bool");
        app.update();

        // Verify it toggled
        {
            let registry = app.world().resource::<ConsoleRegistry>();
            assert_eq!(registry.get::<bool>("test_bool"), Some(true));
        }

        // Toggle again
        queue_command(&mut app, "toggle test_bool");
        app.update();

        // Verify it toggled back
        {
            let registry = app.world().resource::<ConsoleRegistry>();
            assert_eq!(registry.get::<bool>("test_bool"), Some(false));
        }
    }

    #[test]
    fn test_builtin_reset_command() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_plugins(ConsolePlugin);

        app.add_systems(Startup, |mut registry: ResMut<ConsoleRegistry>| {
            registry.register_var(ConVar::new("test_var", 42i32));
        });

        // Run startup
        app.update();

        // Change the value
        queue_command(&mut app, "test_var 100");
        app.update();

        // Verify it changed
        {
            let registry = app.world().resource::<ConsoleRegistry>();
            assert_eq!(registry.get::<i32>("test_var"), Some(100));
        }

        // Reset to default
        queue_command(&mut app, "reset test_var");
        app.update();

        // Verify it reset
        {
            let registry = app.world().resource::<ConsoleRegistry>();
            assert_eq!(registry.get::<i32>("test_var"), Some(42));
        }
    }

    #[cfg(feature = "persist")]
    #[test]
    fn test_alias_expansion() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_plugins(ConsolePlugin);
        app.init_resource::<TestCommandExecuted>();

        app.add_systems(Startup, |mut registry: ResMut<ConsoleRegistry>, mut handlers: ResMut<CommandHandlers>| {
            register_cmd(&mut registry, &mut handlers,
                ConCommand::new("test_cmd", |args, world| {
                    let mut tracker = world.resource_mut::<TestCommandExecuted>();
                    tracker.count += 1;
                    tracker.last_args = args.iter().map(|s| s.to_string()).collect();
                })
            );
        });

        // Run startup
        app.update();

        // Create an alias
        {
            let mut aliases = app.world_mut().resource_mut::<persist::CommandAliases>();
            aliases.add("tc", "test_cmd");
        }

        // Use the alias
        queue_command(&mut app, "tc arg1 arg2");

        // Run update - alias should expand and execute test_cmd
        app.update();
        // Need another update because alias expansion queues the command
        app.update();

        // Verify the aliased command was executed with args
        let tracker = app.world().resource::<TestCommandExecuted>();
        assert_eq!(tracker.count, 1, "Aliased command should have executed");
        assert_eq!(tracker.last_args, vec!["arg1", "arg2"], "Args should be passed through");
    }

    #[cfg(feature = "persist")]
    #[test]
    fn test_alias_command() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_plugins(ConsolePlugin);

        // Run startup (registers built-in commands including alias)
        app.update();

        // Create an alias via the alias command
        queue_command(&mut app, "alias q quit");
        app.update();

        // Verify the alias was created
        {
            let aliases = app.world().resource::<persist::CommandAliases>();
            assert_eq!(aliases.get("q"), Some("quit"));
        }

        // Remove the alias
        queue_command(&mut app, "unalias q");
        app.update();

        // Verify the alias was removed
        {
            let aliases = app.world().resource::<persist::CommandAliases>();
            assert_eq!(aliases.get("q"), None);
        }
    }

    #[test]
    fn test_cheat_enforcement() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_plugins(ConsolePlugin);

        // Register a cheat-protected variable
        app.add_systems(Startup, |mut registry: ResMut<ConsoleRegistry>| {
            registry.register_var(
                ConVar::new("god_mode", false)
                    .flags(ConVarFlags::CHEAT)
                    .description("Enable god mode")
            );
        });

        // Run startup
        app.update();

        // Verify sv_cheats is 0 by default
        {
            let registry = app.world().resource::<ConsoleRegistry>();
            assert_eq!(registry.get::<i32>("sv_cheats"), Some(0));
        }

        // Try to set cheat variable without sv_cheats - should fail
        queue_command(&mut app, "god_mode 1");
        app.update();

        // Verify it was NOT set
        {
            let registry = app.world().resource::<ConsoleRegistry>();
            assert_eq!(registry.get::<bool>("god_mode"), Some(false));
        }

        // Enable sv_cheats (need Admin permission, but we're at Server level by default)
        queue_command(&mut app, "sv_cheats 1");
        app.update();

        // Verify sv_cheats is now 1
        {
            let registry = app.world().resource::<ConsoleRegistry>();
            assert_eq!(registry.get::<i32>("sv_cheats"), Some(1));
        }

        // Now try to set the cheat variable again - should succeed
        queue_command(&mut app, "god_mode 1");
        app.update();

        // Verify it WAS set
        {
            let registry = app.world().resource::<ConsoleRegistry>();
            assert_eq!(registry.get::<bool>("god_mode"), Some(true));
        }
    }

    #[test]
    fn test_permission_enforcement() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_plugins(ConsolePlugin);
        app.init_resource::<TestCommandExecuted>();

        // Register an admin-only command
        app.add_systems(Startup, |mut registry: ResMut<ConsoleRegistry>, mut handlers: ResMut<CommandHandlers>| {
            register_cmd(&mut registry, &mut handlers,
                ConCommand::new("admin_cmd", |_args, world| {
                    let mut tracker = world.resource_mut::<TestCommandExecuted>();
                    tracker.count += 1;
                })
                .permission(PermissionLevel::Admin)
                .description("Admin only command")
            );
        });

        // Run startup
        app.update();

        // Set permission level to User
        {
            let mut perms = app.world_mut().resource_mut::<ConsolePermissions>();
            perms.current_level = PermissionLevel::User;
        }

        // Try to execute admin command - should fail
        queue_command(&mut app, "admin_cmd");
        app.update();

        // Verify it was NOT executed
        {
            let tracker = app.world().resource::<TestCommandExecuted>();
            assert_eq!(tracker.count, 0);
        }

        // Set permission level to Admin
        {
            let mut perms = app.world_mut().resource_mut::<ConsolePermissions>();
            perms.current_level = PermissionLevel::Admin;
        }

        // Try to execute admin command again - should succeed
        queue_command(&mut app, "admin_cmd");
        app.update();

        // Verify it WAS executed
        {
            let tracker = app.world().resource::<TestCommandExecuted>();
            assert_eq!(tracker.count, 1);
        }
    }

    #[test]
    fn test_combined_cheat_and_permission() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_plugins(ConsolePlugin);
        app.init_resource::<TestCommandExecuted>();

        // Register a command that requires BOTH cheats AND admin permission
        app.add_systems(Startup, |mut registry: ResMut<ConsoleRegistry>, mut handlers: ResMut<CommandHandlers>| {
            register_cmd(&mut registry, &mut handlers,
                ConCommand::new("cheat_admin_cmd", |_args, world| {
                    let mut tracker = world.resource_mut::<TestCommandExecuted>();
                    tracker.count += 1;
                })
                .flags(ConVarFlags::CHEAT)
                .permission(PermissionLevel::Admin)
                .description("Requires both cheats and admin")
            );
        });

        // Run startup
        app.update();

        // Set permission level to Admin but sv_cheats is 0
        {
            let mut perms = app.world_mut().resource_mut::<ConsolePermissions>();
            perms.current_level = PermissionLevel::Admin;
        }

        // Try to execute - should fail (sv_cheats not enabled)
        queue_command(&mut app, "cheat_admin_cmd");
        app.update();

        {
            let tracker = app.world().resource::<TestCommandExecuted>();
            assert_eq!(tracker.count, 0);
        }

        // Enable sv_cheats
        queue_command(&mut app, "sv_cheats 1");
        app.update();

        // Now try again - should succeed
        queue_command(&mut app, "cheat_admin_cmd");
        app.update();

        {
            let tracker = app.world().resource::<TestCommandExecuted>();
            assert_eq!(tracker.count, 1);
        }
    }

    #[test]
    fn test_sv_cheats_requires_admin() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_plugins(ConsolePlugin);

        // Run startup
        app.update();

        // Set permission level to User
        {
            let mut perms = app.world_mut().resource_mut::<ConsolePermissions>();
            perms.current_level = PermissionLevel::User;
        }

        // Try to enable sv_cheats - should fail (requires Admin)
        queue_command(&mut app, "sv_cheats 1");
        app.update();

        // Verify sv_cheats is still 0
        {
            let registry = app.world().resource::<ConsoleRegistry>();
            assert_eq!(registry.get::<i32>("sv_cheats"), Some(0));
        }

        // Set permission level to Admin
        {
            let mut perms = app.world_mut().resource_mut::<ConsolePermissions>();
            perms.current_level = PermissionLevel::Admin;
        }

        // Now try to enable sv_cheats - should succeed
        queue_command(&mut app, "sv_cheats 1");
        app.update();

        // Verify sv_cheats is now 1
        {
            let registry = app.world().resource::<ConsoleRegistry>();
            assert_eq!(registry.get::<i32>("sv_cheats"), Some(1));
        }
    }
}
