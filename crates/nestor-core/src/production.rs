/// Expected utility terms for an ACT-R production.
///
/// The utility equation is `U = P * G - C`, where `P` is estimated success
/// probability, `G` is goal value, and `C` is estimated cost.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ProductionUtility {
    pub success_probability: f64,
    pub goal_value: f64,
    pub estimated_cost: f64,
}

impl ProductionUtility {
    /// Computes `P * G - C`.
    pub fn utility(self) -> f64 {
        self.success_probability * self.goal_value - self.estimated_cost
    }
}

/// Input for delta-rule utility learning.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UtilityUpdate {
    pub current: f64,
    pub reward: f64,
    pub learning_rate: f64,
}

impl UtilityUpdate {
    pub fn new(current: f64, reward: f64, learning_rate: f64) -> Self {
        Self {
            current,
            reward,
            learning_rate,
        }
    }

    pub fn apply(self) -> f64 {
        update_utility_delta(self.current, self.reward, self.learning_rate)
    }
}

/// Applies ACT-R-style utility learning: `U_n = U_o + alpha * (R - U_o)`.
pub fn update_utility_delta(current: f64, reward: f64, alpha: f64) -> f64 {
    current + alpha.clamp(0.0, 1.0) * (reward - current)
}

/// Computes numerically stable softmax probabilities for utility values.
pub fn softmax_probabilities(utilities: &[f64], temperature: f64) -> Vec<f64> {
    if utilities.is_empty() {
        return Vec::new();
    }

    let temp = temperature.max(1e-9);
    let max_utility = utilities.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let exp_values: Vec<f64> = utilities
        .iter()
        .map(|utility| ((*utility - max_utility) / temp).exp())
        .collect();
    let sum = exp_values.iter().sum::<f64>();

    exp_values.into_iter().map(|value| value / sum).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utility_follows_actr_equation() {
        let utility = ProductionUtility {
            success_probability: 0.8,
            goal_value: 10.0,
            estimated_cost: 3.0,
        };

        assert_eq!(utility.utility(), 5.0);
    }

    #[test]
    fn utility_update_moves_toward_reward() {
        assert_eq!(update_utility_delta(1.0, 3.0, 0.25), 1.5);
    }

    #[test]
    fn utility_update_clamps_learning_rate() {
        assert_eq!(UtilityUpdate::new(1.0, 3.0, -1.0).apply(), 1.0);
        assert_eq!(UtilityUpdate::new(1.0, 3.0, 2.0).apply(), 3.0);
    }

    #[test]
    fn softmax_probabilities_sum_to_one() {
        let probabilities = softmax_probabilities(&[1.0, 2.0, 3.0], 1.0);
        let sum = probabilities.iter().sum::<f64>();

        assert!((sum - 1.0).abs() < 1e-12);
        assert!(probabilities[2] > probabilities[1]);
    }
}
