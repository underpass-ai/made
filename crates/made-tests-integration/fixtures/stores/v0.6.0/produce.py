#!/usr/bin/env python3
"""Produce a small upgrade fixture through the published v0.6.0 MCP binary."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import selectors
import sqlite3
import subprocess

parser = argparse.ArgumentParser()
parser.add_argument('--binary', type=Path, required=True)
parser.add_argument('--output', type=Path, required=True)
args = parser.parse_args()
binary = args.binary.resolve()
version = subprocess.check_output([str(binary), '--version'], text=True).strip()
assert version == 'made-mcp 0.6.0 (embedded store: sqlite)', version
expected = '905410b881851ea1e9190694e6ff5f8ed99067d872f902f725f6ea104b6f896d'
digest = hashlib.sha256(binary.read_bytes()).hexdigest()
assert digest == expected, 'This producer requires the published Linux arm64 asset'
args.output.mkdir(parents=True, exist_ok=True)
store = (args.output / 'ceremonies.sqlite3').resolve()
assert not store.exists(), 'Never overwrite a historical fixture'
env = {key: value for key, value in os.environ.items() if not key.startswith('MADE_')}
env.update(MADE_MCP_BACKEND='embedded', MADE_MCP_STORE_PATH=str(store))
process = subprocess.Popen([str(binary)], env=env, stdin=subprocess.PIPE,
                           stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
selector = selectors.DefaultSelector()
selector.register(process.stdout, selectors.EVENT_READ)
calls = []


def call(name, arguments):
    request = {'jsonrpc': '2.0', 'id': len(calls) + 1, 'method': 'tools/call',
               'params': {'name': name, 'arguments': arguments}}
    process.stdin.write(json.dumps(request) + '\n')
    process.stdin.flush()
    assert selector.select(timeout=30), f'timeout: {name}'
    response = json.loads(process.stdout.readline())
    assert response['id'] == request['id'] and 'error' not in response, response
    assert not response['result'].get('isError'), response
    calls.append({'request': request, 'response': response})
    return response['result']['structuredContent']


definition = '''version: "1.0"
name: upgrade_v060
states:
  - {id: WORKING, initial: true}
  - {id: CLOSED, terminal: true}
transitions:
  - {from: WORKING, to: CLOSED, trigger: finish, guards: [done]}
guards:
  done: {type: automated, check: "step_status:work:COMPLETED"}
steps:
  - {id: work, state: WORKING, handler: noop}
roles:
  - {id: WORKER, allowed_actions: [work, finish]}
'''
manifest = {'writer_version': version, 'writer_sha256': digest,
            'writer_asset': 'made-mcp-v0.6.0-aarch64-unknown-linux-gnu',
            'source_tag': 'v0.6.0', 'sessions': {}}
try:
    call('made_publish_ceremony_definition', {'definition_yaml': definition})
    for label in ['completed', 'pending', 'claimed']:
        identifier = 'v060-' + label
        scoped = {'ceremony_id': identifier}
        call('made_start_published_ceremony', {**scoped, 'ceremony': 'upgrade_v060',
             'version': '1.0', 'actor_id': 'upgrade-fixture', 'actor_kind': 'service'})
        if label != 'pending':
            claim = call('made_claim_ceremony_step', {**scoped, 'step_id': 'work',
                         'actor_kind': 'agent', 'idempotency_key': identifier,
                         'lease_owner_id': 'v060-writer', 'lease_ttl_ms': 300000})
            manifest['sessions'][identifier] = {'claim_fence': claim['claim_fence']}
        else:
            manifest['sessions'][identifier] = {}
        if label == 'completed':
            call('made_complete_ceremony_step', {**scoped, 'step_id': 'work',
                 'actor_kind': 'agent', 'status': 'completed',
                 'claim_fence': claim['claim_fence'], 'output': {'fixture': 'v0.6.0'}})
            call('made_apply_ceremony_transition', {**scoped, 'trigger': 'finish',
                 'actor_kind': 'agent'})
        records = call('made_read_ceremony_events', {**scoped, 'limit': 1000})['records']
        assert call('made_verify_ceremony_journal', scoped)['intact']
        manifest['sessions'][identifier].update(
            records=records, instance=call('made_get_ceremony_instance', scoped))
finally:
    process.stdin.close()
    try:
        assert process.wait(timeout=10) == 0, process.stderr.read()
    finally:
        if process.poll() is None:
            process.kill()
            process.wait()
        selector.close()

with sqlite3.connect(store) as connection:
    connection.execute('PRAGMA wal_checkpoint(TRUNCATE)')
manifest['store_sha256'] = hashlib.sha256(store.read_bytes()).hexdigest()
manifest['store_bytes'] = store.stat().st_size
(args.output / 'manifest.json').write_text(json.dumps(manifest, indent=2) + '\n')
(args.output / 'requests-responses.json').write_text(json.dumps(calls, indent=2) + '\n')
(args.output / 'definition.yaml').write_text(definition)
print(json.dumps({key: manifest[key] for key in ['writer_version', 'writer_sha256',
                                              'store_sha256', 'store_bytes']}, indent=2))
