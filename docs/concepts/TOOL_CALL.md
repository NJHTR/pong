# Tool Call

**Status: Normative.** A ToolCall is the normalized invocation envelope nested in or referenced by an Operation. It identifies a capability-qualified tool/action, schema version, redacted input digest, resource claims, adapter/provider metadata, and output/error references.

A ToolCall describes the attempted interface; an Operation adds lifecycle, causality, before/after state, policy, and capture evidence. Framework adapters may retain native call data as an opaque artifact. Tool names are extensible and cannot be used alone to infer reversibility or authorization.

