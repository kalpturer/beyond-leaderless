use crate::probability::{probabilities, quorum};
use crate::{
    model::{Model, View},
    network::Network,
};

fn model() -> Model {
    Model {
        stall_probability: 0.17,
        stall_ms: 500.0,
        base_eth: 0.041,
        slope_eth_per_sec: 0.0065,
    }
}

#[test]
fn cdf_matches_quantiles_and_clamped_endpoint_masses() {
    let net = Network::new("global").unwrap();
    assert_eq!(net.cdf(0, 0, -1.0), 0.0);
    assert_eq!(net.cdf(0, 0, 0.0), 1.0);
    let low = net.quantile(0, 5, 0.0);
    let high = net.quantile(0, 5, 1.0);
    assert_eq!(net.cdf(0, 5, low.next_down()), 0.0);
    assert_eq!(net.cdf(0, 5, low), 0.10);
    assert_eq!(net.cdf(0, 5, high), 1.0);
    for a in 0..100 {
        for b in 0..100 {
            for p in [0.0, 0.05, 0.10, 0.20, 0.40, 0.70, 0.90, 0.985, 0.995, 1.0] {
                let q = net.quantile(a, b, p);
                assert!(net.cdf(a, b, q) + 1e-12 >= p);
                assert!(net.cdf(a, b, q.next_down()) <= p + 1e-12);
            }
        }
    }
}

#[test]
fn quorum_dp_matches_enumeration_of_every_vote_pattern() {
    for ps in [
        vec![0.0, 1.0, 0.2, 0.9],
        vec![0.1, 0.3, 0.5, 0.7, 0.99, 1.0, 0.0, 0.25],
    ] {
        for q in 0..=ps.len() + 1 {
            let expected: f64 = (0usize..1 << ps.len())
                .filter(|mask| mask.count_ones() as usize >= q)
                .map(|mask| {
                    ps.iter()
                        .enumerate()
                        .map(|(i, &p)| if mask & (1 << i) != 0 { p } else { 1.0 - p })
                        .product::<f64>()
                })
                .sum();
            assert!((quorum(&ps, q) - expected).abs() < 1e-12);
        }
    }
}

#[test]
fn quorum_dp_matches_exact_binomial_counts_for_49_voters() {
    // With fair independent votes, all 2^49 vote patterns are equally likely.
    let mut counts = vec![1u64; 50];
    for r in 1..=49 {
        counts[r] = counts[r - 1] * (50 - r) as u64 / r as u64;
    }
    for q in 0..=50 {
        let expected = counts[q..].iter().sum::<u64>() as f64 / (1u64 << 49) as f64;
        assert_eq!(quorum(&[0.5; 49], q), expected);
    }
}

// Enumerate every proposal-arrival matrix, then use the simulator's vote rule.
// This oracle uses neither the support-probability formula nor the quorum DP.
fn enumerate_arrivals(
    view: &View,
    profile: &[f64],
    receipt: &[Vec<f64>],
    orders: &[Vec<usize>],
) -> Vec<f64> {
    let n = view.network.n();
    let k = profile.len();
    let mut wins = vec![0.0; k];
    for mask in 0usize..1 << (n * k) {
        let mut weight = 1.0;
        let mut latest = vec![vec![f64::NEG_INFINITY; n]; k];
        for p in 0..k {
            for v in 0..n {
                if mask & (1 << (p * n + v)) != 0 {
                    weight *= receipt[p][v];
                    latest[p][v] = profile[p];
                } else {
                    weight *= 1.0 - receipt[p][v];
                }
            }
        }
        if weight == 0.0 {
            continue;
        }
        for priority in orders {
            let mut votes = vec![0; k];
            for voter in 0..n {
                if let Some(&leader) = priority.iter().find(|&&p| profile[p] <= latest[p][voter]) {
                    votes[leader] += 1;
                }
            }
            for (leader, count) in votes.iter().enumerate() {
                if *count >= view.quorum {
                    wins[leader] += weight;
                }
            }
        }
    }
    wins
}

