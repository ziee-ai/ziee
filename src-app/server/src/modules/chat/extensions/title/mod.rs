// Title Generation Extension for Chat Module
//
// Automatically generates conversation titles using AI after the first message exchange.

mod title;
pub mod extension; // Auto-discovered by build script

pub use title::TitleGenerationExtension;
