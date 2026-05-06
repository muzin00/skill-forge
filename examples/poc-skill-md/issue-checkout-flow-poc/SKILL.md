---
name: issue-checkout-flow-poc
description: This skill should be used when the user asks to "create a branch for an issue", "switch to an issue branch", "Issue #N でブランチ切って", "Issue から branch 切って", or otherwise requests checking out a Git branch derived from a GitHub Issue. Composes the skill-forge issue-branch-name and issue-checkout MCP tools.
---

# issue-checkout flow PoC

A composition workflow that demonstrates Claude Code calling two MCP tools sequentially: `mcp__skill-forge__issue-branch-name` (preview a recommended branch name) and `mcp__skill-forge__issue-checkout` (actually create and switch to the branch).

Both tools are exposed by `skill-forge mcp-server --mode skills`.

## When to use

The user wants to start working on a specific GitHub Issue by creating and switching to a Git branch, for example:

- "Issue #123 でブランチ切って"
- "https://github.com/owner/repo/issues/45 で branch 切って"
- "Check out a branch for issue #7"

## Steps

1. Parse the issue identifier from the user's request:
   - integer issue number — required for `issue-branch-name`
   - full issue URL — required for `issue-checkout`

   If the user only provided one form, derive the other when possible (e.g. infer the URL from the current repository's `gh repo view` and the number, or extract the number from the URL path).

2. Call `mcp__skill-forge__issue-branch-name` with:

   ```json
   { "issue_number": <N> }
   ```

   (Add `prefix` only if the user explicitly asked for a non-default type such as `fix` or `chore`.) Show the proposed branch name to the user.

3. Call `mcp__skill-forge__issue-checkout` with:

   ```json
   { "url": "<full issue URL>" }
   ```

   This tool internally generates its own branch name and runs `git checkout -b`, so the actual branch created may differ from the proposal in step 2. That is expected.

4. Report the result:
   - The branch name actually created (from `issue-checkout`'s response)
   - The proposed name from step 2, noted as a comparison if it differs

## Notes

- `issue-checkout` runs `git checkout -b` in the current working directory of the MCP server process (which is the directory where Claude Code was launched). Make sure Claude Code is started from inside the target Git repository.
- If the user's request is ambiguous (e.g. issue number without a repository context), ask for clarification before invoking either tool.
