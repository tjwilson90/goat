"""Evaluate war policies with fixed, equal rummy strength and paired deal splits."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess

import numpy as np


VARIANTS = {
    "adapt": {"samples": 0},
    "selected": json.loads(Path(__file__).with_name("configs").joinpath("one-deck-war.json").read_text()),
}


def write(path, value):
    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2) + "\n")


def summarize(data, baseline="adapt"):
    if not data.get("complete"):
        raise ValueError("cannot summarize an incomplete evaluation")
    rows = data["results"]
    names = list(data["job"]["variants"])
    if baseline not in names:
        raise ValueError("baseline variant is missing")
    seeds = data["job"]["seeds"]
    rng = np.random.default_rng(192831)
    # Same sampled deal indices across seats, player counts and opponent sets.
    bootstrap = rng.integers(seeds, size=(10000, seeds))
    base_rows = [r for r in rows if r["variant"] == baseline]
    weights = np.array([r["games"] for r in base_rows], dtype=float)
    weights /= weights.sum()
    base = np.average([r["cluster_losses"] for r in base_rows], weights=weights, axis=0)
    summary = {}
    for name in names:
        matched = [next(r for r in rows if r["variant"] == name and
                        (r["players"], r["opponent"]) == (b["players"], b["opponent"])) for b in base_rows]
        candidate = np.average([r["cluster_losses"] for r in matched], weights=weights, axis=0)
        diff = (candidate - base)[bootstrap].mean(axis=1)
        base_samples = base[bootstrap].mean(axis=1)
        relative = -diff / np.maximum(base_samples, 1e-12)
        summary[name] = {
            "games": sum(r["games"] for r in matched),
            "goat_rate": float(candidate.mean()),
            "normalized_score": float(np.mean([r["players"] * r["goat_rate"] for r in matched])),
            "difference": float(candidate.mean() - base.mean()),
            "difference_ci95": np.quantile(diff, [.025, .975]).tolist(),
            "relative_reduction": float(1 - candidate.mean() / base.mean()) if base.mean() else None,
            "relative_reduction_ci95": np.quantile(relative, [.025, .975]).tolist(),
            "truncated": sum(r["truncated"] for r in matched),
            "rollouts": sum(r["rollouts"] for r in matched),
        }
        for row, control in zip(matched, base_rows):
            cell_diff = np.array(row["cluster_losses"]) - control["cluster_losses"]
            row["paired_difference_ci95"] = np.quantile(cell_diff[bootstrap].mean(axis=1), [.025, .975]).tolist()
    data["summary"] = summary
    return summary


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", required=True)
    parser.add_argument("--summarize", action="store_true")
    parser.add_argument("--seed", type=int, default=11010000)
    parser.add_argument("--seeds", type=int, default=200)
    parser.add_argument("--players", type=int, nargs="+", default=[3, 4, 5, 6])
    parser.add_argument("--decks", type=int, default=1)
    parser.add_argument("--opponents", nargs="+", default=["adapt", "mixed"])
    parser.add_argument("--rummy", choices=["simple", "advanced", "learned", "simulate-128"], default="simple")
    parser.add_argument("--model", help="Optional candidate scorer override; defaults to bundled weights")
    parser.add_argument("--rummy-config", default="training/configs/one-deck-advanced.json")
    parser.add_argument("--variants", nargs="*", choices=list(VARIANTS), default=["adapt", "selected"])
    parser.add_argument("--configs", help="additional named war configurations as a JSON object")
    parser.add_argument("--no-build", action="store_true")
    args = parser.parse_args()
    if not args.summarize:
        variants = {name: VARIANTS[name] for name in args.variants}
        if args.configs:
            variants.update(json.loads(Path(args.configs).read_text()))
        job = {"seed": args.seed, "seeds": args.seeds, "players": args.players,
               "decks": args.decks, "opponents": args.opponents, "rummy": args.rummy,
               "model": args.model, "rummy_config": args.rummy_config, "variants": variants}
        job_path = Path(args.output).with_suffix(".job.json")
        write(job_path, job)
        if not args.no_build:
            subprocess.run(["cargo", "build", "--release", "-p", "goat_training", "--bin", "war_eval"], check=True)
        sources = [Path(__file__), Path("crates/goat_training/src/lib.rs"),
                   Path("crates/goat_training/src/bin/war_eval.rs"), Path("target/release/war_eval")]
        sources += sorted(Path("crates/goat_bot/src").glob("*.rs"))
        sources += sorted(Path("crates/goat_api/src").glob("*.rs"))
        if args.rummy in ["advanced", "learned"]:
            sources += [Path(args.model) if args.model else Path("crates/goat_bot/models/rummy.json"), Path(args.rummy_config)]
        provenance = {str(p): hashlib.sha256(p.read_bytes()).hexdigest() for p in sources}
        subprocess.run(["target/release/war_eval", str(job_path), args.output], check=True)
    data = json.loads(Path(args.output).read_text())
    # Preserve provenance when only recomputing statistics for an old run.
    if not args.summarize:
        data["sha256"] = provenance
    summary = summarize(data)
    write(args.output, data)
    print(json.dumps(summary, indent=2), flush=True)


if __name__ == "__main__":
    main()
