# Local publication operator

`lenso-marketplace-publisher` is a native operator executable owned by the
Catalog publisher. It calls the existing Directory transaction and does not
implement an alternative signing format, public HTTP API or direct SQL writer.
Removing the Catalog package removes this entrypoint.

The trusted boundary is a controlled local user or an approved CI environment
with access to the private database, configuration and signing-key input.
An actor argument is an audit identity checked against configured reviewers;
it is not remote authentication. Never expose this command to public request
parameters or let an untrusted caller edit its configuration. The public Worker
must not receive the database, reviewers or private key.

Configuration is a JSON file containing `database` (absolute private SQLite
path), `catalog_id`, `reviewers` (actor IDs), `key_id`, and `public_key_hex`.
The operator does not generate keys, choose production identities or create
parent directories. Prepare and protect those outside the command.

Build with `cargo build --locked --manifest-path
plugins/marketplace/catalog/Cargo.toml --bin lenso-marketplace-publisher`.
Invoke the built executable as follows:

```sh
lenso-marketplace-publisher operator.json initialize
lenso-marketplace-publisher operator.json publish reviewer 0 604800 < signing-key.bin
lenso-marketplace-publisher operator.json export
```

`initialize` explicitly creates a new database and refuses existing paths.
It creates no namespace, submission, approved release or key. A publication of
an empty new database is an explicit empty catalog; fixtures are never promoted
into official releases. Existing reviewed submissions must come from the
Catalog admission/review domain before publication.

`publish` requires an existing database, the exact expected revision, and
validity of 1–604800 seconds. It reads exactly 32 raw Ed25519 seed bytes from
stdin, checks the derived public key against configuration, and uses the current
system time. The process inherits the operator's local authority; secure CI
must obtain this key from its protected environment, not a repository file or
command-line argument. Do not persist stdin in logs.

The single stdout JSON receipt contains `envelope` (the exact signed UTF-8
string) and `digest`. Decode the string to bytes when uploading; do not parse and
reserialize the envelope. If stdout fails after commit, publication remains
durable. Run `export` and reconcile the receipt; retrying the old expected
revision is rejected. Export can return an expired historical publication for
recovery and does not claim that it is currently acceptable to consumers.

Renewal is another `publish` using the current revision. The database retains
publication history and audit records. A timestamp or key mismatch is not
permission to reset that history. Back up this publisher database independently
of consumer D1/R2 state.

This executable does not upload to R2, mutate the D1 publication pointer,
configure renewal scheduling, or deploy Workers. Those operations must preserve
create-only object identity and compare-and-swap publication, and verify the
public consumer receipt before declaring an end-to-end publication complete.
The production key custodian and initial reviewed catalog remain launch inputs.
