use actr_core::update_utility_delta;

pub fn apply_reward(current_utility: f64, reward: f64, learning_rate: f64) -> f64 {
    update_utility_delta(current_utility, reward, learning_rate)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reward_update_reuses_core_delta_learning() {
        assert_eq!(apply_reward(0.0, 10.0, 0.1), 1.0);
    }
}
