---
description: Systematically debug and diagnose a reported problem
---

# Systematic Debugging Workflow

Use this workflow to find the root cause of complex bugs and implement reliable fixes.

## 1. Reproduce and Isolate
- Get exact steps to reproduce the issue.
- Verify the problem locally and note logs/error messages.
- Use binary search (commenting out code) or git bisect to isolate the cause.

## 2. Classify the Issue
Identify the strategy based on the type:
- **Runtime Error**: Analyze stack traces and variable states at the crash point.
- **Logic Error**: Trace execution step-by-step with logs.
- **Performance**: Measure timings and look for N+1 queries or inefficient loops.
- **Integration**: Verify external services, auth, and request/response formats.

## 3. Root Cause Analysis (5 Whys)
Ask "Why" until the fundamental cause is found:
1. Why is X happening?
2. Why does Y cause that?
3. ...and so on.

## 4. Implement and Verify
- Fix the **root cause**, not just the symptoms.
- Add defensive programming and consider edge cases.
- **Verify**: Confirm the fix, check for regressions, and add a test case.

## 5. Debug Summary
Document the findings:
- **Issue**: What was broken.
- **Root Cause**: Why it was broken.
- **Fix**: What was changed.
- **Prevention**: How to avoid similar issues in the future.

## 6. Autonomous Bug Fixing
When given a bug report, just fix it. Don't ask for hand-holding.
Point at logs, errors, failing tests — then resolve them.
Zero context switching required from the user.
Go fix failing CI tests without being told how.
