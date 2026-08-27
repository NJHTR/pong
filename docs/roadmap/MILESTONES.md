# Milestones

| Milestone | Exit criteria | Dependencies | Status |
| --- | --- | --- | --- |
| M0 Architecture baseline | Required docs, ADRs, constitution, review, open questions | None | Complete |
| M1 Durable local repository | CAS, metadata/WAL, event append, recovery fixtures | M0 | Broad local/host evidence; gate not passed |
| M2 Workspace and snapshot | Local driver, leases, tree snapshot, safe environment facts | M1 | Bounded internal slice frozen until M1 passes; gate not passed |
| M3 Operation ledger | Generic envelope, explicit API, filesystem/process capture, artifact refs | M1, M2 | Bounded internal slice frozen until M1 passes; gate not passed |
| M4 Version collaboration | Branch/CAS refs, commits, diff/log, agent registry, conflict records | M2, M3 | Planned |
| M5 Recovery and replay | Checkpoint restore, rollback plan, replay policy, approval and idempotency | M3, M4 | Planned |
| M6 v0.1 contracts | SDK/protocol/CLI, compatibility fixtures, local release | M4, M5 | Planned |
| M7 Adapter pilot | Two framework adapters plus generic runtime with declared gaps | M6 | Planned |
| M8 Drivers and sync | Container/remote drivers, artifact scaling, optional replication | M7 | Research gated |
