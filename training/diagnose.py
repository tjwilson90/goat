"""Paired move diagnostics on learner-visited, one-deck rummy positions."""
import argparse
from collections import defaultdict
import json
from pathlib import Path
import numpy as np
import torch
from train import engine, load, observe


def diagnose(model, *, seed=6_100_000, games=64, positions=256, samples=64):
    report = []
    for players in [3, 4, 5, 6]:
        batch = engine.Batch(list(range(seed, seed + games)), [players] * games, [1] * games, False, 0)
        done = np.zeros(games, dtype=bool)
        groups = defaultdict(lambda: {'positions': 0, 'paired': 0, 'learner_loss_sum': 0., 'teacher_loss_sum': 0.})
        examples = []
        decisions = disagreements = queried = proofs = 0
        bucket_counts = defaultdict(int)
        for tick in range(2048):
            if done.all():
                break
            obs, mask, teachers, actors, _ = observe(batch)
            with torch.no_grad():
                actions = model(torch.from_numpy(obs), torch.from_numpy(mask))[0].argmax(-1).numpy()
            active = ~done & (mask.sum(1) > 1)
            different = active & (actions != teachers)
            decisions += int(active.sum())
            disagreements += int(different.sum())
            active_counts = obs[:, :15 * 111].reshape(-1, 15, 111)[:, :, 106].sum(1)
            chosen = []
            for i in np.flatnonzero(different):
                bucket = 'endgame' if active_counts[i] == 2 else ('lead' if obs[i, 15 * 111 + 9] == 0 else 'response')
                quota = positions // 2 if bucket == 'endgame' else positions // 4
                if bucket_counts[bucket] < quota:
                    chosen.append(i)
                    bucket_counts[bucket] += 1
            chosen = np.array(chosen, dtype=int)
            if chosen.size:
                enabled = np.zeros(games, dtype=bool)
                enabled[chosen] = True
                comparisons = batch.search([seed + i + tick * 100003 for i in range(games)],
                    actions.tolist(), enabled.tolist(), samples, False)
                indices, raw = engine.candidates(obs.astype('<f4').tobytes())
                features = dict(zip(map(tuple, indices), np.frombuffer(raw, dtype='<f4').reshape(-1, engine.CANDIDATE_FEATURES)))
                for i in chosen:
                    action, teacher = int(actions[i]), int(teachers[i])
                    own = features[i, action]
                    target = features[i, teacher]
                    categories = ['all', 'two_active' if round(float(own[11]) * 6) == 2 else 'three_plus_active']
                    categories += ['lead' if own[9] else 'response']
                    if own[1] != target[1]: categories += ['pickup_vs_play']
                    if own[2] != target[2]: categories += ['trump_choice']
                    if target[8] and not own[8]: categories += ['missed_teacher_exit']
                    if own[14] > target[14]: categories += ['more_remaining_runs']
                    best, losses, exact, _ = comparisons[i]
                    proofs += int(exact)
                    loss = dict(losses)
                    paired = action in loss and teacher in loss
                    for category in categories:
                        group = groups[category]
                        group['positions'] += 1
                        if paired:
                            group['paired'] += 1
                            group['learner_loss_sum'] += loss[action]
                            group['teacher_loss_sum'] += loss[teacher]
                    if len(examples) < 12:
                        examples.append({'seed': seed + int(i), 'tick': tick, 'actor': int(actors[i]),
                            'learner': action, 'teacher': teacher, 'search': best, 'proved_safe': exact,
                            'learner_loss': loss.get(action), 'teacher_loss': loss.get(teacher), 'categories': categories})
                queried += len(chosen)
            actions[done] = 472
            done = np.asarray(batch.step(actions.tolist())) >= 0
        for group in groups.values():
            n = group['paired']
            group['mean_paired_loss_difference'] = (group['learner_loss_sum'] - group['teacher_loss_sum']) / n if n else None
        report.append({'players': players, 'games': games, 'decisions': decisions, 'disagreements': disagreements,
                       'queried': queried, 'buckets': dict(bucket_counts), 'proved_safe_positions': proofs, 'groups': dict(groups), 'examples': examples})
    return report


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--model', required=True)
    parser.add_argument('--output', required=True)
    parser.add_argument('--seed', type=int, default=6_100_000)
    parser.add_argument('--samples', type=int, default=64)
    parser.add_argument('--positions', type=int, default=256)
    args = parser.parse_args()
    torch.set_num_threads(2)
    result = {'model': args.model, 'seed': args.seed, 'samples_per_action': args.samples,
              'interpretation': 'Conditional diagnostic on disagreements, under simple continuations; not causal estimates of tournament strength.',
              'results': diagnose(load(args.model)[0], seed=args.seed, positions=args.positions, samples=args.samples)}
    Path(args.output).parent.mkdir(parents=True, exist_ok=True)
    Path(args.output).write_text(json.dumps(result, indent=2) + '\n')
    for row in result['results']:
        print(json.dumps({k: v for k, v in row.items() if k != 'examples'}), flush=True)
