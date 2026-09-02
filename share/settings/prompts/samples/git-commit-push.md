---
description: Review all uncommitted tracked and untracked work, create one complete commit, and push the current branch.
---

# Commit and Push All Pending Work

Review every uncommitted change in the current Git worktree, including staged, unstaged, and untracked files. Summarize the complete pending change set, commit it as one cohesive snapshot, then push the current branch.

## Inspect

1. Confirm the current directory is inside a Git worktree. Stop if it is not.
2. Inspect `git status --short`, `git diff`, `git diff --cached`, untracked files, and `git log --oneline -10` before staging anything.
3. Review every changed file and hunk, including untracked files. Do not omit work merely because it predates this session or belongs to a different task.
4. Summarize the pending changes by file and functional area. State whether the work forms a reasonable single commit.
5. Stop and report the blocker if any file contains credentials, secrets, private keys, tokens, generated build output, dependency caches, or material that should not be versioned. Do not stage or commit it.

## Commit

1. Preserve the existing worktree and index. Never reset, restore, discard, unstage, amend, or otherwise alter prior history.
2. Stage the complete reviewed pending change set, including tracked and intended untracked files. Use explicit paths when practical; do not blindly stage ignored files.
3. Before committing, inspect the staged diff and confirm it exactly matches the reviewed summary.
4. Write one commit message describing the complete change set.
5. Use this format:

   ```text
   <Scope> - <Subject>
   ```

6. Use a capitalized imperative subject of 50 characters or fewer, with no ending punctuation. Add a concise body only when it supplies essential context absent from the subject.
7. Create one commit without bypassing Git hooks.

## Push

1. Confirm the current branch has an unambiguous configured upstream. If it does not, stop and ask for the intended remote and branch.
2. Push with `git push`.
3. Never force-push or alter remotes, branch names, history, or Git configuration.
4. Stop on failure. State the failed command and its error without destructive recovery.

## Report

State the pending-change summary, commit SHA, commit message subject, push result, and final `git status --short` output.
