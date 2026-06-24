# Progressive Disclosure

[CLI docs index](./README.md)

Progressive disclosure is a product requirement for this CLI. Agents should see
only the smallest useful command map at first, then drill into command groups,
leaf help, and long-form documentation on demand.

This follows the same direction as Anthropic's agent documentation: skills keep
long instructions unloaded until used, and tool descriptions influence when a
model chooses a tool. The practical rule here is simple: always-loaded help must
stay compact, while deeper instructions must be easy to discover.

References:

- [Claude Code skills](https://code.claude.com/docs/en/slash-commands)
- [Claude tool use overview](https://platform.claude.com/docs/en/agents-and-tools/tool-use/overview)

## Discovery Levels

Level 0:

```sh
actr-memory --help
actr-memory guide
actr-memory guide commands
```

Shows the command map, global options, and next drill-down commands.

Level 1:

```sh
actr-memory chunk
actr-memory buffer
actr-memory rule
```

Calling a command group without a subcommand prints valid subcommands, one-line
purposes, and next actions.

Level 2:

```sh
actr-memory chunk put --help
actr-memory retrieve --help
actr-memory rule eval --help
```

Leaf help includes purpose, endpoint, required args, defaults, examples, and
links to relevant docs.

Level 3:

```sh
actr-memory guide slots
actr-memory guide workflow
actr-memory guide errors
actr-memory guide docs
```

Deep guides explain slot grammar, full workflows, error recovery, and the docs
index.

See [Command Reference](./commands.md) and [Output And Errors](./output-and-errors.md).
