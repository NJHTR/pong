# Branch

**Status: Normative.** A Branch is a named, project-scoped movable reference to a commit in the version DAG. It is not an agent, workspace, or task.

Branch names are unique within a project namespace and advance through compare-and-swap on the expected old head. A workspace materializes one branch at a time, but a branch can be materialized by many read-only workspaces and one or more independent writable workspaces. The latter requires explicit merge or rebase conflict handling; there is no hidden shared filesystem.

Branch creation records the source head. Checkout changes a workspace materialization and lease, not the branch's history. Deleting a branch requires authorization and leaves retained commits reachable from audit/event retention until garbage-collection policy permits removal.

