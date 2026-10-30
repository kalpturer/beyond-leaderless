"""Validate profiles, aggregate election weights, and export the paper CSVs."""
import csv
import json
from collections import Counter, defaultdict
from math import comb, isclose
from pathlib import Path

COUNTS = {"global": [10] * 10, "regional": [20, 20, 13, 4, 13, 4, 4, 14, 4, 4]}


def load(path):
    return json.loads(Path(path).read_text())


def summarize(data):
    groups = defaultdict(list)
    config = data["config"]
    for profile in data["profiles"]:
        case, search = profile["case"], profile["search"]
        counts = COUNTS[case["placement"]]
        n, k = sum(counts), len(case["leaders"])
        regions = [region for region, count in enumerate(counts) for _ in range(count)]
        if len(set(case["leaders"])) != k:
            raise ValueError("leaders must be distinct replicas")
        if [regions[i] for i in case["leaders"]] != case["regions"]:
            raise ValueError("leader locations do not match the placement")
        multiplicity = 1
        for region, count in Counter(case["regions"]).items():
            multiplicity *= comb(counts[region], count)
        if case["multiplicity"] != multiplicity or not isclose(
            case["weight"], multiplicity / comb(n, k), rel_tol=0, abs_tol=1e-15
        ):
            raise ValueError("incorrect leader-election weight")
        if not search["converged"] or max(search["regret_eth"]) > 1e-14:
            raise ValueError("profile failed the final grid-deviation check")
        probabilities = search["vc_probabilities"]
        if any(not 0 <= p <= 1 for p in probabilities) or not isclose(
            sum(probabilities) + search["no_vc_probability"], 1, abs_tol=1e-12
        ):
            raise ValueError("invalid certificate probabilities")
        groups[case["placement"], case["vote_ms"], k].append(profile)
    rows = []
    for (placement, deadline, k), profiles in sorted(groups.items()):
        if not isclose(sum(p["case"]["weight"] for p in profiles), 1, abs_tol=1e-12):
            raise ValueError("leader-election weights do not sum to one")
        rows.append(dict(
            placement=placement, k=k, deadline_ms=deadline,
            leader_sets=len(profiles), grid_ms=config["step_ms"],
            prompt_miss=sum(p["case"]["weight"] * p["prompt_no_vc_probability"]
                            for p in profiles) if k == 1 else "",
            computed_miss=sum(p["case"]["weight"] * p["search"]["no_vc_probability"]
                              for p in profiles),
            mean_margin_ms=sum(p["case"]["weight"] *
                               (deadline - sum(p["search"]["delays_ms"]) / k)
                               for p in profiles),
            max_regret_eth=max(max(p["search"]["regret_eth"]) for p in profiles),
        ))
    return rows


def write_csv(path, rows):
    with Path(path).open("w", newline="") as file:
        writer = csv.DictWriter(file, fieldnames=list(rows[0]), lineterminator="\n")
        writer.writeheader()
        writer.writerows(rows)


def export(deadlines, leaders, folder):
    folder = Path(folder)
    folder.mkdir(parents=True, exist_ok=True)
    sweep, counts = summarize(deadlines), summarize(leaders)
    for row in sweep:
        if row["deadline_ms"] == 4000 and row not in counts:
            raise ValueError("overlapping experiments disagree")
    for placement in COUNTS:
        for k in [1, 2]:
            write_csv(folder / f"{placement}-k{k}-stall.csv",
                      [r for r in sweep if r["placement"] == placement and r["k"] == k])
        write_csv(folder / f"{placement}-k-stall.csv",
                  [r for r in counts if r["placement"] == placement])
