//! Console variable (ConVar) implementation.
//!
//! ConVars are typed variables that can be modified via the console.
//! Inspired by the Source Engine ConVar system.

use std::any::Any;
use std::fmt::{self, Display};

use bevy::prelude::*;

use super::PermissionLevel;

/// Flags controlling ConVar behavior.
///
/// These match the Source Engine FCVAR_ flags where applicable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ConVarFlags(u32);

impl ConVarFlags {
    /// No flags set.
    pub const NONE: Self = Self(0);

    /// Value persists to config file (FCVAR_ARCHIVE).
    ///
    /// **Security note**: ARCHIVE values are stored in plaintext.
    /// Do not mark sensitive data (passwords, tokens) with this flag.
    pub const ARCHIVE: Self = Self(1 << 0);

    /// Requires sv_cheats to modify (FCVAR_CHEAT).
    pub const CHEAT: Self = Self(1 << 1);

    /// Cannot be modified at runtime (FCVAR_READONLY).
    pub const READ_ONLY: Self = Self(1 << 2);

    /// Hidden from autocomplete/listing (FCVAR_HIDDEN).
    pub const HIDDEN: Self = Self(1 << 3);

    /// Triggers notification on change (FCVAR_NOTIFY).
    pub const NOTIFY: Self = Self(1 << 4);

    /// Development only, stripped in release builds.
    pub const DEV_ONLY: Self = Self(1 << 5);

    /// Check if a flag is set.
    #[inline]
    pub const fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }

    /// Combine two flag sets.
    #[inline]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// Remove flags.
    #[inline]
    pub const fn difference(self, other: Self) -> Self {
        Self(self.0 & !other.0)
    }

    /// Check if no flags are set.
    #[inline]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }
}

impl std::ops::BitOr for ConVarFlags {
    type Output = Self;

    #[inline]
    fn bitor(self, rhs: Self) -> Self::Output {
        self.union(rhs)
    }
}

impl std::ops::BitOrAssign for ConVarFlags {
    #[inline]
    fn bitor_assign(&mut self, rhs: Self) {
        *self = self.union(rhs);
    }
}

/// Trait for types that can be stored in a ConVar.
///
/// Implemented for common types: `bool`, `i32`, `i64`, `f32`, `f64`, `String`.
pub trait ConVarValue: Clone + Send + Sync + 'static {
    /// Parse a value from a string.
    fn parse(s: &str) -> Option<Self>;

    /// Format the value as a string.
    fn format(&self) -> String;

    /// Clamp the value to min/max bounds if applicable.
    fn clamp(self, min: Option<&Self>, max: Option<&Self>) -> Self;

    /// Check if this type supports min/max constraints.
    fn supports_bounds() -> bool {
        false
    }
}

impl ConVarValue for bool {
    fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "true" | "1" | "yes" | "on" => Some(true),
            "false" | "0" | "no" | "off" => Some(false),
            _ => None,
        }
    }

    fn format(&self) -> String {
        if *self { "1".to_string() } else { "0".to_string() }
    }

    fn clamp(self, _min: Option<&Self>, _max: Option<&Self>) -> Self {
        self
    }
}

impl ConVarValue for i32 {
    fn parse(s: &str) -> Option<Self> {
        s.parse().ok()
    }

    fn format(&self) -> String {
        self.to_string()
    }

    fn clamp(self, min: Option<&Self>, max: Option<&Self>) -> Self {
        let mut v = self;
        if let Some(&min) = min {
            v = v.max(min);
        }
        if let Some(&max) = max {
            v = v.min(max);
        }
        v
    }

    fn supports_bounds() -> bool {
        true
    }
}

impl ConVarValue for i64 {
    fn parse(s: &str) -> Option<Self> {
        s.parse().ok()
    }

    fn format(&self) -> String {
        self.to_string()
    }

    fn clamp(self, min: Option<&Self>, max: Option<&Self>) -> Self {
        let mut v = self;
        if let Some(&min) = min {
            v = v.max(min);
        }
        if let Some(&max) = max {
            v = v.min(max);
        }
        v
    }

    fn supports_bounds() -> bool {
        true
    }
}

impl ConVarValue for f32 {
    fn parse(s: &str) -> Option<Self> {
        s.parse().ok()
    }

    fn format(&self) -> String {
        // Avoid unnecessary decimal places
        if self.fract() == 0.0 {
            format!("{:.0}", self)
        } else {
            format!("{}", self)
        }
    }

    fn clamp(self, min: Option<&Self>, max: Option<&Self>) -> Self {
        let mut v = self;
        if let Some(&min) = min {
            v = v.max(min);
        }
        if let Some(&max) = max {
            v = v.min(max);
        }
        v
    }

