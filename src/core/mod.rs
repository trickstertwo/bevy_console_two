//! Core console types with zero optional dependencies.
//!
//! This module provides the fundamental building blocks:
//! - [`Console`] - Unified system parameter for convenient access
//! - [`ConVar`] - Console variables with typed values and constraints
//! - [`ConCommand`] - Console commands with handlers
//! - [`ConsoleRegistry`] - Central registry for all console entries
//! - [`Trie`] - Fast prefix lookup for autocomplete
//! - [`tokenize`] - Simple command tokenizer
//! - Events for communication between layers

mod convar;
mod concommand;
mod registry;
mod trie;
mod matcher;
mod tokenizer;
mod events;
mod permissions;
mod console;

pub use convar::{ConVar, ConVarFlags, ConVarValue, ConVarDyn};
pub use concommand::{ConCommand, ConCommandMeta, CommandHandler, CommandArgs};
pub use registry::{ConsoleRegistry, ConEntry, ConVarMeta, CommandHandlers};
pub use trie::Trie;
pub use matcher::{subsequence_match, match_and_sort, MatchResult};
pub use tokenizer::{tokenize, tokenize_string, split_commands, TokenizedCommand, TokenizeError};
pub use events::{
    ConsoleInputEvent, ConsoleOutputEvent, ConsoleOutputLevel,
    ConVarChangedEvent, ConsoleToggleEvent, ConsoleClearEvent,
    ConsoleEventsPlugin,
};
pub use permissions::{PermissionLevel, ConsolePermissions};
pub use console::{Console, ConsoleRef};
