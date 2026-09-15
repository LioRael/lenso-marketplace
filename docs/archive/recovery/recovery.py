#!/usr/bin/env python3
"""Dedicated consumer recovery proof. Never writes the G3 source or production.
OAuth is read in memory from Wrangler's existing local configuration.
"""
import base64
import hashlib
import json
import pathlib
import re
import subprocess
import sys
import urllib.parse
import urllib.request
from datetime import datetime, timezone

ROOT = pathlib.Path(__file__).resolve().parent
SOURCE_DB = '2b913921-3f0a-43b4-ae8f-cb219c21db81'
SOURCE = 'lenso-marketplace-g3-proof'
TARGET = 'lenso-marketplace-recovery-proof'
TARGET_DB = '256d0844-cff5-4ead-8770-ab62c7f181d1'
CATALOG = 'workers-g3-proof'
ARTIFACT = 'd4ff79a9c7b6d6c2b8543f4be52fc9e2780d1687454d0dd55d19c52d03220faa'
API = 'https://api.cloudflare.com/client/v4/'

def sha(data): return hashlib.sha256(data).hexdigest()
def save(name, value):
    path = ROOT / 'evidence' / name
    path.write_text(json.dumps(value, indent=2) + '\n')
def read(name): return json.loads((ROOT / 'evidence' / name).read_text())
def now(): return datetime.now(timezone.utc).isoformat()

def api(path, body=None, method=None, raw=False):
    config = (pathlib.Path.home() / 'Library/Preferences/.wrangler/config/default.toml').read_text()
    token = json.loads(re.search(r'^oauth_token\s*=\s*("[^"\n]*")', config, re.M).group(1))
    headers = {'Authorization': 'Bearer ' + token}
    if body is not None:
        headers['Content-Type'] = 'application/json'
        body = json.dumps(body).encode()
    request = urllib.request.Request(API + path, data=body, headers=headers, method=method)
    with urllib.request.urlopen(request, timeout=90) as response:
        data = response.read()
    if raw: return data
    result = json.loads(data)
    assert result['success'], 'Cloudflare operation failed'
    return result['result']

def account():
    result = [a['id'] for a in api('accounts') if a['name'] == 'LioRael']
    assert len(result) == 1
    return result[0]

def query(account_id, database, sql, params=None, write=False):
    if database == SOURCE_DB: assert not write and sql.startswith('SELECT '), 'source is read-only'
    if write:
        config = json.loads((ROOT / 'wrangler.jsonc').read_text())
        assert database == config['d1_databases'][0]['database_id'] == TARGET_DB
    result = api(f'accounts/{account_id}/d1/database/{database}/query', {'sql': sql, 'params': params or []})
    assert all(r['success'] for r in result)
    if not write: assert all(r['meta']['rows_written'] == 0 for r in result)
    return result

def rows(account_id, database):
    return {table: query(account_id, database, f'SELECT * FROM {table} ORDER BY catalog_id')[0]
            for table in ['marketplace_publications', 'marketplace_accepted']}

def obj(account_id, bucket, key):
    assert bucket in [SOURCE, TARGET]
    return api(f'accounts/{account_id}/r2/buckets/{bucket}/objects/{urllib.parse.quote(key, safe="")}', raw=True)

def put(key, file):
    assert not key.startswith('/') and '..' not in key.split('/')
    subprocess.run(['pnpm', 'exec', 'wrangler', 'r2', 'object', 'put', TARGET + '/' + key,
                    '--file', str(file), '--remote', '--config', str(ROOT / 'wrangler.jsonc')], cwd=ROOT.parent, check=True, stdout=subprocess.DEVNULL)

def checked_backup():
    backup = read('backup.json')
    for item in backup['objects']:
        assert sha((ROOT / 'evidence/backup' / item['file']).read_bytes()) == item['sha256']
    return backup

