use nestor_core::update_utility_delta;

use crate::rule::{ProductionRule, RuleId};

#[derive(Debug, Clone, PartialEq)]
pub struct RuleRewardUpdate {
    pub rule_id: RuleId,
    pub previous_utility: f64,
    pub reward: f64,
    pub learning_rate: f64,
    pub updated_utility: f64,
    pub previous_version: u64,
    pub updated_version: u64,
}

pub fn apply_reward(current_utility: f64, reward: f64, learning_rate: f64) -> f64 {
    update_utility_delta(current_utility, reward, learning_rate)
}

pub fn reward_update(
    rule_id: RuleId,
    current_utility: f64,
    current_version: u64,
    reward: f64,
    learning_rate: f64,
) -> RuleRewardUpdate {
    RuleRewardUpdate {
        rule_id,
        previous_utility: current_utility,
        reward,
        learning_rate: learning_rate.clamp(0.0, 1.0),
        updated_utility: apply_reward(current_utility, reward, learning_rate),
        previous_version: current_version,
        updated_version: current_version.saturating_add(1),
    }
}

pub fn apply_reward_to_rule(
    rule: &mut ProductionRule,
    reward: f64,
    learning_rate: f64,
) -> RuleRewardUpdate {
    let update = reward_update(
        rule.rule_id.clone(),
        rule.utility,
        rule.version,
        reward,
        learning_rate,
    );
    rule.utility = update.updated_utility;
    rule.version = update.updated_version;
    update
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rule::{BufferCondition, RuleId};
    use nestor_session::BufferName;

    #[test]
    fn reward_update_reuses_core_delta_learning() {
        assert_eq!(apply_reward(0.0, 10.0, 0.1), 1.0);
    }

    #[test]
    fn rule_reward_update_moves_utility_and_versions_metadata() {
        let mut rule = ProductionRule::new(
            RuleId::from("rule-1"),
            "rule one",
            vec![BufferCondition::buffer_present(BufferName::Goal)],
        )
        .with_utility(2.0)
        .with_version(7);

        let update = apply_reward_to_rule(&mut rule, 6.0, 0.25);

        assert_eq!(update.previous_utility, 2.0);
        assert_eq!(update.updated_utility, 3.0);
        assert_eq!(update.previous_version, 7);
        assert_eq!(update.updated_version, 8);
        assert_eq!(rule.utility, 3.0);
        assert_eq!(rule.version, 8);
    }
}
