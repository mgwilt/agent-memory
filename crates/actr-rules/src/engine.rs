use actr_core::Chunk;
use actr_session::BufferSnapshot;

use crate::rule::{ProductionRule, RuleId};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuleRejectionReason {
    Disabled,
    NonFiniteUtility,
    BufferConditionsNotMet,
    RetrievedChunkConditionNotMet,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RuleMatch {
    pub rule_id: RuleId,
    pub name: String,
    pub utility: f64,
    pub specificity: usize,
    pub version: u64,
    pub rank: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RuleCandidateDiagnostic {
    pub rule_id: RuleId,
    pub name: String,
    pub enabled: bool,
    pub utility: f64,
    pub specificity: usize,
    pub version: u64,
    pub matched: bool,
    pub rank: Option<usize>,
    pub rejection_reason: Option<RuleRejectionReason>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ConflictResolution {
    pub candidates: Vec<RuleCandidateDiagnostic>,
    pub matches: Vec<RuleMatch>,
    pub selected: Option<RuleMatch>,
}

impl ConflictResolution {
    pub fn no_match(&self) -> bool {
        self.selected.is_none()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct RuleEvaluationContext<'a> {
    pub buffers: &'a [BufferSnapshot],
    pub retrieved_chunk: Option<&'a Chunk>,
}

impl<'a> RuleEvaluationContext<'a> {
    pub fn from_buffers(buffers: &'a [BufferSnapshot]) -> Self {
        Self {
            buffers,
            retrieved_chunk: None,
        }
    }

    pub fn with_retrieved_chunk(mut self, retrieved_chunk: &'a Chunk) -> Self {
        self.retrieved_chunk = Some(retrieved_chunk);
        self
    }
}

#[derive(Debug, Default, Clone)]
pub struct RuleEngine {
    rules: Vec<ProductionRule>,
}

impl RuleEngine {
    pub fn new(rules: Vec<ProductionRule>) -> Self {
        Self { rules }
    }

    pub fn rules(&self) -> &[ProductionRule] {
        &self.rules
    }

    pub fn conflict_resolution(&self, context: RuleEvaluationContext<'_>) -> ConflictResolution {
        let mut candidates = self
            .rules
            .iter()
            .map(|rule| evaluate_rule(rule, context))
            .collect::<Vec<_>>();

        let mut matches = candidates
            .iter()
            .filter(|candidate| candidate.matched)
            .map(|candidate| RuleMatch {
                rule_id: candidate.rule_id.clone(),
                name: candidate.name.clone(),
                utility: candidate.utility,
                specificity: candidate.specificity,
                version: candidate.version,
                rank: 0,
            })
            .collect::<Vec<_>>();
        matches.sort_by(compare_rule_matches);
        for (index, rule_match) in matches.iter_mut().enumerate() {
            rule_match.rank = index + 1;
        }

        for candidate in &mut candidates {
            candidate.rank = matches
                .iter()
                .find(|rule_match| rule_match.rule_id == candidate.rule_id)
                .map(|rule_match| rule_match.rank);
        }
        candidates.sort_by(compare_candidate_diagnostics);

        ConflictResolution {
            selected: matches.first().cloned(),
            candidates,
            matches,
        }
    }

    pub fn matching_rules_in_context(&self, context: RuleEvaluationContext<'_>) -> Vec<RuleMatch> {
        self.conflict_resolution(context).matches
    }

    pub fn choose_best_in_context(&self, context: RuleEvaluationContext<'_>) -> Option<RuleMatch> {
        self.conflict_resolution(context).selected
    }

    pub fn matching_rules(&self, buffers: &[BufferSnapshot]) -> Vec<RuleMatch> {
        self.matching_rules_in_context(RuleEvaluationContext::from_buffers(buffers))
    }

    pub fn choose_best(&self, buffers: &[BufferSnapshot]) -> Option<RuleMatch> {
        self.choose_best_in_context(RuleEvaluationContext::from_buffers(buffers))
    }
}

fn evaluate_rule(
    rule: &ProductionRule,
    context: RuleEvaluationContext<'_>,
) -> RuleCandidateDiagnostic {
    let buffers_match = rule
        .conditions
        .iter()
        .all(|condition| condition.matches(context.buffers));
    let retrieved_chunk_matches = rule
        .retrieved_chunk
        .as_ref()
        .is_none_or(|condition| condition.matches(context.retrieved_chunk));
    let rejection_reason = if !rule.enabled {
        Some(RuleRejectionReason::Disabled)
    } else if !rule.utility.is_finite() {
        Some(RuleRejectionReason::NonFiniteUtility)
    } else if !buffers_match {
        Some(RuleRejectionReason::BufferConditionsNotMet)
    } else if !retrieved_chunk_matches {
        Some(RuleRejectionReason::RetrievedChunkConditionNotMet)
    } else {
        None
    };

    RuleCandidateDiagnostic {
        rule_id: rule.rule_id.clone(),
        name: rule.name.clone(),
        enabled: rule.enabled,
        utility: rule.utility,
        specificity: rule.specificity(),
        version: rule.version,
        matched: rejection_reason.is_none(),
        rank: None,
        rejection_reason,
    }
}

fn compare_rule_matches(left: &RuleMatch, right: &RuleMatch) -> std::cmp::Ordering {
    right
        .specificity
        .cmp(&left.specificity)
        .then_with(|| right.utility.total_cmp(&left.utility))
        .then_with(|| right.version.cmp(&left.version))
        .then_with(|| left.rule_id.cmp(&right.rule_id))
        .then_with(|| left.name.cmp(&right.name))
}

fn compare_candidate_diagnostics(
    left: &RuleCandidateDiagnostic,
    right: &RuleCandidateDiagnostic,
) -> std::cmp::Ordering {
    right
        .enabled
        .cmp(&left.enabled)
        .then_with(|| right.matched.cmp(&left.matched))
        .then_with(|| right.specificity.cmp(&left.specificity))
        .then_with(|| right.utility.total_cmp(&left.utility))
        .then_with(|| right.version.cmp(&left.version))
        .then_with(|| left.rule_id.cmp(&right.rule_id))
        .then_with(|| left.name.cmp(&right.name))
}

#[cfg(test)]
mod tests {
    use actr_core::{AgentId, Chunk, ChunkId, ChunkType, SlotValue};
    use actr_session::{BufferName, SessionState};

    use super::*;
    use crate::rule::{BufferCondition, ProductionRule, RetrievedChunkCondition, RuleId};

    fn goal_session() -> SessionState {
        let mut session = SessionState::new(AgentId::from("agent"));
        session.set_buffer(
            BufferName::Goal,
            ChunkId::from("goal-1"),
            ChunkType::from("goal"),
            1,
        );
        session
    }

    fn retrieved_fact(topic: &str) -> Chunk {
        Chunk::new(
            AgentId::from("agent"),
            ChunkId::from("fact-1"),
            ChunkType::from("fact"),
            1,
        )
        .with_slot("topic", SlotValue::Symbol(topic.to_string()))
    }

    #[test]
    fn matches_buffer_and_optional_retrieved_chunk_conditions() {
        let session = goal_session();
        let retrieved = retrieved_fact("act-r");
        let engine = RuleEngine::new(vec![
            ProductionRule::new(
                RuleId::from("needs-retrieved"),
                "needs retrieved",
                vec![BufferCondition::buffer_present(BufferName::Goal)],
            )
            .with_retrieved_chunk(
                RetrievedChunkCondition::chunk_type(ChunkType::from("fact"))
                    .with_slot("topic", SlotValue::Symbol("ACT-R".to_string())),
            )
            .with_utility(1.0),
        ]);

        let matches = engine.matching_rules_in_context(
            RuleEvaluationContext::from_buffers(&session.snapshot())
                .with_retrieved_chunk(&retrieved),
        );

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].rule_id, RuleId::from("needs-retrieved"));
    }

    #[test]
    fn conflict_resolution_prefers_specificity_then_utility_then_deterministic_tie_breaks() {
        let session = goal_session();
        let engine = RuleEngine::new(vec![
            ProductionRule::new(
                RuleId::from("generic-high-utility"),
                "generic",
                vec![BufferCondition::buffer_present(BufferName::Goal)],
            )
            .with_utility(99.0),
            ProductionRule::new(
                RuleId::from("specific-low-utility"),
                "specific",
                vec![BufferCondition {
                    buffer: BufferName::Goal,
                    chunk_id: Some(ChunkId::from("goal-1")),
                    chunk_type: Some(ChunkType::from("goal")),
                }],
            )
            .with_utility(1.0),
            ProductionRule::new(
                RuleId::from("tie-a"),
                "tie a",
                vec![BufferCondition::buffer_present(BufferName::Goal)],
            )
            .with_utility(5.0),
            ProductionRule::new(
                RuleId::from("tie-b"),
                "tie b",
                vec![BufferCondition::buffer_present(BufferName::Goal)],
            )
            .with_utility(5.0),
        ]);

        let conflict =
            engine.conflict_resolution(RuleEvaluationContext::from_buffers(&session.snapshot()));

        assert_eq!(
            conflict.selected.as_ref().map(|rule| rule.rule_id.clone()),
            Some(RuleId::from("specific-low-utility"))
        );
        assert_eq!(conflict.matches[0].rank, 1);
        let tie_a_rank = conflict
            .matches
            .iter()
            .find(|rule_match| rule_match.rule_id == RuleId::from("tie-a"))
            .map(|rule_match| rule_match.rank);
        let tie_b_rank = conflict
            .matches
            .iter()
            .find(|rule_match| rule_match.rule_id == RuleId::from("tie-b"))
            .map(|rule_match| rule_match.rank);
        assert!(tie_a_rank < tie_b_rank);
    }

    #[test]
    fn disabled_rules_are_reported_but_not_selected() {
        let session = goal_session();
        let engine = RuleEngine::new(vec![
            ProductionRule::new(
                RuleId::from("disabled"),
                "disabled",
                vec![BufferCondition::buffer_present(BufferName::Goal)],
            )
            .with_utility(100.0)
            .disabled(),
            ProductionRule::new(
                RuleId::from("enabled"),
                "enabled",
                vec![BufferCondition::buffer_present(BufferName::Goal)],
            )
            .with_utility(1.0),
        ]);

        let conflict =
            engine.conflict_resolution(RuleEvaluationContext::from_buffers(&session.snapshot()));

        assert_eq!(
            conflict.selected.as_ref().map(|rule| rule.rule_id.clone()),
            Some(RuleId::from("enabled"))
        );
        assert_eq!(
            conflict
                .candidates
                .iter()
                .find(|candidate| candidate.rule_id == RuleId::from("disabled"))
                .and_then(|candidate| candidate.rejection_reason),
            Some(RuleRejectionReason::Disabled)
        );
    }

    #[test]
    fn no_match_path_is_explicit_and_inspectable() {
        let session = SessionState::new(AgentId::from("agent"));
        let engine = RuleEngine::new(vec![ProductionRule::new(
            RuleId::from("needs-goal"),
            "needs goal",
            vec![BufferCondition::buffer_present(BufferName::Goal)],
        )]);

        let conflict =
            engine.conflict_resolution(RuleEvaluationContext::from_buffers(&session.snapshot()));

        assert!(conflict.no_match());
        assert!(conflict.matches.is_empty());
        assert_eq!(
            conflict.candidates[0].rejection_reason,
            Some(RuleRejectionReason::BufferConditionsNotMet)
        );
    }
}
