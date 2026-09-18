//! UI-free typing-test core: modes, word generation, the engine state
//! machine, and result metrics.

pub mod engine;
pub mod generator;
pub mod metrics;
pub mod mode;

pub use engine::{Status, TestEngine, Word};
pub use generator::{Modifiers, RandomGenerator, WordGenerator};
pub use metrics::{CharCounts, Metrics};
pub use mode::Mode;
