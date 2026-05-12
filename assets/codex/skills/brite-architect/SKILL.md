---
name: brite-architect
description: Design Litebrite work graphs and brite hierarchies for project planning. Use when Codex is asked to plan, decompose, create, restructure, or refine Litebrite epics, features, tasks, dependencies, sequencing, or leaf task descriptions.
---

# Brite Architect

## Overview

Create Litebrite items that let runner agents start work without re-planning. Prefer small, well-sequenced leaf tasks with enough handoff context to implement and verify one unit of work.

## Graph Model

- Use parent-child hierarchy for decomposition: epic -> feature -> task. A child should be a smaller part of its parent, not merely related work.
- Use sibling blocking dependencies for sequencing. If task B should not start until task A lands, make A block B instead of hiding order in prose.
- Keep leaves executable by one runner in one focused pass. If a leaf needs design choices, unknown research, or multiple ownership areas, split it or add a preceding planning task.
- Put shared context on the parent only when children genuinely share it. Put implementation-specific details on the leaf that needs them.
- Do not over-model status meetings, review ownership, or shipping decisions as runner tasks unless the project explicitly requires them.

## Planning Workflow

1. Identify the user-visible outcome and create or update the smallest epic that owns it.
2. Split the epic by deliverable behavior or architectural slice, not by vague activity type.
3. Convert each slice into leaf tasks that name concrete files, commands, or interfaces when known.
4. Add blocking deps only for real sequencing constraints: generated artifacts, API contracts, migrations, shared foundations, or tests that require prior behavior.
5. Check each leaf from a runner's perspective: it should say what to build, where to look, how to know it is done, and what not to expand into.

## Leaf Description Template

Use this compact structure for task descriptions:

```text
Goal: <one-sentence outcome>
Context: <why this exists and the important existing code/docs>
Requirements: <specific behavior, files, commands, or constraints>
Acceptance: <observable done state, including tests or manual checks>
Out of scope: <nearby work the runner should not take on>
```

Keep descriptions terse but complete. A good leaf lets an agent claim it, read the named context, implement, commit, close, and stop.

## Quality Bar

- Every open child should either be independently runnable or blocked by the prerequisite that makes it runnable.
- Each dependency should explain actual order, not priority.
- Leaf tasks should avoid phrases like "improve", "clean up", or "handle edge cases" unless they name the concrete behavior to change.
- The graph should expose parallelism: unrelated siblings should not block each other.
