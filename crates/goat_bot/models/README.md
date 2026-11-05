# Bundled rummy scorer

`rummy.json` contains the schema-1 candidate scorer and heuristic prior from
`training/runs/one-deck-v3/best.json`, the selected one-deck checkpoint. It omits
the observation trunk, global policy head, value head, and unused war network;
those components are only needed by the training pipeline. The scorer's weights
are unchanged. Loading and scoring need no Python runtime or external files.

Source checkpoint SHA-256: `5368307224f11b31274d6d28310d206755e1392aec7d7f2e74e80ce16dfb2ffa`

Bundled asset SHA-256: `1be2176006e592915d1186bfeda0cb7a22dfc31a8d548d510d99527edd617ba1`

The server uses `Selected::default()`. War searches 64 worlds per legal move,
subject to 512 total rollouts, and changes the adaptive move only for an estimated
loss reduction greater than 0.10. Rummy uses 16 worlds per move, up to 1,024
rollouts, and cached two-player proofs up to 26 cards and 200,000 nodes.
Multi-deck games retain `AdaptSimulate`.

See [the war results](../../../training/WAR.md) and
[the rummy results](../../../training/ADVANCED.md) for evaluation scope.
