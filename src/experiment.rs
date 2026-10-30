//! Enumerate distinct leader sets by location and solve each timing game.
use crate::{
    model::{Model, View},
    network::Network,
    search,
};
use serde::{Deserialize, Serialize};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Mutex,
};
use std::time::Instant;

#[derive(Deserialize, Serialize)]
pub struct Config {
    pub placements: Vec<String>,
    pub vote_times_ms: Vec<f64>,
    pub leaders: Vec<usize>,
    pub quorum: usize,
    pub model: Model,
    pub step_ms: f64,
    pub max_passes: usize,
    pub workers: usize,
}

#[derive(Serialize)]
pub struct Profile {
    pub case: Case,
    pub search: search::Search,
    pub prompt_no_vc_probability: Option<f64>,
}

#[derive(Serialize)]
pub struct Results {
    pub config: Config,
    pub elapsed_seconds: f64,
    pub profiles: Vec<Profile>,
}

pub fn run(config: Config) -> Result<Results, String> {
    if config.placements.is_empty()
        || config.vote_times_ms.is_empty()
        || config.leaders.is_empty()
        || config.workers == 0
        || config.max_passes == 0
        || !config.step_ms.is_finite()
        || config.step_ms <= 0.0
    {
        return Err("empty experiment or invalid compute settings".into());
    }
    let mut cases = Vec::new();
    for placement in &config.placements {
        let network = Network::new(placement)?;
        for &vote_ms in &config.vote_times_ms {
            for &k in &config.leaders {
                if k == 0 || k > 4 {
                    return Err("supported leader counts are 1 through 4".into());
                }
                View {
                    network: &network,
                    leaders: (0..k).collect(),
                    vote_ms,
                    quorum: config.quorum,
                    model: &config.model,
                }
                .validate(&vec![0.0; k])?;
                let locations = locations(&network, k);
                let total = locations.iter().map(|(_, w)| w).sum::<u64>() as f64;
                for (leaders, multiplicity) in locations {
                    cases.push(Case {
                        placement: placement.clone(),
                        vote_ms,
                        regions: leaders.iter().map(|&id| network.regions[id]).collect(),
                        leaders,
                        multiplicity,
                        weight: multiplicity as f64 / total,
                    });
                }
            }
        }
    }
    let start = Instant::now();
    let profiles = parallel(&cases, config.workers, true, |_, case| {
        let network = Network::new(&case.placement).unwrap();
        let view = View {
            network: &network,
            leaders: case.leaders.clone(),
            vote_ms: case.vote_ms,
            quorum: config.quorum,
            model: &config.model,
        };
        let search = search::run(&view, config.step_ms, config.max_passes);
        let prompt_no_vc_probability = (case.leaders.len() == 1)
            .then(|| (1.0 - crate::probability::probabilities(&view, &[0.0])[0]).clamp(0.0, 1.0));
        Profile {
            case: case.clone(),
            search,
            prompt_no_vc_probability,
        }
    });
    Ok(Results {
        config,
        elapsed_seconds: start.elapsed().as_secs_f64(),
        profiles,
    })
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Case {
    pub placement: String,
    pub vote_ms: f64,
    pub leaders: Vec<usize>,
    pub regions: Vec<usize>,
    pub multiplicity: u64,
    pub weight: f64,
}

fn choose(n: usize, k: usize) -> u64 {
    if k > n {
        return 0;
    }
    (0..k.min(n - k)).fold(1, |c, i| c * (n - i) as u64 / (i + 1) as u64)
}

/// Unordered region multisets, including same-region pairs of distinct replicas.
/// Weight is product C(region size, leaders there), divided by C(n,k).
pub fn locations(network: &Network, k: usize) -> Vec<(Vec<usize>, u64)> {
    fn visit(
        groups: &[Vec<usize>],
        region: usize,
        left: usize,
        ids: &mut Vec<usize>,
        weight: u64,
        out: &mut Vec<(Vec<usize>, u64)>,
    ) {
        if left == 0 {
            out.push((ids.clone(), weight));
            return;
        }
        if region == groups.len() {
            return;
        }
        for count in 0..=left.min(groups[region].len()) {
            ids.extend(&groups[region][..count]);
            visit(
                groups,
                region + 1,
                left - count,
                ids,
                weight * choose(groups[region].len(), count),
                out,
            );
            ids.truncate(ids.len() - count);
        }
    }
    let groups: Vec<Vec<_>> = (0..10)
        .map(|r| {
            (0..network.n())
                .filter(|&i| network.regions[i] == r)
                .collect()
        })
        .collect();
    let mut out = Vec::new();
    visit(&groups, 0, k, &mut Vec::new(), 1, &mut out);
    out
}

pub(crate) fn parallel<T: Sync, R: Send>(
    jobs: &[T],
    workers: usize,
    report: bool,
    f: impl Fn(usize, &T) -> R + Sync,
) -> Vec<R> {
    let next = AtomicUsize::new(0);
    let done = AtomicUsize::new(0);
    let output = Mutex::new(Vec::new());
    std::thread::scope(|scope| {
        for _ in 0..workers.min(jobs.len()) {
            scope.spawn(|| loop {
                let i = next.fetch_add(1, Ordering::Relaxed);
                if i >= jobs.len() {
                    break;
                }
                let result = f(i, &jobs[i]);
                output.lock().unwrap().push((i, result));
                let count = done.fetch_add(1, Ordering::Relaxed) + 1;
                if report && (count.is_multiple_of(100) || count == jobs.len()) {
                    eprintln!("{count}/{} cases", jobs.len());
                }
            });
        }
    });
    let mut output = output.into_inner().unwrap();
    output.sort_by_key(|(i, _)| *i);
    output.into_iter().map(|(_, result)| result).collect()
}
