"""Compare one-deck policies and search budgets on validation deals only."""
import argparse
import hashlib
import json
from pathlib import Path
import numpy as np
import torch
from train import CandidatePolicy, load, tournament

if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--models', nargs='*', default=[])
    parser.add_argument('--seeds', type=int, default=200)
    parser.add_argument('--seed', type=int, default=6_000_000)
    parser.add_argument('--search', nargs='+', type=int, default=[0, -1, 16])
    parser.add_argument('--output', required=True)
    parser.add_argument('--skip-simple', action='store_true')
    args = parser.parse_args()
    torch.set_num_threads(2)
    models = ([] if args.skip_simple else [('simple-prior', None)]) + [(path, path) for path in args.models]
    results = []
    for name, path in models:
        model, checkpoint = load(path) if path else (CandidatePolicy(64), {})
        digest = hashlib.sha256(Path(path).read_bytes()).hexdigest() if path else None
        for search in args.search:
            rows = [tournament(model, seeds=range(args.seed, args.seed + args.seeds), players=n, decks=1,
                search_samples=max(0, search), endgame=search == -1) for n in [3,4,5,6]]
            result = {'model': name, 'search': search,
                      'model_sha256': digest, 'checkpoint_metadata': checkpoint.get('metadata'),
                      'normalized_goat_rate': float(np.mean([r['goat_rate'] * r['players'] for r in rows])),
                      'results': [{k:v for k,v in row.items() if k != 'cluster_losses'} for row in rows]}
            results.append(result)
            print(json.dumps(result), flush=True)
            Path(args.output).parent.mkdir(parents=True, exist_ok=True)
            Path(args.output).write_text(json.dumps({'seed':args.seed, 'seeds':args.seeds, 'results':results}, indent=2) + '\n')
