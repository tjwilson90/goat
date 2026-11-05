import copy
import json
from pathlib import Path
import tempfile
import unittest

import numpy as np
import torch

from train import CandidatePolicy, Policy, collect, engine, load, observe, optimize, parity, permute_suits, save


class TrainingTests(unittest.TestCase):
    def test_advanced_rejects_bad_configs_and_honors_rollout_cap(self):
        batch = engine.Batch([3,4], [3,4], [1,1], False, 0)
        _, _, teacher, _, _ = observe(batch)
        for config in [{'styles':[]}, {'styles':[4]}, {'samples':0}, {'unknown':1}, {'max_rollouts':65537}]:
            with self.assertRaises(ValueError):
                batch.advanced([1,2], teacher.tolist(), [True,True], json.dumps(config))
        result = batch.advanced([1,2], teacher.tolist(), [True,True], json.dumps({'max_rollouts':1,'endgame_cards':0}))
        self.assertTrue(all(r[3]<=1 for r in result))
        self.assertEqual([r[0] for r in result],teacher.tolist())

    @classmethod
    def setUpClass(cls):
        torch.set_num_threads(2)

    def test_suit_augmentation_preserves_action_and_observation_mapping(self):
        for war in [False, True]:
            batch = engine.Batch([17, 23], [3, 6], [1, 3], war, 0)
            obs, mask, teacher, _, _ = observe(batch)
            permutation = np.array([2, 0, 3, 1])
            augmented, new_mask, target, order = permute_suits(obs, mask, teacher, permutation)
            recovered, recovered_mask, recovered_teacher, _ = permute_suits(
                augmented, new_mask, target, np.argsort(permutation))
            np.testing.assert_array_equal(obs, recovered)
            np.testing.assert_array_equal(mask, recovered_mask)
            np.testing.assert_array_equal(teacher, recovered_teacher)
            np.testing.assert_array_equal(order[target], teacher)
            self.assertTrue(new_mask[np.arange(2), target].all())
            batch.step(order[target].tolist())

    def test_seeded_rollouts_and_terminal_credit_are_reproducible(self):
        model = Policy(16)
        kwargs = dict(games=4, seed=21, player_counts=[3], deck_counts=[1],
                      train_war=False, war_style=0, suit_augmentation=True)
        torch.manual_seed(99)
        a, _ = collect(model, **kwargs)
        torch.manual_seed(99)
        b, _ = collect(model, **kwargs)
        for key in a:
            torch.testing.assert_close(a[key], b[key], rtol=0, atol=0)
        self.assertEqual(set(a["returns"].tolist()), {-1.0, 0.0})
        self.assertTrue(a["mask"].gather(1, a["action"][:, None]).all())

    def test_war_updates_preserve_rummy_continuation_and_export(self):
        model = Policy(16)
        model.war = Policy(16)
        for component in [model.trunk, model.policy, model.value]:
            for parameter in component.parameters():
                parameter.requires_grad_(False)
        before = copy.deepcopy(model)
        data, _ = collect(model, games=4, seed=29, player_counts=[3], deck_counts=[1],
                          train_war=True, war_style=0, imitation=True, phase="war")
        optimizer = torch.optim.Adam(model.parameters(), lr=0.01)
        optimize(model, optimizer, data, imitation=True, epochs=1, minibatch=128)
        batch = engine.Batch([33, 34], [3, 4], [1, 2], False, 0)
        obs, mask, _, _, _ = observe(batch)
        for actual, expected in zip(model(torch.from_numpy(obs), torch.from_numpy(mask)),
                                    before(torch.from_numpy(obs), torch.from_numpy(mask))):
            torch.testing.assert_close(actual, expected, rtol=0, atol=0)
        self.assertFalse(torch.equal(model.war.policy.weight, before.war.policy.weight))
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "model.pt"
            save(model, path, True, {})
            restored, _ = load(path)
            for key, value in model.state_dict().items():
                torch.testing.assert_close(value, restored.state_dict()[key], rtol=0, atol=0)
            self.assertTrue(parity(restored, path)["actions_identical"])
            exported = json.loads(path.with_suffix(".json").read_text())
            exported["trunk"][0]["weight"][0].pop()
            path.with_suffix(".json").write_text(json.dumps(exported))
            with self.assertRaises(ValueError):
                engine.NativeModel(str(path.with_suffix(".json")))

    def test_search_benchmark_has_seeded_repeatable_actions(self):
        batch = engine.Batch([7, 9], [3, 4], [1, 2], False, 0)
        a = batch.baseline("simulate-128", [42, 43])
        b = batch.baseline("simulate-128", [42, 43])
        self.assertEqual(a, b)
        batch.step(a)

    def test_candidate_prior_export_and_learned_residual(self):
        model = CandidatePolicy(16)
        batch = engine.Batch([17, 19, 23, 29], [3, 4, 5, 6], [1] * 4, False, 0)
        for _ in range(15):
            obs, mask, teacher, _, _ = observe(batch)
            logits, _ = model(torch.from_numpy(obs), torch.from_numpy(mask))
            np.testing.assert_array_equal(logits.argmax(-1).detach().numpy(), teacher)
            batch.step(teacher.tolist())
        data, _ = collect(model, games=8, seed=42, player_counts=[3, 6], deck_counts=[1],
                          train_war=False, war_style=0, imitation=True, balanced=True, dagger=True)
        configs = (data['obs'][:, 15 * 111 + 2] * 15).round()
        totals = [data['weight'][configs == n].sum() for n in [3, 6]]
        torch.testing.assert_close(totals[0], totals[1])
        optimizer = torch.optim.Adam(model.parameters(), lr=0.01)
        optimize(model, optimizer, data, imitation=True, epochs=1, minibatch=128)
        self.assertGreater(float(model.candidate[2].weight.detach().abs().sum()), 0)
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'candidate.pt'
            save(model, path, False, {})
            restored, _ = load(path)
            self.assertIsInstance(restored, CandidatePolicy)
            self.assertTrue(parity(restored, path)['actions_identical'])
            exported = json.loads(path.with_suffix('.json').read_text())
            exported['candidate_schema'] += 1
            path.with_suffix('.json').write_text(json.dumps(exported))
            with self.assertRaises(ValueError):
                engine.NativeModel(str(path.with_suffix('.json')))

    def test_dagger_queries_search_on_learner_visited_states(self):
        data, _ = collect(CandidatePolicy(16), games=4, seed=22, player_counts=[3], deck_counts=[1],
                          train_war=False, war_style=0, imitation=True, dagger=True,
                          phase='rummy', teacher_style='search-16', suit_augmentation=True)
        self.assertTrue(data['mask'].gather(1, data['action'][:, None]).all())
        self.assertTrue(torch.isfinite(data['obs']).all())


if __name__ == "__main__":
    unittest.main()
