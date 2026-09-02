---
description: Create a comprehensive implementation plan for a feature or task
---

# Implementation Planning Workflow

Use this workflow to transform a requirement into a battle-tested implementation plan.

## Phase 1: Strategic Focus
**ASK THE USER**: "Should this plan be **Dev-Focused** (technical patterns, code snippets, validation scripts) or **Generalized** (high-level objectives, documentation, process steps)?"

- If **Dev-Focused**: Emphasize exact file paths, code snippets to mirror, and rigorous validation commands.
- If **Generalized**: Emphasize functional requirements, user value, and high-level process steps.

## Phase 2: Discovery (DETECT & PARSE)
- **Identify project type**: Check for `package.json`, `pyproject.toml`, etc.
- **Understand the feature**: Formulate a user story (As a... I want to... So that...).
- **Find patterns**: Search the codebase for similar implementations to follow. Do **not** invent new patterns if existing ones work.

## Phase 3: Research & Design
- **Research**: Look up external documentation for libraries and tools being used.
- **Architect**: Define integration points and data flow.
- **UX Transformation**: Describe or visualize the "Before" vs "After" state.

## Phase 4: Generate Plan
Create a plan file at `.agent/PRPs/plans/{feature-name}.plan.md` with:
- **Summary**: High-level approach.
- **Metadata**: Type, complexity, systems affected.
- **Mandatory Reading**: Critical files implementation should mirror.
- **Step-by-Step Tasks**: Atomic, independently verifiable tasks.
- **Validation Commands**: Specific commands for syntax, tests, and build.

## Phase 5: Verification
- Ensure tasks are ordered by dependency.
- Confirm every task has an executable validation command.
- Verify pattern references include actual code snippets from the codebase.