// Independent oracle: enumerate complete priority permutations and all stalls,
// rather than grouping higher-priority sets or merging identical contexts.
fn brute(view: &View, profile: &[f64], enumerate: bool) -> Vec<f64> {
    fn permutations(rest: Vec<usize>) -> Vec<Vec<usize>> {
        if rest.is_empty() {
            return vec![vec![]];
        }
        let mut out = Vec::new();
        for (i, &p) in rest.iter().enumerate() {
            let mut tail = rest.clone();
            tail.remove(i);
            for mut order in permutations(tail) {
                order.insert(0, p);
                out.push(order);
            }
        }
        out
    }
    let k = profile.len();
    let orders = permutations((0..k).collect());
    let mut result = vec![0.0; k];
    for mask in 0usize..1 << k {
        let weight = (0..k)
            .map(|p| {
                if mask & (1 << p) == 0 {
                    1.0 - view.model.stall_probability
                } else {
                    view.model.stall_probability
                }
            })
            .product::<f64>()
            / orders.len() as f64;
        let receipt: Vec<Vec<_>> = (0..k)
            .map(|p| {
                let stall = if mask & (1 << p) == 0 {
                    0.0
                } else {
                    view.model.stall_ms
                };
                let t = view.vote_ms - stall - profile[p];
                (0..view.network.n())
                    .map(|v| {
                        if t <= 0.0 {
                            0.0
                        } else {
                            view.network.cdf(view.leaders[p], v, t)
                        }
                    })
                    .collect()
            })
            .collect();
        if enumerate {
            for (p, probability) in enumerate_arrivals(view, profile, &receipt, &orders)
                .into_iter()
                .enumerate()
            {
                result[p] += weight * probability;
            }
            continue;
        }
        for order in &orders {
            let mut eligible = vec![1.0; view.network.n()];
            for &p in order {
                let votes: Vec<_> = (0..view.network.n())
                    .map(|v| receipt[p][v] * eligible[v])
                    .collect();
                result[p] += weight * quorum(&votes, view.quorum);
                for (v, e) in eligible.iter_mut().enumerate() {
                    *e *= 1.0 - receipt[p][v];
                }
            }
        }
    }
    result
}

#[test]
fn grouped_priorities_match_full_enumeration_with_shared_stalls() {
    for placement in ["global", "regional"] {
        let net = Network::new(placement).unwrap();
        for stall_probability in [0.0, 0.17, 1.0] {
            let mut m = model();
            m.stall_probability = stall_probability;
            for k in 1..=4 {
                let view = View {
                    network: &net,
                    leaders: (0..k).map(|p| p * 5).collect(),
                    vote_ms: 600.0,
                    quorum: 67,
                    model: &m,
                };
                // Mix profiles for which stalled proposals can and cannot arrive.
                let profile: Vec<_> = (0..k).map(|p| [0.0, 485.0, 510.0, 600.0][p]).collect();
                let expected = brute(&view, &profile, false);
                let actual = probabilities(&view, &profile);
                assert!(actual.iter().sum::<f64>() <= 1.0 + 1e-12);
                for (a, b) in actual.iter().zip(expected) {
                    assert!((a - b).abs() < 1e-12);
                }
            }
        }
    }
}

#[test]
fn probabilities_match_exhaustive_arrivals_stalls_and_priorities() {
    let mut net = Network::new("global").unwrap();
    // Four voters make exhaustive receipt enumeration practical. Include both
    // distinct regions and distinct replicas sharing a region.
    let m = model();
    for regions in [vec![0, 1, 2, 3], vec![0, 0, 1, 2]] {
        net.regions = regions;
        for profile in [
            vec![0.0, 20.0, 50.0, 80.0],
            vec![480.0, 515.0, 550.0, 600.0],
        ] {
            let view = View {
                network: &net,
                leaders: vec![0, 1, 2, 3],
                vote_ms: 600.0,
                quorum: 3,
                model: &m,
            };
            view.validate(&profile).unwrap();
            let actual = probabilities(&view, &profile);
            let expected = brute(&view, &profile, true);
            for (a, b) in actual.iter().zip(&expected) {
                assert!((a - b).abs() < 1e-12, "{actual:?} != {expected:?}");
            }
        }
    }
}

