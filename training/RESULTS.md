> Historical experiment report. For current default serving and supported
> training commands, see [README.md](README.md). Experimental serving flags
> and superseded policies described below have been removed.

# Initial training results — 2026-10-18

For the subsequent one-deck improvements and selected bot, see
[ONE_DECK.md](ONE_DECK.md).

The pipeline is implemented and has been run through imitation, rummy PPO, war
PPO, and joint PPO. The selected checkpoint improves two-deck games against the
simple baseline, but regresses in one-deck games. It remains an experimental,
opt-in bot, not a demonstrated replacement for the three-second search bot.

## Training and model selection

Runs used CPU training on an ARM64 Mac with 12 logical CPUs, four PyTorch
threads, and six Rust workers. Python was 3.14.6, PyTorch 2.14.1, and NumPy 2.5.4.
Each network has two 128-unit ReLU layers. Training covered 3–6 players and 1–2
decks. War and joint runs also used suit augmentation.

| Run | Training games | Simulated decisions | Training samples | Best normalized validation goat rate |
| --- | ---: | ---: | ---: | ---: |
| Rummy | 30,720 | 4,324,666 | 2,424,995 | **0.9425** |
| War, frozen rummy continuation | 8,960 | 2,847,943 | 1,070,553 | 0.9575 |
| Joint fine-tuning | 2,560 | 840,140 | 442,986 | 0.9575 |
| Total | **42,240** | **8,012,749** | **3,938,534** | |

Validation used 100 reserved seeds per player/deck configuration and every seat
rotation. A normalized rate of 1 is the symmetric simple-bot reference; lower
is better. The selected rummy checkpoint is PPO iteration 60, after 40 imitation
iterations. The war run selected iteration 20; the joint run did not improve on
its starting checkpoint. Those later runs were retained as experiments, not
promoted over the rummy checkpoint.

Selected artifacts:

- [Rust model](runs/rummy-v1/best.json), 6.9 MiB.
- [PyTorch checkpoint](runs/rummy-v1/best.pt), 2.1 MiB.
- [Configuration](runs/rummy-v1/config.json) and [training log](runs/rummy-v1/metrics.jsonl).
- [War log](runs/war-v1/metrics.jsonl) and [joint log](runs/joint-v1/metrics.jsonl).

These generated artifacts are local files under the ignored `training/runs/`
directory. Reproduction commands are below; model files are not checked into
source control automatically.

## Held-out results

Evaluation used 3,334 new seeds per configuration, starting at 2,000,000, and
rotated one learned candidate through every seat. All other seats used
`AdaptSimple`. The control replaced the candidate with `AdaptSimple` too.
Both candidates used the same adaptive war strategy.

There were **120,024 learned-candidate games and 120,024 matched control games**.
The rates below are the probability that the candidate becomes the goat.

| Players | Simple control | Learned, one deck | Learned, two decks | Relative reduction with two decks |
| --- | ---: | ---: | ---: | ---: |
| 3 | 33.33% | 37.56% | **25.56%** | 23.3% |
| 4 | 25.00% | 28.96% | **16.77%** | 32.9% |
| 5 | 20.00% | 23.41% | **14.84%** | 25.8% |
| 6 | 16.67% | 20.90% | **14.06%** | 15.7% |

Every two-deck improvement and every one-deck regression has a paired 95%
confidence interval excluding zero, clustering all seat rotations by seed.
See the [full evaluation](runs/rummy-v1/evaluation.json) for intervals and counts.
No illegal actions or action-limit failures occurred.

A smaller comparison used the existing rollout search with a deterministic
128-rollout budget, 100 seeds per configuration, and every seat rotation:

| Players | Decks | Learned candidate vs search opponents | Simple candidate vs the same search opponents |
| --- | --- | ---: | ---: |
| 3 | 1 | 29.33% | 21.33% |
| 3 | 2 | **13.00%** | 19.67% |
| 4 | 1 | 26.00% | 22.50% |
| 4 | 2 | **10.25%** | 18.00% |

Both two-deck paired improvements exclude zero at 95%. This benchmark is
**not** the existing three-second bot: it has a fixed 128-rollout budget and
retains the original search's restricted root moves and determinization bias.
See [search evaluation](runs/rummy-v1/search-evaluation.json). No strength claim
is made for 3 decks, 7–15 players, human opponents, or the full three-second bot.

## Runtime verification

- All **25 Rust tests** and **four Python tests** passed.
- Rust and PyTorch selected identical actions on **2,560 parity positions**
  covering both phases; maximum logit difference was about `1.91e-5`.
- A further **1,400 candidate games** with native Rust inference matched the
  PyTorch tournament's goat rates and mean episode lengths exactly, across
  3/4 players and 1/2 decks. [Native evaluation](runs/rummy-v1/native-evaluation.json).
- Isolated native inference over 1,000 decisions: **0.220 ms median**, **0.228 ms
  p95**, and **0.689 ms maximum** on this machine, excluding deliberate bot
  pacing and network/UI overhead. [Inference measurements](runs/rummy-v1/inference.json).
- The actual HTTP/SSE server loaded the exported model, served the frontend,
  and completed a three-bot live game through war, dreck redistribution, rummy,
  and the goat event in **164.29 seconds**, including normal bot pacing.
  [Live-game record](runs/live-smoke.json).

## Reproduce these runs

After the [setup instructions](README.md), use new output directories if the
original runs already exist:

```sh
.venv/bin/python training/train.py train \
  --output training/runs/rummy-v1 --players 3 4 5 6 --decks 1 2 \
  --games-per-iteration 256 --imitation-iterations 40 --imitation-epochs 4 \
  --ppo-iterations 80 --ppo-epochs 3 --validation-seeds 100 --validate-every 10

.venv/bin/python training/train.py train --phase war \
  --resume training/runs/rummy-v1/best.pt --output training/runs/war-v1 \
  --seed 100000 --players 3 4 5 6 --decks 1 2 --games-per-iteration 128 \
  --imitation-iterations 30 --imitation-epochs 4 --ppo-iterations 40 \
  --ppo-epochs 3 --validation-seeds 100 --validate-every 10 --suit-augmentation

.venv/bin/python training/train.py train --phase joint \
  --resume training/runs/war-v1/best.pt --output training/runs/joint-v1 \
  --seed 200000 --players 3 4 5 6 --decks 1 2 --games-per-iteration 128 \
  --imitation-iterations 0 --ppo-iterations 20 --ppo-epochs 3 \
  --validation-seeds 100 --validate-every 10 --suit-augmentation

.venv/bin/python training/train.py evaluate \
  --model training/runs/rummy-v1/best.pt --players 3 4 5 6 --decks 1 2 \
  --seeds 3334 --opponents simple --output training/runs/rummy-v1/evaluation.json

GOAT_MODEL=training/runs/rummy-v1/best.json cargo run --release -p goat_server
```

The final command serves the bot at <http://127.0.0.1:9402>. Further work should
prioritize one-deck strength and robustness to other opponent styles before
promoting this model as the default bot.
