# Git Compatibility

## Intent

Pong borrows Git's content addressing, immutable objects, refs, branches, parent-linked commits, and three-way merge concepts. It remains a distinct system because execution operations, environments, agent identity, and uncertain side effects are first-class.

## Mapping

| Git | Pong | Boundary |
|---|---|---|
| blob/tree | file/artifact CAS object | Pong adds provenance and sensitivity metadata |
| commit | semantic version node | includes workspace/environment/operation references |
| ref/branch | branch ref | CAS update plus workspace policy |
| index | staged state manifest | may include non-filesystem resources |
| worktree | workspace | physical location is abstracted |
| hook | runtime adapter/interceptor | hooks are advisory unless native |
| reflog | ref/audit events | retains actor and request context |

## Import/export

A Pong workspace may expose a Git worktree for source files. Git commits can be imported as source-only ancestors with provenance marked `git`. Pong commits can export a deterministic source tree and message, but execution/environment metadata is represented in sidecar manifests and cannot be losslessly encoded in ordinary Git.

## Interoperability rules

Git commands must not mutate `.pong` metadata behind Pong's back; such changes are detected as external modifications. A Git checkout updates the workspace projection and emits a reconciliation event. Git LFS pointers are treated as external artifact references and require an artifact adapter.

## Deliberate differences

`rollback` is dimension-specific, `replay` may involve side effects, operations can be unknown, and branch heads are guarded by leases/CAS. Pong never rewrites history as a normal conflict resolution mechanism.

