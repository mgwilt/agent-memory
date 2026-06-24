# Progressive Disclosure

[CLI docs index](./README.md)

Progressive disclosure is a product requirement for this CLI. Agents should see
only the smallest useful command map at first, then drill into command groups,
leaf help, and long-form documentation on demand.

This follows Anthropic's agent documentation pattern: keep always-loaded context
small, use short descriptions to help the model choose the right tool or command,
and load detailed instructions only when the task calls for them. The practical
rule here is simple: root help should orient, group help should route, leaf help
should execute, and long docs should explain.

References:

- [Claude Code skills](https://code.claude.com/docs/en/slash-commands)
- [Claude tool use overview](https://platform.claude.com/docs/en/agents-and-tools/tool-use/overview)

## Discovery Levels

## Design Rules

- Root help and `nestor guide commands` are selection surfaces. They should stay
  compact and avoid long reference material.
- Command group help is a routing surface. It should name valid subcommands,
  one-line purposes, and the next 2-3 commands to try.
- Leaf help is an execution surface. It should include required arguments,
  defaults, examples, the endpoint, and a relevant docs link.
- Deep guide pages are reference surfaces. They can include workflow bodies,
  grammar rules, error recovery, and crosslinks.
- Every deep page should say when to use it and where to go next.

## Level 0: Entry Surface

```sh
nestor --help
nestor guide
nestor guide commands
```

Shows the command map, global options, and next drill-down commands.

## Level 1: Command Group Discovery

```sh
nestor chunk
nestor buffer
nestor rule
```

Calling a command group without a subcommand prints valid subcommands, one-line
purposes, and next actions.

## Level 2: Leaf Command Help

```sh
nestor chunk put --help
nestor retrieve --help
nestor rule eval --help
```

Leaf help includes purpose, endpoint, required args, defaults, examples, and
links to relevant docs.

## Level 3: Deep Guides

```sh
nestor guide slots
nestor guide workflow
nestor guide errors
nestor guide docs
```

Deep guides explain slot grammar, full workflows, error recovery, and the docs
index.

See [Command Reference](./commands.md) and [Output And Errors](./output-and-errors.md).
