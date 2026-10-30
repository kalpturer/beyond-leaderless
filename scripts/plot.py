"""Plot the paper's leader-count, deadline, and utility comparisons."""
import argparse
import csv
from pathlib import Path

import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt
from matplotlib.ticker import PercentFormatter, ScalarFormatter


def read(folder, name):
    with (folder / f"{name}.csv").open() as file:
        return list(csv.DictReader(file))


def save(figure, folder, name):
    for suffix in ["pdf", "png"]:
        figure.savefig(folder / f"{name}.{suffix}", dpi=180)
    plt.close(figure)


def plot(data, output):
    output.mkdir(parents=True, exist_ok=True)
    for kind in ["leaders", "deadlines"]:
        fig, axes = plt.subplots(1, 2, figsize=(9, 3.3), layout="constrained")
        for ax, placement in zip(axes, ["global", "regional"]):
            if kind == "leaders":
                rows = read(data, f"{placement}-k-stall")
                ax.plot([int(r["k"]) for r in rows],
                        [float(r["computed_miss"]) for r in rows], color="black")
                ax.set(xlabel="number of leaders", xticks=[1, 2, 3, 4], ylim=(-0.0002, 0.011))
            else:
                for k, field, label, style in [
                    (1, "computed_miss", "one leader", "-"),
                    (2, "computed_miss", "two leaders", "--"),
                    (1, "prompt_miss", "one prompt leader", "-."),
                ]:
                    rows = read(data, f"{placement}-k{k}-stall")
                    ax.plot([float(r["deadline_ms"]) / 1000 for r in rows],
                            [float(r[field]) for r in rows], style, color="black", label=label)
                ax.set(xscale="log", xlabel="voting deadline (seconds)",
                       xticks=[0.2, 0.5, 1, 2, 4], ylim=(-0.0005, 0.013))
                ax.xaxis.set_major_formatter(ScalarFormatter())
                ax.legend(frameon=False, fontsize=8)
            ax.set(title=placement.capitalize(), ylabel="view-failure probability")
            ax.yaxis.set_major_formatter(PercentFormatter(1))
            ax.grid(alpha=0.2)
        save(fig, output, kind)

    rows = read(data, "single-leader-utility")
    fig, axes = plt.subplots(1, 2, figsize=(9, 3.3), layout="constrained")
    for ax in axes:
        for placement, style in [("global", "-"), ("regional", "--")]:
            ax.plot([float(r["proposal_time_ms"]) / 1000 for r in rows],
                    [float(r[f"{placement}_utility_eth"]) for r in rows],
                    style, color="black", label=placement)
        ax.set(xlabel="proposal time (seconds)", ylabel="expected reward (ETH)")
        ax.grid(alpha=0.2)
    axes[0].set(title="Frankfurt leader", xlim=(0, 4), ylim=(0, 0.07))
    axes[0].legend(frameon=False)
    axes[1].set(title="Near the peak", xlim=(3.2, 4), ylim=(0.061, 0.067))
    save(fig, output, "utility")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("data", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    plot(args.data, args.output)
