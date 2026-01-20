pub mod config;
pub mod manager;
pub mod runner;

pub use config::{SubagentConfig, SubagentType};
pub use manager::SubagentManager;
pub use runner::{SubagentRunner, SubagentResult, SubagentError};
