python := env("PYTHON", ".venv/bin/python")

# Show available commands.
default:
    @just --list

# Install plotting dependencies.
setup:
    python3 -m venv .venv
    .venv/bin/python -m pip install -r requirements.txt

[private]
build:
    cargo build --release --locked

# Reproduce one and two leaders at four seconds in both placements.
quick: build
    {{python}} scripts/run.py quick

# Generate all paper CSVs and figures.
full: build
    {{python}} scripts/run.py full
