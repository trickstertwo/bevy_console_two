//! Permission levels for console access control.
//!
//! Provides hierarchical permissions for commands and variables.

use bevy::prelude::*;

/// Permission level for console access control.
///
/// Levels are ordered from least to most permissive:
/// `User < Admin < Server`
///
/// A user with a given permission level can access anything
/// at that level or below.
///
/// # Design
///
/// Three levels suffice for a developer console:
/// - **User**: Remote players with restricted access
/// - **Admin**: Authenticated administrators who can enable cheats and configure
/// - **Server**: Local/trusted context with unrestricted access
///
/// Games needing finer-grained roles (e.g., moderators) should implement
/// their own authorization layer on top of these base levels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
#[repr(u8)]
pub enum PermissionLevel {
    /// Basic user - can access general commands and variables.
    #[default]
    User = 0,
    /// Administrator - can access admin commands, enable cheats.
    Admin = 1,
    /// Server - unrestricted access (local/single-player).
    Server = 2,
}

impl PermissionLevel {
    /// Get the display name for this permission level.
    pub fn name(&self) -> &'static str {
        match self {
            PermissionLevel::User => "User",
            PermissionLevel::Admin => "Admin",
            PermissionLevel::Server => "Server",
        }
    }
}

impl std::fmt::Display for PermissionLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

/// Resource tracking the current console permission level.
///
/// By default, this is set to `Server` (unrestricted) for local/single-player.
/// Multiplayer games should set this based on their authentication system.
///
/// # Examples
///
/// ```ignore
/// fn on_player_connect(mut perms: ResMut<ConsolePermissions>, auth: Res<AuthState>) {
///     perms.current_level = if auth.is_admin {
///         PermissionLevel::Admin
///     } else {
///         PermissionLevel::User
///     };
/// }
/// ```
#[derive(Resource, Debug, Clone)]
pub struct ConsolePermissions {
    /// The current permission level.
    pub current_level: PermissionLevel,
}

impl Default for ConsolePermissions {
    fn default() -> Self {
        // Default to Server (unrestricted) for backwards compatibility
        Self {
            current_level: PermissionLevel::Server,
        }
    }
}

impl ConsolePermissions {
    /// Create new permissions with the specified level.
    pub fn new(level: PermissionLevel) -> Self {
        Self {
            current_level: level,
        }
    }

    /// Check if the current level has permission for the required level.
    ///
    /// Returns `true` if `current_level >= required`.
    #[inline]
    pub fn has_permission(&self, required: PermissionLevel) -> bool {
        self.current_level >= required
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_permission_ordering() {
        assert!(PermissionLevel::User < PermissionLevel::Admin);
        assert!(PermissionLevel::Admin < PermissionLevel::Server);
    }

    #[test]
    fn test_has_permission() {
        let perms = ConsolePermissions::new(PermissionLevel::Admin);

        assert!(perms.has_permission(PermissionLevel::User));
        assert!(perms.has_permission(PermissionLevel::Admin));
        assert!(!perms.has_permission(PermissionLevel::Server));
    }

    #[test]
    fn test_default_is_server() {
        let perms = ConsolePermissions::default();
        assert_eq!(perms.current_level, PermissionLevel::Server);
    }

    #[test]
    fn test_permission_name() {
        assert_eq!(PermissionLevel::User.name(), "User");
        assert_eq!(PermissionLevel::Admin.name(), "Admin");
        assert_eq!(PermissionLevel::Server.name(), "Server");
    }
}
