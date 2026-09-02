---
description: Execute an implementation plan with rigorous validation loops
---

# Plan Execution Workflow

Use this workflow to execute a previously created implementation plan with high reliability.

## Phase 1: Preparation (LOAD & PREPARE)
- **Load Plan**: Read the plan file and identify tasks, patterns, and validation commands.
- **Environment**: Detect the project runner (`npm`, `uv`, etc.) and available scripts.
- **Git State**: Ensure you are on a feature branch (not `main` with uncommitted changes).

## Phase 2: Execution (Task Loop)
For each task in the plan:
1. **Read Context**: Understand the pattern to mirror.
2. **Implement**: Make the change exactly as specified, following codebase conventions.
3. **Validate**: Run the type-check or syntax command immediately. **Never** move to the next task if validation fails.

## Phase 3: Verification (Full Suite)
Once all tasks are complete:
- **Static Analysis**: Run full lint and type-checking.
- **Tests**: Write or update unit tests for the changes. All tests must pass.
- **Build**: Ensure the project builds successfully.
- **Integration**: Perform manual or automated health checks.

## Phase 4: Reporting
- **Generate Report**: Save a summary to `.agent/PRPs/reports/`.
- **Archive Plan**: Move the completed plan to `.agent/PRPs/plans/completed/`.
- **Status**: Mark the feature as complete in any tracking documents.

## Failure Handling
- If a check fails, read the error carefully, fix the root cause, and re-validate.
- Do not accumulate broken state.
