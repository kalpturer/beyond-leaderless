//! Independent one-way message delays, interpolated from measured RTT quantiles.

pub const REGIONS: [&str; 10] = [
    "us-west-1",
    "us-east-1",
    "eu-west-1",
    "ap-northeast-1",
    "eu-north-1",
    "ap-south-1",
    "sa-east-1",
    "eu-central-1",
    "ap-northeast-2",
    "ap-southeast-2",
];
const LEVELS: [f64; 7] = [0.10, 0.25, 0.50, 0.75, 0.90, 0.98, 0.99];

pub struct Network {
    pub regions: Vec<usize>,
    links: [[[f64; 7]; 10]; 10],
}

impl Network {
    pub fn new(placement: &str) -> Result<Self, String> {
        let counts = match placement {
            "global" => [10; 10],
            "regional" => [20, 20, 13, 4, 13, 4, 4, 14, 4, 4],
            _ => return Err(format!("unknown placement: {placement}")),
        };
        let regions = counts
            .iter()
            .enumerate()
            .flat_map(|(region, &count)| std::iter::repeat_n(region, count))
            .collect();
        let mut links = [[[0.0; 7]; 10]; 10];
        let mut seen = [[false; 10]; 10];
        for row in include_str!("../data/latencies.csv").lines().skip(1) {
            let fields: Vec<_> = row.split(',').collect();
            if fields.len() != 9 {
                return Err("expected nine latency CSV fields".into());
            }
            let region = |name| {
                REGIONS
                    .iter()
                    .position(|&r| r == name)
                    .ok_or("unknown region")
            };
            let (from, to) = (region(fields[0])?, region(fields[1])?);
            if seen[from][to] {
                return Err("duplicate latency link".into());
            }
            seen[from][to] = true;
            for (i, field) in fields[2..].iter().enumerate() {
                let rtt = field.parse::<f64>().map_err(|e| e.to_string())?;
                if !rtt.is_finite() || rtt <= 0.0 {
                    return Err("invalid RTT".into());
                }
                links[from][to][i] = rtt / 2.0;
            }
            if links[from][to].windows(2).any(|w| w[0] > w[1]) {
                return Err("nonmonotone RTT quantiles".into());
            }
        }
        if seen.iter().flatten().any(|&b| !b) {
            return Err("missing latency link".into());
        }
        Ok(Self { regions, links })
    }

    pub fn n(&self) -> usize {
        self.regions.len()
    }

    pub fn quantile(&self, from: usize, to: usize, p: f64) -> f64 {
        assert!((0.0..=1.0).contains(&p));
        if from == to {
            return 0.0;
        }
        let values = &self.links[self.regions[from]][self.regions[to]];
        let i = LEVELS.partition_point(|&level| level < p);
        match i {
            0 => values[0],
            7 => values[6],
            _ => {
                values[i - 1]
                    + (values[i] - values[i - 1])
                        * ((p - LEVELS[i - 1]) / (LEVELS[i] - LEVELS[i - 1]))
            }
        }
    }

    /// CDF of the same interpolated distribution defined by quantile(), including
    /// its point masses at the lowest and highest measured quantiles.
    pub fn cdf(&self, from: usize, to: usize, latency: f64) -> f64 {
        if latency < 0.0 {
            return 0.0;
        }
        if from == to {
            return 1.0;
        }
        let values = &self.links[self.regions[from]][self.regions[to]];
        if latency < values[0] {
            return 0.0;
        }
        if latency >= values[6] {
            return 1.0;
        }
        let i = values.partition_point(|&value| value <= latency);
        LEVELS[i - 1]
            + (LEVELS[i] - LEVELS[i - 1]) * (latency - values[i - 1]) / (values[i] - values[i - 1])
    }
}