    fn supports_bounds() -> bool {
        true
    }
}

impl ConVarValue for f64 {
    fn parse(s: &str) -> Option<Self> {
        s.parse().ok()
    }

    fn format(&self) -> String {
        if self.fract() == 0.0 {
            format!("{:.0}", self)
        } else {
            format!("{}", self)
        }
    }

    fn clamp(self, min: Option<&Self>, max: Option<&Self>) -> Self {
        let mut v = self;
        if let Some(&min) = min {
            v = v.max(min);
        }
        if let Some(&max) = max {
            v = v.min(max);
        }
        v
    }

    fn supports_bounds() -> bool {
        true
    }
}

impl ConVarValue for String {
    fn parse(s: &str) -> Option<Self> {
        Some(s.to_string())
    }

    fn format(&self) -> String {
        self.clone()
    }

    fn clamp(self, _min: Option<&Self>, _max: Option<&Self>) -> Self {
        self
    }
}

/// Type-erased trait for ConVar storage.
///
/// This allows storing ConVars of different types in the same registry.
pub trait ConVarDyn: Send + Sync {
    /// Get the current value as a string.
    fn get_string(&self) -> String;

    /// Set the value from a string. Returns true if successful.
    fn set_string(&mut self, s: &str) -> bool;

    /// Get the default value as a string.
    fn default_string(&self) -> String;

    /// Reset to default value.
    fn reset(&mut self);

    /// Check if the current value differs from default.
    fn is_modified(&self) -> bool;

    /// Get as Any for downcasting.
    fn as_any(&self) -> &dyn Any;

    /// Get as mutable Any for downcasting.
    fn as_any_mut(&mut self) -> &mut dyn Any;

    /// Clone the value into a new box.
    fn clone_boxed(&self) -> Box<dyn ConVarDyn>;
}

/// A console variable with typed value and constraints.
///
/// # Examples
///
/// ```
/// use bevy_console::core::{ConVar, ConVarFlags};
///
/// // Create a simple convar
/// let mut gravity = ConVar::new("sv_gravity", 800.0f32)
///     .description("World gravity")
///     .flags(ConVarFlags::ARCHIVE);
///
/// // Set with clamping
/// gravity.set(10000.0);
/// assert_eq!(gravity.get(), 10000.0);
///
/// // With min/max constraints
/// let mut fov = ConVar::new("cl_fov", 90i32)
///     .min(60)
///     .max(120);
///
/// fov.set(150);
/// assert_eq!(fov.get(), 120); // Clamped to max
/// ```
#[derive(Clone)]
pub struct ConVar<T: ConVarValue> {
    name: Box<str>,
    value: T,
    default: T,
    flags: ConVarFlags,
    description: &'static str,
    min: Option<T>,
    max: Option<T>,
    required_permission: PermissionLevel,
}

impl<T: ConVarValue> ConVar<T> {
    /// Create a new ConVar with the given name and default value.
    pub fn new(name: impl Into<Box<str>>, default: T) -> Self {
        Self {
            name: name.into(),
            value: default.clone(),
            default,
            flags: ConVarFlags::NONE,
            description: "",
            min: None,
            max: None,
            required_permission: PermissionLevel::User,
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

    /// Set the minimum value.
    pub fn min(mut self, min: T) -> Self {
        self.min = Some(min);
        // Re-clamp current value
        self.value = self.value.clone().clamp(self.min.as_ref(), self.max.as_ref());
        self
    }

    /// Set the maximum value.
    pub fn max(mut self, max: T) -> Self {
        self.max = Some(max);
        // Re-clamp current value
        self.value = self.value.clone().clamp(self.min.as_ref(), self.max.as_ref());
        self
    }

    /// Set the required permission level.
    pub fn permission(mut self, level: PermissionLevel) -> Self {
        self.required_permission = level;
        self
    }

    /// Get the name.
    #[inline]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get the current value.
    #[inline]
    pub fn get(&self) -> T {
        self.value.clone()
    }

    /// Get a reference to the current value.
    #[inline]
    pub fn get_ref(&self) -> &T {
        &self.value
    }

    /// Set the value, applying constraints.
    ///
    /// Returns `false` if the ConVar is read-only.
    pub fn set(&mut self, value: T) -> bool {
        if self.flags.contains(ConVarFlags::READ_ONLY) {
            return false;
        }
        self.value = value.clamp(self.min.as_ref(), self.max.as_ref());
        true
    }

    /// Get the default value.
    #[inline]
    pub fn default_value(&self) -> &T {
        &self.default
    }

    /// Reset to the default value.
    pub fn reset(&mut self) {
        if !self.flags.contains(ConVarFlags::READ_ONLY) {
            self.value = self.default.clone();
        }
    }

    /// Check if the current value differs from default.
    #[inline]
    pub fn is_modified(&self) -> bool
    where
        T: PartialEq,
    {
        self.value != self.default
    }

    /// Get the flags.
    #[inline]
    pub fn get_flags(&self) -> ConVarFlags {
        self.flags
    }

    /// Get the description.
    #[inline]
    pub fn get_description(&self) -> &'static str {
        self.description
    }

    /// Check if this ConVar has min/max constraints.
    #[inline]
    pub fn has_bounds(&self) -> bool {
        self.min.is_some() || self.max.is_some()
    }

    /// Get the required permission level.
    #[inline]
    pub fn get_required_permission(&self) -> PermissionLevel {
        self.required_permission
    }
}

impl<T: ConVarValue + PartialEq> ConVarDyn for ConVar<T> {
    fn get_string(&self) -> String {
        self.value.format()
    }

