# Task

**Status: Normative.** A Task is a unit of work and context in the execution DAG. It may be created by a framework or human and can have parent/child tasks, assigned agents, desired outcome, status, and related branch/workspace/checkpoint IDs.

Pong stores task identity, lifecycle, and provenance; it does not schedule or assign work. Task status is an observed or declared fact and may lag a framework's internal state. Operations, messages, artifacts, and checkpoints link to a task for filtering and recovery.

