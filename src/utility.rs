//! Expected reward for a single Frankfurt leader.
use crate::{
    experiment::Config,
    model::View,
    network::{Network, REGIONS},
    probability::Response,
};
use std::{error::Error, fs, path::Path};

pub fn write(config: &Config, path: &Path) -> Result<(), Box<dyn Error>> {
    if config.vote_times_ms.len() != 1 || config.step_ms <= 0.0 || !config.step_ms.is_finite() {
        return Err("need one vote time and a positive finite grid step".into());
    }
    let t = config.vote_times_ms[0];
    if !t.is_finite() || t <= 0.0 {
        return Err("need a positive finite vote time".into());
    }
    let grid: Vec<_> = crate::search::grid(t, config.step_ms)
        .into_iter()
        .filter(|&s| s >= t - 800.0 || s % 10.0 == 0.0)
        .collect();
    let mut curves = Vec::new();
    for placement in ["global", "regional"] {
        let network = Network::new(placement)?;
        let leader = network
            .regions
            .iter()
            .position(|&r| REGIONS[r] == "eu-central-1")
            .ok_or("missing Frankfurt")?;
        let view = View {
            network: &network,
            leaders: vec![leader],
            vote_ms: t,
            quorum: config.quorum,
            model: &config.model,
        };
        view.validate(&[0.0])?;
        let response = Response::new(&view, &[0.0], 0);
        curves.push(
            grid.iter()
                .map(|&s| response.probability(s))
                .collect::<Vec<_>>(),
        );
    }
    let mut csv = String::from("proposal_time_ms,block_value_eth,global_success,global_utility_eth,regional_success,regional_utility_eth\n");
    for (i, &s) in grid.iter().enumerate() {
        let value = config.model.value(s);
        let (global, regional) = (curves[0][i], curves[1][i]);
        csv.push_str(&format!(
            "{s},{value},{global},{},{regional},{}\n",
            value * global,
            value * regional
        ));
    }
    if path.exists() {
        return Err("output already exists".into());
    }
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, csv)?;
    Ok(())
}