    fn set_string(&mut self, s: &str) -> bool {
        if self.flags.contains(ConVarFlags::READ_ONLY) {
            return false;
        }
        if let Some(value) = T::parse(s) {
            self.value = value.clamp(self.min.as_ref(), self.max.as_ref());
            true
        } else {
            false
        }
    }

    fn default_string(&self) -> String {
        self.default.format()
    }

    fn reset(&mut self) {
        ConVar::reset(self);
    }

    fn is_modified(&self) -> bool {
        self.value != self.default
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn clone_boxed(&self) -> Box<dyn ConVarDyn> {
        Box::new(self.clone())
    }
}

impl<T: ConVarValue> Display for ConVar<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "\"{}\" = \"{}\"",
            self.name,
            self.value.format()
        )?;
        if !self.description.is_empty() {
            write!(f, " - {}", self.description)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_convar_basic() {
        let cvar = ConVar::new("test", 42i32);
        assert_eq!(cvar.get(), 42);
        assert_eq!(cvar.name(), "test");
    }

    #[test]
    fn test_convar_set() {
        let mut cvar = ConVar::new("test", 42i32);
        assert!(cvar.set(100));
        assert_eq!(cvar.get(), 100);
    }

    #[test]
    fn test_convar_clamping() {
        let mut cvar = ConVar::new("test", 50i32).min(0).max(100);

        cvar.set(150);
        assert_eq!(cvar.get(), 100);

        cvar.set(-50);
        assert_eq!(cvar.get(), 0);

        cvar.set(50);
        assert_eq!(cvar.get(), 50);
    }

    #[test]
    fn test_convar_readonly() {
        let mut cvar = ConVar::new("test", 42i32).flags(ConVarFlags::READ_ONLY);
        assert!(!cvar.set(100));
        assert_eq!(cvar.get(), 42);
    }

    #[test]
    fn test_convar_reset() {
        let mut cvar = ConVar::new("test", 42i32);
        cvar.set(100);
        assert!(cvar.is_modified());

        cvar.reset();
        assert_eq!(cvar.get(), 42);
        assert!(!cvar.is_modified());
    }

    #[test]
    fn test_convar_bool() {
        let mut cvar = ConVar::new("enabled", false);

        assert!(bool::parse("true").unwrap());
        assert!(bool::parse("1").unwrap());
        assert!(bool::parse("yes").unwrap());
        assert!(!bool::parse("false").unwrap());
        assert!(!bool::parse("0").unwrap());

        cvar.set(true);
        assert_eq!(cvar.get_string(), "1");
    }

    #[test]
    fn test_convar_float() {
        let cvar = ConVar::new("gravity", 800.0f32);
        assert_eq!(cvar.get_string(), "800");

        let cvar = ConVar::new("ratio", 0.5f32);
        assert_eq!(cvar.get_string(), "0.5");
    }

    #[test]
    fn test_convar_dyn() {
        let cvar: Box<dyn ConVarDyn> = Box::new(ConVar::new("test", 42i32));
        assert_eq!(cvar.get_string(), "42");
        assert_eq!(cvar.default_string(), "42");
    }

    #[test]
    fn test_convar_flags() {
        let flags = ConVarFlags::ARCHIVE | ConVarFlags::NOTIFY;
        assert!(flags.contains(ConVarFlags::ARCHIVE));
        assert!(flags.contains(ConVarFlags::NOTIFY));
        assert!(!flags.contains(ConVarFlags::CHEAT));
    }
}
