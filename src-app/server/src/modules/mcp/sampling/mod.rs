// MCP Sampling module
// Implements the platform side of the MCP sampling protocol:
// https://spec.modelcontextprotocol.io/specification/client/sampling/

pub mod handler;
pub mod models;
pub mod session_counter;

pub use handler::{ChatSamplingHandler, SamplingHandler};
pub use session_counter::acquire_session;
