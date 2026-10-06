# Evidence bundles

A ceremony's journal is a hash chain: inside its store, a record that was
altered, removed or reordered no longer verifies. That proves consistency to
whoever holds the store. It proves nothing to a reviewer, an auditor or a
customer who holds a copy, because a copy has no store to vouch for it and a
chain says nothing about who wrote it.

An evidence bundle is the journal as a file somebody else can judge:

- the records, verified before they leave the store;
- the head they end at (version, digest, record count);
- an Ed25519 signature over that head by a key the operator holds, with the
  public key beside it.

The digest of the head commits to every record before it, so signing the head
signs the journal. The verifier recomputes the chain from the records,
recomputes the head, compares it with the signed one and checks the signature.
It needs the file and, to pin the signer, the public key. It needs no store,
no engine and no MADE account.

These commands are in `main` and unreleased.

## Create the key once

The plugin's `made-setup` creates one key per store, beside the private setup
configuration, owner-readable only, and prints the public key:

```text
MADE setup: evidence signing key at ~/.config/underpass-made/embedded/<digest>.evidence-key (owner-readable only).
MADE setup: evidence public key 3f1c…9a; give it to whoever verifies your exports.
```

Without the plugin:

```bash
made-mcp keygen /path/to/evidence.key
made-mcp public-key /path/to/evidence.key
```

`keygen` refuses to replace an existing file: an export is only evidence if
the key that signed it stays the same key. Rotate on purpose by moving the old
file aside, and tell your verifiers the new public key.

## Export

```bash
plugins/made/scripts/made-export-evidence.sh --ceremony pr-1 --out pr-1.evidence.json
# or, without the plugin:
made-mcp export-evidence /path/to/ceremonies.sqlite3 --ceremony pr-1 \
  --key /path/to/evidence.key --out pr-1.evidence.json
```

The export reads the whole stream, refuses a journal that does not verify,
and writes a JSON file under the schema
`underpass.made.signed-evidence-bundle.v1`. It never overwrites a file. The
receipt names the ceremony, the head version, the record count and the public
key.

## Verify, anywhere

```bash
made-mcp verify-evidence pr-1.evidence.json --public-key 3f1c…9a
```

```text
ceremony `pr-1`: 9 records through version 9
chain: intact
head: the records end where the bundle says
signature: valid under public key 3f1c…9a
key: the expected public key
verdict: sound
```

Three answers, on their own, because they fail for different reasons:

| Line | When it fails | What it means |
|:--|:--|:--|
| chain | a record was altered, removed or reordered | the records are not the journal that was written |
| head | the records do not reach the signed head | the file was rewritten after signing |
| signature | the signature does not verify under the carried key | a forged or mismatched attestation |
| key | `--public-key` names another key | signed by somebody else's key |

Exit code `0` is sound, `1` is not sound, `2` is a file that cannot be judged
at all.

## What this proves, and what it does not

- The bundle proves the records are the ones the key holder's store held when
  the export was made, unaltered since. It does not prove that what the records
  say about the world is true: a step completed by a host is still the host's
  claim, as everywhere in MADE.
- The key is a file on the operator's machine. Whoever can read it can sign.
  Keep it with the other private setup configuration; back it up through a
  channel you would trust with a password.
- A verifier who does not pin `--public-key` learns that the file is
  internally consistent and signed by *some* key; pinning is what ties it to
  a person or an organization.
