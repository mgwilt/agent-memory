#![forbid(unsafe_code)]

pub mod engine;
pub mod rule;
pub mod utility;

pub use engine::{RuleEngine, RuleMatch};
pub use rule::{BufferCondition, ProductionRule, RuleId};
pub use utility::apply_reward;
