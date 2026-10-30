"""Run a small experiment or the full paper evaluation."""
import argparse
from datetime import datetime, timezone
from pathlib import Path
import subprocess
import sys

from results import export, load, summarize, write_csv

ROOT = Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("mode", choices=["quick", "full"])
    args = parser.parse_args()
    stamp = datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%S.%fZ")
    folder = ROOT / "results" / f"{args.mode}-{stamp}"
    folder.mkdir(parents=True, exist_ok=False)
    binary = ROOT / "target/release/beyond-leaderless"

    def run(config, name):
        destination = folder / f"{name}.json"
        subprocess.run([str(binary), "run", str(ROOT / "configs" / config), str(destination)], check=True)
        return load(destination)

    if args.mode == "quick":
        data = run("quick.json", "quick")
        write_csv(folder / "summary.csv", summarize(data))
    else:
        deadlines = run("deadlines.json", "deadlines")
        leaders = run("leaders.json", "leaders")
        export(deadlines, leaders, folder / "data")
        subprocess.run([str(binary), "utility", str(ROOT / "configs/leaders.json"),
                        str(folder / "data/single-leader-utility.csv")], check=True)
        subprocess.run([sys.executable, str(ROOT / "scripts/plot.py"),
                        str(folder / "data"), str(folder / "plots")], check=True)

    print(folder)


if __name__ == "__main__":
    main()
