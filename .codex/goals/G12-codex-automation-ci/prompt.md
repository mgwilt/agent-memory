# Goal

Add Codex-native automation for this repository.

# Context

Create goal folders, prompt files, output schemas, verification scripts, and
GitHub Actions wiring that can run review and selected non-interactive Codex
tasks. Treat "Goal" as a repository convention, not as an assumed first-class
OpenAI product object.

# Constraints

- Keep API keys scoped to the Codex step only.
- Do not expose secrets to arbitrary repository code.
- Use machine-readable output.
- Prefer trusted triggers and explicit allowlists.

# Done When

- A local `codex exec --json ... --output-schema ...` command can execute at
  least one goal.
- CI captures Codex output as an artifact or review result.
- Goal prompts are reviewed into the repository.
