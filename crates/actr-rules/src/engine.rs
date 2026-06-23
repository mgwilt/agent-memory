use std::cmp::Ordering;

use actr_session::BufferSnapshot;

use crate::rule::{ProductionRule, RuleId};

#[derive(Debug, Clone, PartialEq)]
pub struct RuleMatch {
    pub rule_id: RuleId,
    pub name: String,
    pub utility: f64,
    pub specificity: usize,
}

#[derive(Debug, Default, Clone)]
pub struct RuleEngine {
    rules: Vec<ProductionRule>,
}

impl RuleEngine {
    pub fn new(rules: Vec<ProductionRule>) -> Self {
        Self { rules }
    }

    pub fn matching_rules(&self, buffers: &[BufferSnapshot]) -> Vec<RuleMatch> {
        let mut matches = self
            .rules
            .iter()
            .filter(|rule| rule.enabled)
            .filter(|rule| {
                rule.conditions
                    .iter()
                    .all(|condition| condition.matches(buffers))
            })
            .map(|rule| RuleMatch {
                rule_id: rule.rule_id.clone(),
                name: rule.name.clone(),
                utility: rule.utility,
                specificity: rule.specificity(),
            })
            .collect::<Vec<_>>();

        matches.sort_by(|left, right| {
            right
                .specificity
                .cmp(&left.specificity)
                .then_with(|| {
                    right
                        .utility
                        .partial_cmp(&left.utility)
                        .unwrap_or(Ordering::Equal)
                })
                .then_with(|| left.name.cmp(&right.name))
        });
        matches
    }

    pub fn choose_best(&self, buffers: &[BufferSnapshot]) -> Option<RuleMatch> {
        self.matching_rules(buffers).into_iter().next()
    }
}

#[cfg(test)]
mod tests {
    use actr_core::{AgentId, ChunkId, ChunkType};
    use actr_session::{BufferName, SessionState};

    use super::*;
    use crate::rule::{BufferCondition, ProductionRule, RuleId};

    #[test]
    fn chooses_more_specific_matching_rule_before_utility_tie_break() {
        let mut session = SessionState::new(AgentId("agent".to_string()));
        session.set_buffer(
            BufferName::Goal,
            ChunkId("goal-1".to_string()),
            ChunkType("goal".to_string()),
            1,
        );
        let engine = RuleEngine::new(vec![
            ProductionRule {
                rule_id: RuleId("generic".to_string()),
                name: "generic".to_string(),
                enabled: true,
                utility: 99.0,
                version: 1,
                conditions: vec![BufferCondition::buffer_present(BufferName::Goal)],
            },
            ProductionRule {
                rule_id: RuleId("specific".to_string()),
                name: "specific".to_string(),
                enabled: true,
                utility: 1.0,
                version: 1,
                conditions: vec![BufferCondition {
                    buffer: BufferName::Goal,
                    chunk_id: Some(ChunkId("goal-1".to_string())),
                    chunk_type: Some(ChunkType("goal".to_string())),
                }],
            },
        ]);

        let chosen = engine
            .choose_best(&session.snapshot())
            .expect("a rule should match");
        assert_eq!(chosen.rule_id, RuleId("specific".to_string()));
    }
}
