//! Structured error types for the Minecraft server.
//!
//! `McError` covers all error categories across the codebase.
//! Use `McResult<T>` as the standard result type throughout the project.

use thiserror::Error;

/// Unified error type for all Minecraft server operations.
#[derive(Error, Debug)]
pub enum McError {
    /// Protocol errors (serialization, deserialization, invalid packets)
    #[error("protocol error: {0}")]
    Protocol(String),

    /// Network errors (connection, timeout, encryption)
    #[error("network error: {0}")]
    Network(String),

    /// World errors (chunk not found, generation failure, block out of bounds)
    #[error("world error: {0}")]
    World(String),

    /// Player errors (not found, inventory full, invalid action)
    #[error("player error: {0}")]
    Player(String),

    /// Persistence errors (database, file I/O, NBT parse)
    #[error("persistence error: {0}")]
    Persistence(String),

    /// Command errors (invalid syntax, permission denied)
    #[error("command error: {0}")]
    Command(String),

    /// Configuration errors (missing fields, invalid values)
    #[error("config error: {0}")]
    Config(String),

    /// Internal errors (unexpected state, bugs)
    #[error("internal error: {0}")]
    Internal(String),
}

impl McError {
    /// Create a protocol error from a string or error message.
    pub fn protocol(msg: impl Into<String>) -> Self {
        Self::Protocol(msg.into())
    }

    /// Create a network error.
    pub fn network(msg: impl Into<String>) -> Self {
        Self::Network(msg.into())
    }

    /// Create a world error.
    pub fn world(msg: impl Into<String>) -> Self {
        Self::World(msg.into())
    }

    /// Create a player error.
    pub fn player(msg: impl Into<String>) -> Self {
        Self::Player(msg.into())
    }

    /// Create a persistence error.
    pub fn persistence(msg: impl Into<String>) -> Self {
        Self::Persistence(msg.into())
    }
}

/// Standard result type for Minecraft server operations.
pub type McResult<T> = Result<T, McError>;

/// Convenience trait for converting Option to McResult.
pub trait McOptionExt<T> {
    fn ok_or_mc(self, category: &str, msg: impl Into<String>) -> McResult<T>;
}

impl<T> McOptionExt<T> for Option<T> {
    fn ok_or_mc(self, category: &str, msg: impl Into<String>) -> McResult<T> {
        self.ok_or_else(|| McError::Internal(format!("{category}: {}", msg.into())))
    }
}
