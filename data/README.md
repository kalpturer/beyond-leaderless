# Data and model

`raw/` archives seven CloudPing responses fetched on **2026-08-11** from
`https://www.cloudping.co/api/latencies?percentile=<p>&timeframe=1Y`.
These are RTT quantiles for TCP pings to regional DynamoDB endpoints.
`python3 scripts/calibrate.py` rebuilds `latencies.csv`.

The evaluator divides RTTs by two, linearly interpolates the inverse CDF at
P10/P25/P50/P75/P90/P98/P99, and clamps outside P10–P99. This gives bounded
tails with endpoint masses. Messages have independent delays; self-delivery
is instantaneous, while distinct replicas in one city use its measured link.

| Region | City | Global replicas | Regional replicas |
| --- | --- | ---: | ---: |
| us-west-1 | San Jose | 10 | 20 |
| us-east-1 | Ashburn | 10 | 20 |
| eu-west-1 | Dublin | 10 | 13 |
| ap-northeast-1 | Tokyo | 10 | 4 |
| eu-north-1 | Stockholm | 10 | 13 |
| ap-south-1 | Mumbai | 10 | 4 |
| sa-east-1 | São Paulo | 10 | 4 |
| eu-central-1 | Frankfurt | 10 | 14 |
| ap-northeast-2 | Seoul | 10 | 4 |
| ap-southeast-2 | Sydney | 10 | 4 |

There are 100 replicas and quorum 67. Each leader independently stalls with
probability 1%, adding 500 ms to its proposal delivery to every recipient.
Rewards are `0.041 + 0.0065 * proposal_time_seconds` ETH.
Experiment parameters are set in `configs/`.