def backup(frozen, old):
    assert not (ROOT / 'evidence/backup.json').exists(), 'refuse overwrite of frozen backup receipt'
    account_id = account()
    snapshot = rows(account_id, SOURCE_DB)
    publication = snapshot['marketplace_publications']['results']
    accepted = snapshot['marketplace_accepted']['results']
    assert len(publication) == len(accepted) == 1
    publication, accepted = publication[0], accepted[0]
    assert publication['catalog_id'] == accepted['catalog_id'] == CATALOG
    assert publication['revision'] == 8
    objects = []
    for file, key in [('publication-8.json', publication['object_key']), ('accepted-8.json', accepted['object_key'])]:
        content = obj(account_id, SOURCE, key)
        (ROOT / 'evidence/backup' / file).write_bytes(content)
        objects.append({'file': file, 'key': key, 'bytes': len(content), 'sha256': sha(content)})
    envelope = (ROOT / 'evidence/backup/publication-8.json').read_bytes()
    assert envelope == pathlib.Path(frozen).read_bytes()
    assert publication['digest'] == 'sha256:' + sha(envelope)
    state_bytes = (ROOT / 'evidence/backup/accepted-8.json').read_bytes()
    state = json.loads(state_bytes)
    assert accepted['object_key'] == f'accepted/{CATALOG}/{sha(state_bytes)}.json'
    assert state['envelope'].encode() == envelope and state['token'] == accepted['token'] == publication['digest']
    assert state['checkpoint']['revision'] == 8
    first = pathlib.Path(old).read_bytes()
    payload = json.loads(base64.b64decode(json.loads(first)['payload_base64']))
    assert payload['catalog_id'] == CATALOG and payload['revision'] == 1
    assert payload['expires_at'] > int(datetime.now().timestamp())
    (ROOT / 'evidence/backup/rollback-1.json').write_bytes(first)
    save('backup.json', {'timestamp': now(), 'account_id': account_id, 'source_database_id': SOURCE_DB,
        'source_bucket': SOURCE, 'source_rows': snapshot, 'objects': objects,
        'checkpoint_revision': 8, 'history_size': len(state['checkpoint']['release_identities']),
        'rollback_fixture_sha256': sha(first), 'rollback_fixture_expires_at': payload['expires_at'],
        'artifact_sha256': ARTIFACT, 'migration_sha256': sha((ROOT / 'migrations/0001_public_reads.sql').read_bytes())})
    print(json.dumps({'backup': 'complete', 'revision': 8, 'history_size': len(state['checkpoint']['release_identities']), 'objects': objects}))

def restore():
    backup = checked_backup()
    config = json.loads((ROOT / 'wrangler.jsonc').read_text())
    assert config['name'] == TARGET and config['main'] == '../worker.mjs'
    database = config['d1_databases'][0]['database_id']
    assert config['d1_databases'][0]['database_name'] == TARGET and database == TARGET_DB
    assert config['r2_buckets'][0]['bucket_name'] == TARGET
    assert sha((ROOT.parent / 'pkg/lenso_marketplace_workers_host_bg.wasm').read_bytes()) == ARTIFACT
    before = rows(backup['account_id'], database)
    assert all(not r['results'] for r in before.values()), 'target must have empty migrated tables'
    receipts = []
    for item in backup['objects']:
        put(item['key'], ROOT / 'evidence/backup' / item['file'])
        assert sha(obj(backup['account_id'], TARGET, item['key'])) == item['sha256']
    for table, columns in [('marketplace_publications', ['catalog_id', 'revision', 'object_key', 'digest']),
                           ('marketplace_accepted', ['catalog_id', 'token', 'object_key'])]:
        row = backup['source_rows'][table]['results'][0]
        receipts.extend(query(backup['account_id'], database, f'INSERT INTO {table} ({",".join(columns)}) VALUES ({",".join("?" for _ in columns)})', [row[c] for c in columns], write=True))
    after = rows(backup['account_id'], database)
    assert all(after[t]['results'] == backup['source_rows'][t]['results'] for t in after)
    save('restore.json', {'timestamp': now(), 'database_id': database, 'bucket': TARGET, 'before': before, 'writes': receipts, 'after': after, 'objects_verified': backup['objects']})
    print(json.dumps({'restored': True, 'database_id': database, 'revision': 8}))

def public_check(label, expected, raw=False):
    path = '/api/marketplace/v1/snapshot' if raw else '/api/marketplace/v1/plugins'
    script = """const response = await fetch(process.argv[1], {signal:AbortSignal.timeout(15000), redirect:'error'});
const bytes=Buffer.from(await response.arrayBuffer());
console.log(JSON.stringify({status:response.status,headers:Object.fromEntries(response.headers),body_base64:bytes.toString('base64')}));"""
    result = json.loads(subprocess.check_output(['node', '--input-type=module', '-e', script,
        'https://' + TARGET + '.lenso.workers.dev' + path], text=True))
    body = base64.b64decode(result.pop('body_base64'))
    record = {'label': label, 'timestamp': now(), 'expected_status': expected, **result,
              'body_sha256': sha(body), 'body_bytes': len(body), 'passed': result['status'] == expected}
    if expected != 200: record['body'] = body.decode()
    if raw and expected == 200:
        record['exact_frozen_bytes'] = body == (ROOT / 'evidence/backup/publication-8.json').read_bytes()
        record['passed'] &= record['exact_frozen_bytes']
    return record


