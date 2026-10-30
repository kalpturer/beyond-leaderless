//! One view, independent proposer stalls, and proposal rewards.
use crate::network::Network;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Model {
    pub stall_probability: f64,
    pub stall_ms: f64,
    pub base_eth: f64,
    pub slope_eth_per_sec: f64,
}

impl Model {
    pub fn value(&self, delay_ms: f64) -> f64 {
        self.base_eth + self.slope_eth_per_sec * delay_ms / 1000.0
    }
}

pub struct View<'a> {
    pub network: &'a Network,
    pub leaders: Vec<usize>,
    pub vote_ms: f64,
    pub quorum: usize,
    pub model: &'a Model,
}

impl View<'_> {
    pub fn validate(&self, profile: &[f64]) -> Result<(), String> {
        let n = self.network.n();
        if self.leaders.is_empty()
            || self.leaders.len() != profile.len()
            || self.leaders.iter().any(|&id| id >= n)
            || (0..self.leaders.len()).any(|i| self.leaders[..i].contains(&self.leaders[i]))
        {
            return Err("need distinct valid leaders and one delay per leader".into());
        }
        if self.quorum <= n / 2 || self.quorum > n {
            return Err("quorum must be a strict majority, at most n".into());
        }
        if !self.vote_ms.is_finite()
            || self.vote_ms <= 0.0
            || !(2.0 * self.vote_ms).is_finite()
            || profile
                .iter()
                .any(|&s| !s.is_finite() || s < 0.0 || s > self.vote_ms)
        {
            return Err("invalid vote time or proposal delay".into());
        }
        let m = self.model;
        if !(0.0..=1.0).contains(&m.stall_probability)
            || !m.stall_ms.is_finite()
            || m.stall_ms < 0.0
            || !m.base_eth.is_finite()
            || m.base_eth <= 0.0
            || !m.slope_eth_per_sec.is_finite()
            || m.slope_eth_per_sec < 0.0
        {
            return Err("invalid stall or value parameters".into());
        }
        Ok(())
    }
}