#[test]
fn shared_stalls_and_priority_match_all_or_nothing_closed_form() {
    let net = Network::new("global").unwrap();
    for stall_probability in [0.0_f64, 0.01, 0.17, 1.0] {
        let mut m = model();
        m.stall_probability = stall_probability;
        m.stall_ms = 5000.0;
        for k in 1..=4 {
            let view = View {
                network: &net,
                leaders: (0..k).map(|p| p * 5).collect(),
                vote_ms: 4000.0,
                quorum: 67,
                model: &m,
            };
            // Every nonstalled prompt proposal reaches all voters. The highest
            // nonstalled leader wins, or nobody wins when all leaders stall.
            let expected = (1.0 - stall_probability.powi(k as i32)) / k as f64;
            for p in probabilities(&view, &vec![0.0; k]) {
                assert!((p - expected).abs() < 1e-14);
            }
        }
    }
}

#[test]
fn two_leader_probabilities_and_deadline_rule() {
    let mut net = Network::new("global").unwrap();
    // Independently calculated two-leader probabilities with 49 replicas.
    net.regions = [5, 5, 5, 5, 5, 5, 5, 5, 5, 4]
        .iter()
        .enumerate()
        .flat_map(|(r, &n)| std::iter::repeat_n(r, n))
        .collect();
    let mut m = model();
    m.stall_probability = 0.01;
    let view = View {
        network: &net,
        leaders: vec![0, 5],
        vote_ms: 4000.0,
        quorum: 33,
        model: &m,
    };
    let p = probabilities(&view, &[3910.0, 3915.0]);
    assert!((p[0] - 0.49995).abs() < 1e-12);
    assert!((p[1] - 0.4999390287593337).abs() < 1e-12);
    assert_eq!(probabilities(&view, &[4000.0, 4000.0]), vec![0.0, 0.0]);
    let result = crate::search::run(&view, 5.0, 64);
    assert!(result.converged);
    for player in 0..2 {
        let response = crate::probability::Response::new(&view, &result.delays_ms, player);
        for delay in crate::search::grid(4000.0, 5.0) {
            assert!(response.utility(delay) <= result.utility_eth[player] + 1e-14);
        }
    }
}

#[test]
fn minimum_experiment_deadline_allows_finalization() {
    let m = model();
    for (placement, expected) in [("global", 80), ("regional", 92)] {
        let net = Network::new(placement).unwrap();
        let view = View {
            network: &net,
            leaders: vec![0, 5],
            vote_ms: 200.0,
            quorum: 67,
            model: &m,
        };
        assert_eq!(
            crate::probability::guaranteed_final_receivers(&view),
            expected
        );
        assert!(crate::probability::guaranteed_final_receivers(&view) >= view.quorum);
    }
}

#[test]
fn network_calibration_and_election_weights() {
    for placement in ["regional", "global"] {
        let net = Network::new(placement).unwrap();
        assert_eq!(net.n(), 100);
        assert_eq!(net.quantile(0, 0, 0.9), 0.0);
        assert_eq!(net.quantile(0, 1, 0.0), 0.958 / 2.0);
        assert_eq!(net.quantile(0, 1, 1.0), 8.90156 / 2.0);
        for (k, total) in [(1, 100), (2, 4950), (3, 161700), (4, 3921225)] {
            let cases = crate::experiment::locations(&net, k);
            assert_eq!(cases.iter().map(|(_, w)| w).sum::<u64>(), total);
            if k == 2 {
                assert_eq!(cases.len(), 55);
            }
            for (ids, _) in cases {
                assert_eq!(ids.len(), k);
                assert!(ids.windows(2).all(|w| w[0] < w[1]));
            }
        }
    }
}