def exercise():
    backup = checked_backup()
    config = json.loads((ROOT / 'wrangler.jsonc').read_text())
    assert config['name'] == TARGET and config['main'] == '../worker.mjs'
    assert config['r2_buckets'][0]['bucket_name'] == TARGET
    database = config['d1_databases'][0]['database_id']
    assert database == TARGET_DB and config['d1_databases'][0]['database_name'] == TARGET
    account_id = backup['account_id']
    receipt = {'timestamp': now(), 'base': 'https://' + TARGET + '.lenso.workers.dev',
               'database_id': database, 'bucket': TARGET, 'passed': False, 'checks': [], 'durable': []}
    assert not (ROOT / 'evidence/qualification.json').exists(), 'refuse replay or overwrite of evidence'
    def checkpoint(label, intact=True):
        current = rows(account_id, database)
        expected = backup['source_rows']['marketplace_accepted']['results']
        assert current['marketplace_accepted']['results'] == expected, 'checkpoint pointer changed'
        key = expected[0]['object_key']
        content = obj(account_id, TARGET, key)
        if intact:
            assert sha(content) == backup['objects'][1]['sha256'], 'complete checkpoint object changed'
            assert json.loads(content)['checkpoint']['revision'] == 8
        record = {'label': label, 'rows': current, 'accepted_object_sha256': sha(content)}
        receipt['durable'].append(record)
        save('qualification.json', receipt)
        return record
    def check(label, status, raw=False):
        result = public_check(label, status, raw)
        receipt['checks'].append(result)
        save('qualification.json', receipt)
        assert result['passed'], label + ' failed; inspect qualification.json; no automatic replay'
    def publication(row):
        return query(account_id, database,
            'UPDATE marketplace_publications SET revision=?,object_key=?,digest=? WHERE catalog_id=?',
            [row['revision'], row['object_key'], row['digest'], CATALOG], write=True)
    expected_row = backup['source_rows']['marketplace_publications']['results'][0]
    check('restored paired backup serves revision 8', 200)
    check('restored signed envelope has exact frozen bytes', 200, True)
    checkpoint('restored paired backup')
    first = ROOT / 'evidence/backup/rollback-1.json'
    old_payload = json.loads(base64.b64decode(json.loads(first.read_bytes())['payload_base64']))
    assert old_payload['expires_at'] > int(datetime.now().timestamp()), 'rollback probe must use unexpired signed data'
    old_key = 'recovery-proof/rollback-1.json'
    put(old_key, first)
    assert obj(account_id, TARGET, old_key) == first.read_bytes()
    try:
        publication({'revision': 1, 'object_key': old_key, 'digest': 'sha256:' + sha(first.read_bytes())})
        check('valid older revision is rejected', 503)
        checkpoint('rollback rejection retains revision 8')
    finally:
        publication(expected_row)
    check('restore current revision 8 succeeds', 200, True)
    checkpoint('after rollback recovery')
    accepted = ROOT / 'evidence/backup/accepted-8.json'
    accepted_key = backup['source_rows']['marketplace_accepted']['results'][0]['object_key']
    corrupt = json.loads(accepted.read_bytes())
    assert len(corrupt['checkpoint']['release_identities']) == 161
    corrupt['checkpoint']['release_identities'] = {}
    corrupt_file = ROOT / 'evidence/accepted-corrupt.json'
    corrupt_file.write_text(json.dumps(corrupt, separators=(',', ':')))
    try:
        put(accepted_key, corrupt_file)
        check('tampered copied historical checkpoint fails closed', 503)
        checkpoint('tamper rejected without pointer replacement', intact=False)
    finally:
        put(accepted_key, accepted)
    assert obj(account_id, TARGET, accepted_key) == accepted.read_bytes()
    check('exact accepted object restoration succeeds', 200, True)
    final = checkpoint('final recovery state')
    assert final['rows']['marketplace_publications']['results'] == [expected_row]
    source_after = rows(account_id, SOURCE_DB)
    assert all(source_after[t]['results'] == backup['source_rows'][t]['results'] for t in source_after)
    for item in backup['objects']:
        assert sha(obj(account_id, SOURCE, item['key'])) == item['sha256']
    receipt['source_after'] = source_after
    receipt['source_unchanged'] = True
    receipt['passed'] = True
    receipt['completed_at'] = now()
    save('qualification.json', receipt)
    print(json.dumps({'passed': True, 'checks': len(receipt['checks']), 'source_unchanged': True}))


if __name__ == '__main__':
    if sys.argv[1] == 'backup': backup(*sys.argv[2:])
    elif sys.argv[1] == 'restore': restore()
    elif sys.argv[1] == 'exercise': exercise()
    else: raise SystemExit('usage: recovery.py backup FROZEN_8_JSON VALID_FIRST_JSON | restore | exercise')
