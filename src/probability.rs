//! Deterministic VC probabilities. Condition on shared stalls and priorities;
//! proposal arrivals at different voters are then independent.
use crate::model::View;

/// Probability that independent Bernoulli variables contain at least q successes.
pub fn quorum(probabilities: &[f64], q: usize) -> f64 {
    let certain = probabilities.iter().filter(|&&p| p == 1.0).count();
    if certain >= q {
        return 1.0;
    }
    let q = q - certain;
    let uncertain: Vec<_> = probabilities
        .iter()
        .copied()
        .filter(|&p| p > 0.0 && p < 1.0)
        .collect();
    if uncertain.len() < q {
        return 0.0;
    }
    let mut dp = vec![0.0; q + 1];
    dp[0] = 1.0;
    for p in uncertain {
        // q is absorbing: its mass already includes every larger vote count.
        dp[q] += dp[q - 1] * p;
        for j in (1..q).rev() {
            dp[j] = dp[j] * (1.0 - p) + dp[j - 1] * p;
        }
        dp[0] *= 1.0 - p;
    }
    dp[q].clamp(0.0, 1.0)
}

fn arrivals(view: &View, player: usize, delay: f64, stalled: bool) -> Vec<f64> {
    let stall = if stalled { view.model.stall_ms } else { 0.0 };
    let remaining = view.vote_ms - stall - delay;
    // Proposals broadcast at the voting deadline miss the vote, even locally.
    if delay >= view.vote_ms - stall {
        return vec![0.0; view.network.n()];
    }
    (0..view.network.n())
        .map(|v| view.network.cdf(view.leaders[player], v, remaining))
        .collect()
}

fn choose(n: usize, k: usize) -> usize {
    (0..k.min(n - k)).fold(1, |value, i| value * (n - i) / (i + 1))
}

/// Cache opponents while sweeping a single leader's delay.
pub struct Response<'a, 'b> {
    view: &'a View<'b>,
    player: usize,
    // Probability and eligible-voter probabilities for each shared scenario.
    contexts: Vec<(f64, Vec<f64>)>,
}

impl<'a, 'b> Response<'a, 'b> {
    pub fn new(view: &'a View<'b>, profile: &[f64], player: usize) -> Self {
        let opponents: Vec<_> = (0..profile.len()).filter(|&p| p != player).collect();
        let receipt: Vec<_> = opponents
            .iter()
            .map(|&p| {
                [
                    arrivals(view, p, profile[p], false),
                    arrivals(view, p, profile[p], true),
                ]
            })
            .collect();
        let mut contexts: Vec<(f64, Vec<f64>)> = Vec::new();
        let k = profile.len();
        // Only the set above this leader matters, not its internal order.
        // A given h-element set ranks above this leader with probability 1/(k*C(k-1,h)).
        for higher in 0usize..(1 << opponents.len()) {
            let h = higher.count_ones() as usize;
            let priority_weight = 1.0 / (k * choose(k - 1, h)) as f64;
            let mut stalled = higher;
            loop {
                let mut eligible = vec![1.0; view.network.n()];
                let mut weight = priority_weight;
                for (j, rows) in receipt.iter().enumerate() {
                    if higher & (1 << j) == 0 {
                        continue;
                    }
                    let state = usize::from(stalled & (1 << j) != 0);
                    weight *= if state == 1 {
                        view.model.stall_probability
                    } else {
                        1.0 - view.model.stall_probability
                    };
                    for (v, p) in eligible.iter_mut().enumerate() {
                        *p *= 1.0 - rows[state][v];
                    }
                }
                if weight > 0.0 {
                    // Stalled late opponents often produce identical scenarios.
                    if let Some((w, _)) = contexts.iter_mut().find(|(_, p)| *p == eligible) {
                        *w += weight;
                    } else {
                        contexts.push((weight, eligible));
                    }
                }
                if stalled == 0 {
                    break;
                }
                stalled = (stalled - 1) & higher;
            }
        }
        Self {
            view,
            player,
            contexts,
        }
    }

    pub fn probability(&self, delay: f64) -> f64 {
        let mut probability = 0.0;
        for stalled in [false, true] {
            let weight = if stalled {
                self.view.model.stall_probability
            } else {
                1.0 - self.view.model.stall_probability
            };
            if weight == 0.0 {
                continue;
            }
            let receipt = arrivals(self.view, self.player, delay, stalled);
            if receipt.iter().filter(|&&p| p > 0.0).count() < self.view.quorum {
                continue;
            }
            for (w, eligible) in &self.contexts {
                let votes: Vec<_> = receipt.iter().zip(eligible).map(|(a, b)| a * b).collect();
                probability += weight * w * quorum(&votes, self.view.quorum);
            }
        }
        probability.clamp(0.0, 1.0)
    }

    pub fn utility(&self, delay: f64) -> f64 {
        self.view.model.value(delay) * self.probability(delay)
    }
}

pub fn probabilities(view: &View, profile: &[f64]) -> Vec<f64> {
    (0..profile.len())
        .map(|p| Response::new(view, profile, p).probability(profile[p]))
        .collect()
}

/// Count recipients whose incoming link delays are all bounded by vote_ms.
pub fn guaranteed_final_receivers(view: &View) -> usize {
    (0..view.network.n())
        .filter(|&receiver| {
            (0..view.network.n())
                .all(|sender| view.network.quantile(sender, receiver, 1.0) <= view.vote_ms)
        })
        .count()
}
