"""Single-game native inference plus optional search latency (no bot pacing)."""
import argparse
import json
from pathlib import Path
import time
import numpy as np
from train import engine, observe

if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--model', required=True, help='Exported Rust JSON model')
    parser.add_argument('--decisions', type=int, default=1000)
    parser.add_argument('--search-samples', type=int, default=16)
    parser.add_argument('--config', help='Advanced search JSON; overrides --search-samples')
    parser.add_argument('--seed', type=int, default=7200000)
    parser.add_argument('--output', required=True)
    args = parser.parse_args()
    model = engine.NativeModel(args.model)
    seed = args.seed
    config = Path(args.config).read_text() if args.config else None
    latencies, inference, search_times = [], [], []
    batch = engine.Batch([seed], [3], [1], False, 0)
    for tick in range(args.decisions):
        obs, mask, _, _, _ = observe(batch)
        start = time.perf_counter()
        raw, _ = model.predict(obs.astype('<f4').tobytes())
        scores = np.frombuffer(raw, dtype='<f4').reshape(1, engine.ACTIONS)
        action = np.where(mask, scores, -1e9).argmax(-1).tolist()
        middle = time.perf_counter()
        preferred = action.copy()
        before = time.perf_counter()
        if config:
            action = [batch.advanced([seed+tick], preferred, [True], config)[0][0]]
        elif args.search_samples:
            action = [batch.search([seed+tick], preferred, [True], args.search_samples, False)[0][0]]
        selected_time = 1000 * (time.perf_counter()-before)
        inference.append(1000 * (middle-start))
        search_times.append(selected_time)
        latencies.append(inference[-1]+selected_time)
        if batch.step(action)[0] >= 0:
            seed += 1
            batch = engine.Batch([seed], [3 + seed % 4], [1], False, 0)
    def summarize(values):
        return {'median_ms':float(np.median(values)), 'p95_ms':float(np.percentile(values,95)),
                'p99_ms':float(np.percentile(values,99)), 'maximum_ms':float(max(values))}
    report = {'model': args.model, 'decisions':args.decisions, 'search_samples':args.search_samples,
              'config': json.loads(config) if config else None, 'seed': args.seed,
              'measurement':'Single-game native inference and Rust search through Python; excludes observation construction, bot pacing, and networking.',
              'total':summarize(latencies), 'inference':summarize(inference), 'search':summarize(search_times)}
    Path(args.output).write_text(json.dumps(report,indent=2)+'\n')
    print(json.dumps(report),flush=True)
