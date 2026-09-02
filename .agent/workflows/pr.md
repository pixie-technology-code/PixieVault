---
description: Create a pull request with a comprehensive description
---

# Pull Request Workflow

Use this workflow to push your branch and create a professional PR.

## 1. Push Changes
- Ensure the current branch is up to date.
- Push the branch to the remote repository.

## 2. Generate Description
Analyze the commits in this branch compared to the base branch (e.g., `main`):
- **Summary**: High-level overview of the feature/fix.
- **Details**: Bullet points of key changes.
- **Testing**: What has been tested and how.
- **Related**: Links to issues or PRDs.

## 3. Create PR
Use the GitHub CLI (`gh pr create`) or provide the link/content for manual creation.
- Set an appropriate title.
- Include the generated description.
- Add labels or reviewers if applicable.
