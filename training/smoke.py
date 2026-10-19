"""Start a three-bot, one-deck game on a local server and await its goat event."""
import argparse
from collections import Counter
import json
from pathlib import Path
import time
import urllib.request

if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--base-url', default='http://127.0.0.1:9402')
    parser.add_argument('--output', required=True)
    args = parser.parse_args()
    headers = {'Cookie':'USER_SECRET=codex-one-deck-smoke; USER_NAME=one-deck-smoke', 'Content-Type':'application/json'}
    def post(path, data=None):
        request = urllib.request.Request(args.base_url + path, method='POST', headers=headers,
            data=json.dumps(data).encode() if data is not None else b'')
        with urllib.request.urlopen(request, timeout=10) as response:
            return response.read()
    game = json.loads(post('/new_game'))
    request = urllib.request.Request(args.base_url + '/subscribe', headers=headers)
    counts = Counter()
    started = time.perf_counter()
    with urllib.request.urlopen(request, timeout=30) as stream:
        post('/apply_action?game_id=' + game, {'type':'start', 'numDecks':1})
        for line in stream:
            if time.perf_counter() - started > 600:
                raise TimeoutError('live game exceeded ten minutes')
            if not line.startswith(b'data:'):
                continue
            response = json.loads(line[5:])
            if response.get('gameId') != game:
                continue
            events = response.get('events', []) if response['type'] == 'replay' else [response.get('event')]
            for event in events:
                if not event:
                    continue
                counts[event['type']] += 1
                if event['type'] == 'goat':
                    result = {'game_id':game, 'completed':True, 'seconds':round(time.perf_counter()-started,2),
                              'events':dict(counts)}
                    Path(args.output).write_text(json.dumps(result,indent=2)+'\n')
                    print(json.dumps(result),flush=True)
                    raise SystemExit(0)
