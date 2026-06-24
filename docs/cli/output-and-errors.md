# Output And Errors

[CLI docs index](./README.md)

The CLI keeps stdout and stderr separate:

- stdout contains successful command output.
- stderr contains errors, hints, and verbose request metadata.

Text output is the default. JSON-producing commands also support:

```sh
--format json
--format pretty-json
```

`--agent-footer` appends a final status line for run-tool environments:

```text
[exit:0 | 12ms]
```

## Exit Codes

```text
0 success
2 CLI usage/local validation
3 API bad_request
4 API not_found
5 API conflict
6 API/network unavailable or timeout
7 invalid API response or internal CLI error
```

## Corrective Error Pattern

Every error should say what failed and where to go next:

```text
[error] invalid slot value "topic=preference"
Use typed values: --slot key=symbol:value
Explore: actr-memory guide slots
```

See [Progressive Disclosure](./progressive-disclosure.md) and
[Testing And Definition Of Done](./testing-and-dod.md).
