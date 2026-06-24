# Progressive Disclosure

[CLI docs index](./README.md)

Nestor CLI help is layered so short command lists stay readable and detailed
references remain available when needed. Root help orients, group help routes,
leaf help supports execution, and long-form docs explain workflows.

## Design Rules

- Root help and `nestor guide commands` list the command map and next places to
  look.
- Command group help names valid subcommands, one-line purposes, and the next
  commands to try.
- Leaf help includes required arguments, defaults, examples, the endpoint, and a
  relevant docs link.
- Deep guide pages contain workflow bodies, grammar rules, error recovery, and
  crosslinks.
- Every deep page explains when to use it and where to go next.

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
