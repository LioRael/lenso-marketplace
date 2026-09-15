# Recovery proof result

Passed on 2026-09-14T17:02:47.704533+00:00. Worker version `7d6723d2-8b63-4b07-b225-2007ef369500` serves the restored public consumer state at the isolated [recovery proof origin](https://lenso-marketplace-recovery-proof.lenso.workers.dev/api/marketplace/v1/snapshot).

| Check | Result |
| --- | --- |
| Paired backup restoration / public listing | 200 |
| Exact signed revision-8 envelope bytes | 200; SHA-256 `4c6f2a7010d3adf46ca0aa2a99958f2c4db3dd61c5494804e9ad65797b3f40c9` |
| Unexpired signed revision-1 publication | 503; accepted checkpoint remains 8 |
| Restore current revision-8 publication | 200; exact original envelope bytes |
| Remove historical identities in copied accepted object | 503; accepted pointer unchanged |
| Restore exact accepted object | 200; checkpoint 8 with all 161 identities |
| Source D1 rows and referenced R2 objects | Unchanged; every source query reports zero rows written |

Six live endpoint checks and five independent durable-state observations passed. Both target application rows finish identical to the frozen source rows. The restored accepted object has SHA-256 `a1571e4313787e632a25ea86866b8ccfe74bee33a4d3a01f897186be74e0b2c5`. The source and recovery proof resources remain intact for review.

The new workers.dev route initially returned an edge 404 while propagating. That attempt made no proof mutation and is retained in `evidence/initial-route-propagation.json`; the subsequent successful run is in `evidence/qualification.json`.

The signed revision-8 fixture expires at **2026-09-14 17:22:03 UTC**. Its intended post-expiry behavior is 503. These receipts qualify the recorded experimental public consumer backup/restore only; they do not qualify private publisher backups, production signing custody, production RPO/RTO, or a production rollout.
