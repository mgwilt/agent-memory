pub const ROOT_AFTER_HELP: &str = "Progressive discovery:
  actr-memory guide commands
  actr-memory chunk --help
  actr-memory retrieve --help
  actr-memory guide workflow

Docs:
  docs/cli/README.md";

pub const SLOT_GUIDE: &str = "Slot and cue values:
  --slot key=symbol:value
  --slot key=text:value
  --slot key=number:12.5
  --slot key=bool:true
  --cue topic=symbol:preference

Explore:
  actr-memory guide slots
  docs/cli/slots-and-json.md";

pub const WORKFLOW: &str = "actr-memory --agent agent-1 chunk put ctx-goal --type goal --slot task=symbol:answer-memory-question
actr-memory --agent agent-1 chunk put mem-preference --type fact --slot subject=symbol:eli --slot topic=symbol:preference --slot detail=text:\"strong black coffee\"
actr-memory --agent agent-1 practice mem-preference --kind retrieve --weight 2
actr-memory --agent agent-1 associate ctx-goal mem-preference --source goal --strength 1.25
actr-memory --agent agent-1 buffer set goal ctx-goal
actr-memory --agent agent-1 retrieve --type fact --cue topic=symbol:preference --context ctx-goal --threshold -10 --result-limit 3
actr-memory --agent agent-1 rule eval --retrieved mem-preference --rules-file rules.json
actr-memory metrics --grep retrieval_hits";

pub const ERRORS: &str = "Exit codes:
  0 success
  2 CLI usage or local validation error
  3 API bad_request
  4 API not_found
  5 API conflict
  6 API unavailable, network failure, or timeout
  7 invalid API response or internal CLI error

Error pattern:
  [error] what failed
  Use or Explore: next useful command";
