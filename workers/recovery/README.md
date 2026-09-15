# Isolated consumer backup and recovery proof

This deployment restores the complete public consumer state into a new D1 database and R2 bucket. It uses the existing public `../worker.mjs`, the final G3 Wasm artifact, and the dedicated G3 test trust key. It does not deploy the privileged CAS proof wrapper or change the G3 source, `catalog.lenso.dev`, or any production configuration.

| Resource | Recovery proof |
| --- | --- |
| Worker | `lenso-marketplace-recovery-proof` |
| Public origin | `https://lenso-marketplace-recovery-proof.lenso.workers.dev` |
| D1 database | `lenso-marketplace-recovery-proof` / `256d0844-cff5-4ead-8770-ab62c7f181d1` |
| R2 bucket | `lenso-marketplace-recovery-proof` |
| Source D1 (read only) | `2b913921-3f0a-43b4-ae8f-cb219c21db81` |
| Source R2 (read only) | `lenso-marketplace-g3-proof` |
| Catalog / signing key ID | `workers-g3-proof` / `test-key` |
| Wasm SHA-256 | `d4ff79a9c7b6d6c2b8543f4be52fc9e2780d1687454d0dd55d19c52d03220faa` |

The frozen backup contains both complete application pointer rows, the exact 351-byte signed revision-8 envelope, and the 16,469-byte accepted object containing revision 8 and all 161 historical release identities. The whole accepted object's SHA-256 is checked against its immutable key; its envelope token is checked against both pointer rows. Copying only the envelope would lose the historical identity and rollback protections.

## Recorded procedure

Run commands from `plugins/marketplace/workers`. The checked-in config and script are deliberately pinned to this proof's resource identities. This is an executed experiment, not a general production restore tool. The receipt files are immutable audit inputs: do not replay against the retained resources, delete evidence to bypass replay refusal, or reuse these test signatures after expiry.

1. Confirm the source publication and accepted checkpoint are frozen at revision 8, and verify the final Wasm hash. Inventory the proposed target names and require that all are unused. The `*-inventory-before.json` receipts record this preflight.
2. Run `python3 recovery/recovery.py backup FROZEN_REVISION_8 VALID_SIGNED_REVISION_1`. Source D1 calls are restricted to `SELECT` with `rows_written == 0`; R2 source calls only read. Backup asserts exact frozen envelope bytes and verifies complete checkpoint content hashes.
3. Create new resources with `pnpm exec wrangler d1 create lenso-marketplace-recovery-proof --location apac --update-config=false` and `pnpm exec wrangler r2 bucket create lenso-marketplace-recovery-proof --location apac`. Preserve the returned database ID in the isolated config and script. Creation receipts are retained.
4. Apply the explicit schema with `pnpm exec wrangler d1 migrations apply lenso-marketplace-recovery-proof --remote --config recovery/wrangler.jsonc`. This migration is a byte-identical copy of the public consumer schema, with its SHA-256 recorded in `evidence/backup.json`.
5. Run `python3 recovery/recovery.py restore`. It requires empty migrated application tables, uploads both referenced objects to the new bucket, checks read-back hashes, and inserts both complete pointer rows. Deployment has not begun, so no consumer can observe the intermediate state.
6. Deploy with `pnpm exec wrangler deploy --config recovery/wrangler.jsonc`. No Rust rebuild or public source edits occur. Preserve `deploy.txt`, `deployment.json`, and module hashes. New workers.dev route propagation may precede availability; the first read-only 404 is retained separately and is not a business regression.
7. Run `python3 recovery/recovery.py exercise` once the new route is available. It verifies successful public reads and exact revision-8 bytes, points only the recovery publication at an unexpired signed revision 1, checks rejection and unchanged accepted pointer, and restores the original revision-8 pointer. It then corrupts only the copied accepted object's historical identities, checks fail-closed behavior without pointer replacement, restores the exact saved object, and verifies successful public reads again. Restoration runs in `finally` for these intentional fault probes; an uncertain platform write is not automatically retried.
8. Independently reread both source rows and source objects and assert equality with the frozen backup. Keep the new proof resources for review.

`evidence/qualification.json` is the machine-readable result, including response hashes, status codes, durable pointer observations, accepted-object hashes, and final source continuity. `evidence/restore.json` records the empty destination, inserted rows, and verified object copies. `evidence/backup/` contains only public signed data and public consumer checkpoint history; it contains no signing secrets or private publisher database.

## Limits and operational prerequisites

This qualifies paired **public consumer** checkpoint and publication restoration for the recorded dedicated proof fixture. It does not qualify private publisher database backup, signing-key custody or recovery, production RPO/RTO, continuously consistent live backups, or a production endpoint. The source was frozen by the coordinator; operators need a publication freeze or a coherent snapshot procedure when taking a production paired backup.

All test envelopes expire at their signed expiry. The deployed proof is expected to fail closed after expiry; retained successful receipts are historical evidence, not a promise of indefinite availability. Renewal requires the existing authorized publisher, a reviewed higher signed revision, and its normal immutable publication process. Never mint a replacement trust root during recovery or lower the accepted checkpoint to make expired/old data pass.

The proof intentionally corrupts one target object to verify integrity enforcement. Production immutable objects must not be overwritten; recovery copies must be hash verified, checkpoint and publication must remain paired, and ambiguous writes must be reconciled from durable state before another attempt. These scripts do not implement a private publisher backup policy or replace production operator review.
