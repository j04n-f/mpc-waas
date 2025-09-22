use anyhow::Result;
use serde::Deserialize;
use std::env;
use thiserror::Error;

// =============================================================================
// Error Types
// =============================================================================

/// Configuration-specific error types
#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("Missing required environment variable: {0}")]
    MissingEnvVar(String),
    #[error("Invalid value for environment variable {var}: {reason}")]
    InvalidEnvVar { var: String, reason: String },
}

// =============================================================================
// Configuration Structures
// =============================================================================

/// Main application configuration containing all subsystem configs
#[derive(Debug, Clone, Deserialize)]
pub struct AppConfig {
    /// HTTP server configuration
    pub server: ServerConfig,
    /// Database connection configuration
    pub database: DatabaseConfig,
    /// Multi-party computation participants configuration
    pub participants: ParticipantsConfig,
    /// Blockchain provider configuration
    pub provider: ProviderConfig,
}

/// HTTP server configuration
#[derive(Debug, Clone, Deserialize)]
pub struct ServerConfig {
    /// Server bind address (e.g., "127.0.0.1" or "0.0.0.0")
    pub host: String,
    /// Server port number
    pub port: u16,
}

/// Database connection configuration
#[derive(Debug, Clone, Deserialize)]
pub struct DatabaseConfig {
    /// Database connection URL (e.g., "postgresql://user:pass@host:port/db")
    pub url: String,
}

/// Individual participant configuration in MPC protocol
#[derive(Debug, Clone, Deserialize)]
pub struct ParticipantConfig {
    /// Participant service endpoint (e.g., "http://participant-1:50051")
    pub host: String,
}

/// Configuration for all MPC participants
///
/// Note: Currently hardcoded to 3 participants. Consider making this
/// more flexible in the future if participant count needs to be dynamic.
#[derive(Debug, Clone, Deserialize)]
pub struct ParticipantsConfig {
    pub participant_1: ParticipantConfig,
    pub participant_2: ParticipantConfig,
    pub participant_3: ParticipantConfig,
}

/// Blockchain provider configuration (e.g., Anvil, Ganache, or live network)
#[derive(Debug, Clone, Deserialize)]
pub struct ProviderConfig {
    /// Provider endpoint hostname or URL
    pub host: String,
    /// Provider RPC port
    pub port: u16,
}

// =============================================================================
// Implementation
// =============================================================================

impl AppConfig {
    /// Load configuration from environment variables
    ///
    /// This method reads configuration from environment variables with sensible
    /// defaults where appropriate. Required variables will cause an error if missing.
    ///
    /// # Environment Variables
    ///
    /// ## Server Configuration
    /// - `SERVER_HOST`: Server bind address (default: "127.0.0.1")
    /// - `SERVER_PORT`: Server port (default: "8000")
    ///
    /// ## Database Configuration
    /// - `DATABASE_URL`: Database connection URL (required)
    ///
    /// ## Participant Configuration
    /// - `PARTICIPANT_1_HOST`: Participant 1 endpoint (default: "http://participant-1:50051")
    /// - `PARTICIPANT_1_INDEX`: Participant 1 index (default: "1")
    /// - `PARTICIPANT_2_HOST`: Participant 2 endpoint (default: "http://participant-2:50052")
    /// - `PARTICIPANT_2_INDEX`: Participant 2 index (default: "2")
    /// - `PARTICIPANT_3_HOST`: Participant 3 endpoint (default: "http://participant-3:50053")
    /// - `PARTICIPANT_3_INDEX`: Participant 3 index (default: "3")
    ///
    /// ## Provider Configuration
    /// - `PROVIDER_HOST`: Blockchain provider host (default: "http://anvil")
    /// - `PROVIDER_PORT`: Blockchain provider port (default: "8545")
    ///
    /// # Errors
    ///
    /// Returns `ConfigError` if:
    /// - Required environment variables are missing
    /// - Environment variables contain invalid values (e.g., non-numeric ports)
    pub fn from_env() -> Result<Self> {
        Ok(AppConfig {
            server: Self::load_server_config()?,
            database: Self::load_database_config()?,
            participants: Self::load_participants_config()?,
            provider: Self::load_provider_config()?,
        })
    }

    /// Load server configuration from environment
    fn load_server_config() -> Result<ServerConfig> {
        let host = env::var("SERVER_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
        let port = Self::parse_port_env("SERVER_PORT", "8000")?;

        Ok(ServerConfig { host, port })
    }

    /// Load database configuration from environment
    fn load_database_config() -> Result<DatabaseConfig> {
        let url = env::var("DATABASE_URL").map_err(|_| {
            ConfigError::MissingEnvVar(
                "DATABASE_URL is required for database connection".to_string(),
            )
        })?;

        Ok(DatabaseConfig { url })
    }

    /// Load all participants configuration from environment
    fn load_participants_config() -> Result<ParticipantsConfig> {
        let participant_1 = Self::load_participant_config(1, "http://participant-1:50051")?;
        let participant_2 = Self::load_participant_config(2, "http://participant-2:50052")?;
        let participant_3 = Self::load_participant_config(3, "http://participant-3:50053")?;

        Ok(ParticipantsConfig {
            participant_1,
            participant_2,
            participant_3,
        })
    }

    /// Load individual participant configuration
    fn load_participant_config(
        participant_num: u8,
        default_host: &str,
    ) -> Result<ParticipantConfig> {
        let host_var = format!("PARTICIPANT_{}_HOST", participant_num);

        let host = env::var(&host_var).unwrap_or_else(|_| default_host.to_string());

        Ok(ParticipantConfig { host })
    }

    /// Load blockchain provider configuration from environment
    fn load_provider_config() -> Result<ProviderConfig> {
        let host = env::var("PROVIDER_HOST").unwrap_or_else(|_| "http://anvil".to_string());
        let port = Self::parse_port_env("PROVIDER_PORT", "8545")?;

        Ok(ProviderConfig { host, port })
    }

    /// Parse a port number from environment variable with default fallback
    fn parse_port_env(var_name: &str, default_value: &str) -> Result<u16> {
        Self::parse_u16_env(var_name, default_value)
    }

    /// Parse a u16 value from environment variable with default fallback
    fn parse_u16_env(var_name: &str, default_value: &str) -> Result<u16> {
        let value_str = env::var(var_name).unwrap_or_else(|_| default_value.to_string());

        let val = value_str.parse().map_err(|_| ConfigError::InvalidEnvVar {
            var: var_name.to_string(),
            reason: format!("expected a valid number, got '{}'", value_str),
        })?;

        Ok(val)
    }
}
