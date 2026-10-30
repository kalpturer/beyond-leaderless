//! Cyclic best responses using deterministic expectations on the full delay grid.
use crate::model::View;
use crate::probability::{probabilities, Response};
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
pub struct Search {
    pub delays_ms: Vec<f64>,
    pub utility_eth: Vec<f64>,
    pub vc_probabilities: Vec<f64>,
    pub no_vc_probability: f64,
    pub regret_eth: Vec<f64>,
    pub best_delays_ms: Vec<f64>,
    pub converged: bool,
    pub passes: usize,
}

fn best_response(view: &View, profile: &[f64], player: usize, grid: &[f64]) -> (f64, f64) {
    let response = Response::new(view, profile, player);
    let current = response.utility(profile[player]);
    let (mut best, mut value) = (profile[player], current);
    for &delay in grid {
        let candidate = response.utility(delay);
        if candidate > value + 1e-14 {
            (best, value) = (delay, candidate);
        }
    }
    (best, (value - current).max(0.0))
}

pub fn run(view: &View, step_ms: f64, max_passes: usize) -> Search {
    let grid = grid(view.vote_ms, step_ms);
    let mut profile = vec![0.0; view.leaders.len()];
    let mut seen = Vec::new();
    let mut passes = 0;
    while passes < max_passes && !seen.contains(&profile) {
        seen.push(profile.clone());
        passes += 1;
        let old = profile.clone();
        for p in 0..profile.len() {
            profile[p] = best_response(view, &profile, p, &grid).0;
        }
        if profile == old {
            break;
        }
    }
    let (best_delays_ms, regret_eth): (Vec<_>, Vec<_>) = (0..profile.len())
        .map(|p| best_response(view, &profile, p, &grid))
        .unzip();
    let vc_probabilities = probabilities(view, &profile);
    // Strict-majority VCs are mutually exclusive.
    let no_vc_probability = (1.0 - vc_probabilities.iter().sum::<f64>()).clamp(0.0, 1.0);
    let utility_eth = profile
        .iter()
        .zip(&vc_probabilities)
        .map(|(&s, &p)| view.model.value(s) * p)
        .collect();
    Search {
        delays_ms: profile,
        utility_eth,
        vc_probabilities,
        no_vc_probability,
        converged: regret_eth.iter().all(|&g| g <= 1e-14),
        regret_eth,
        best_delays_ms,
        passes,
    }
}

pub fn grid(vote_ms: f64, step_ms: f64) -> Vec<f64> {
    let mut grid: Vec<_> = (0..=(vote_ms / step_ms).floor() as usize)
        .map(|i| i as f64 * step_ms)
        .collect();
    if *grid.last().unwrap() < vote_ms {
        grid.push(vote_ms);
    }
    grid
}
