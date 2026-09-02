---
description: Perform a comprehensive code review of changes or a PR
---

# Code Review Workflow

Use this workflow to ensure high code quality and consistency with the project's patterns.

## 1. Context & Scope
- Identify the files changed in the PR or staged area.
- Understand the intent of the changes.

## 2. Review Dimensions
Analyze the changes against these criteria:
- **Bugs & Logic**: Identify potential edge cases, race conditions, or off-by-one errors.
- **Patterns**: Ensure changes follow the project's established naming and architectural conventions.
- **Types & Errors**: Check for rigorous type safety and specific error handling. No "catch-all" exceptions.
- **Tests**: Verify that new code is covered by meaningful tests.
- **Documentation**: Ensure comments are accurate and public APIs are documented.
- **Simplicity**: Identify areas that can be simplified or made more readable.

## 3. Output Format
Categorize findings:
- **CRITICAL**: Issues that must be fixed before merging (bugs, regressions).
- **IMPORTANT**: Best practices, type improvements, test gaps.
- **SUGGESTION**: Style tweaks, readability improvements, future refactors.

## 4. Final Verdict
Provide a summary:
- **APPROVE**: No critical issues.
- **REQUEST CHANGES**: Critical issues found.
- **COMMENT**: Insights or questions without a hard stance.
