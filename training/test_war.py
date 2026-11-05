import copy
import unittest

from war import summarize


class WarEvaluationTests(unittest.TestCase):
    def fixture(self):
        rows = []
        for name, scale in [("adapt", 1.), ("candidate", .5)]:
            for players in [3, 6]:
                rows.append({"variant": name, "players": players, "opponent": "adapt",
                             "games": players * 4, "goat_rate": scale / players,
                             "cluster_losses": [scale / players] * 4,
                             "truncated": 0, "rollouts": 0})
        return {"complete": True, "job": {"seeds": 4, "variants": {"adapt": {}, "candidate": {}}},
                "results": rows}

    def test_player_normalization_and_paired_relative_reduction(self):
        result = summarize(self.fixture())
        self.assertAlmostEqual(result["adapt"]["goat_rate"], 2/9)
        self.assertAlmostEqual(result["adapt"]["normalized_score"], 1.)
        self.assertAlmostEqual(result["candidate"]["normalized_score"], .5)
        self.assertAlmostEqual(result["candidate"]["relative_reduction"], .5)
        for bound in result["candidate"]["relative_reduction_ci95"]:
            self.assertAlmostEqual(bound, .5)

    def test_identical_policies_have_zero_paired_uncertainty(self):
        data = self.fixture()
        for i in range(2):
            row = copy.deepcopy(data["results"][i])
            row["variant"] = "candidate"
            data["results"][i+2] = row
        result = summarize(data)["candidate"]
        self.assertEqual(result["difference_ci95"], [0., 0.])

    def test_partial_evaluations_are_not_summarized(self):
        data = self.fixture()
        data["complete"] = False
        with self.assertRaises(ValueError):
            summarize(data)


if __name__ == "__main__":
    unittest.main()
