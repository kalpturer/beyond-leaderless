"""Build the RTT table from the archived CloudPing responses."""
import csv
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
REGIONS = [
    "us-west-1", "us-east-1", "eu-west-1", "ap-northeast-1", "eu-north-1",
    "ap-south-1", "sa-east-1", "eu-central-1", "ap-northeast-2", "ap-southeast-2",
]
PERCENTILES = [10, 25, 50, 75, 90, 98, 99]


def calibration():
    raw = [json.loads((ROOT / f"data/raw/cloudping-p_{p}-1Y.json").read_text())["data"]
           for p in PERCENTILES]
    rows = []
    for source in REGIONS:
        for destination in REGIONS:
            values = [float(matrix[source][destination]) for matrix in raw]
            # Independently estimated quantiles can cross: apply a running max.
            values = [max(values[:i + 1]) for i in range(len(values))]
            rows.append([source, destination, *values])
    return rows


def main():
    rows = calibration()
    path = ROOT / "data/latencies.csv"
    with path.open("w", newline="") as file:
        writer = csv.writer(file, lineterminator="\n")
        writer.writerow(["source", "destination", *[f"p{p}_rtt_ms" for p in PERCENTILES]])
        writer.writerows(rows)
    print(path)


if __name__ == "__main__":
    main()
