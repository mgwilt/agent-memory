#![forbid(unsafe_code)]

pub mod engine;
pub mod rule;
pub mod utility;

pub use engine::{
    ConflictResolution, RuleCandidateDiagnostic, RuleEngine, RuleEvaluationContext, RuleMatch,
    RuleRejectionReason, RuleSelectionPolicy,
};
pub use rule::{
    BufferCondition, ProductionRule, ProductionRuleMetadata, RetrievedChunkCondition, RuleId,
};
pub use utility::{RuleRewardUpdate, apply_reward, apply_reward_to_rule, reward_update};
