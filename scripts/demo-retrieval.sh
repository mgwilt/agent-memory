#!/usr/bin/env sh
set -eu

api_url="${ACTR_API_URL:-http://127.0.0.1:8080}"
agent_id="${ACTR_DEMO_AGENT_ID:-demo-agent}"
max_attempts="${ACTR_DEMO_MAX_ATTEMPTS:-30}"
tmp_dir="$(mktemp -d)"

cleanup() {
  rm -rf "$tmp_dir"
}
trap cleanup EXIT INT TERM

require_command() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "Required command not found: $1" >&2
    exit 1
  fi
}

assert_matches() {
  file="$1"
  pattern="$2"
  message="$3"

  if ! grep -Eq "$pattern" "$file"; then
    echo "$message" >&2
    echo "Response body:" >&2
    sed -n '1,120p' "$file" >&2
    exit 1
  fi
}

post_json() {
  path="$1"
  payload="$2"
  output="$3"

  curl -fsS \
    --connect-timeout 2 \
    --max-time 10 \
    -H 'content-type: application/json' \
    --data-binary "@$payload" \
    "$api_url$path" \
    -o "$output"
}

require_command curl
require_command grep
require_command sed

health="$tmp_dir/health.json"
attempt=1
while [ "$attempt" -le "$max_attempts" ]; do
  if curl -fsS --connect-timeout 2 --max-time 10 "$api_url/healthz" -o "$health"; then
    break
  fi

  if [ "$attempt" -eq "$max_attempts" ]; then
    echo "Timed out waiting for API at $api_url" >&2
    exit 1
  fi

  sleep 1
  attempt=$((attempt + 1))
done
assert_matches "$health" '"status"[[:space:]]*:[[:space:]]*"pass"' "API health check did not return pass"

context_payload="$tmp_dir/context.json"
cat >"$context_payload" <<EOF
{
  "agent_id": "$agent_id",
  "chunk_id": "ck-demo-context",
  "chunk_type": "goal",
  "now_ms": 1000,
  "slots": {
    "topic": { "type": "symbol", "value": "memory" }
  }
}
EOF

fact_payload="$tmp_dir/fact.json"
cat >"$fact_payload" <<EOF
{
  "agent_id": "$agent_id",
  "chunk_id": "ck-demo-actr",
  "chunk_type": "fact",
  "now_ms": 1000,
  "slots": {
    "topic": { "type": "symbol", "value": "act-r" }
  }
}
EOF

distractor_payload="$tmp_dir/distractor.json"
cat >"$distractor_payload" <<EOF
{
  "agent_id": "$agent_id",
  "chunk_id": "ck-demo-other",
  "chunk_type": "fact",
  "now_ms": 1000,
  "slots": {
    "topic": { "type": "symbol", "value": "unrelated" }
  }
}
EOF

association_payload="$tmp_dir/association.json"
cat >"$association_payload" <<EOF
{
  "agent_id": "$agent_id",
  "src_chunk_id": "ck-demo-context",
  "dst_chunk_id": "ck-demo-actr",
  "source": "demo",
  "strength": 1.5,
  "updated_at_ms": 1500
}
EOF

retrieve_payload="$tmp_dir/retrieve.json"
cat >"$retrieve_payload" <<EOF
{
  "agent_id": "$agent_id",
  "chunk_type": "fact",
  "now_ms": 2000,
  "activation_threshold": -5.0,
  "deterministic_seed": 42,
  "context_chunk_ids": ["ck-demo-context"],
  "cue_slots": [
    { "key": "topic", "value": { "type": "symbol", "value": "ACT-R" } }
  ]
}
EOF

post_json "/v1/memory/chunks" "$context_payload" "$tmp_dir/context-response.json"
post_json "/v1/memory/chunks" "$fact_payload" "$tmp_dir/fact-response.json"
post_json "/v1/memory/chunks" "$distractor_payload" "$tmp_dir/distractor-response.json"
post_json "/v1/memory/associate" "$association_payload" "$tmp_dir/association-response.json"
post_json "/v1/memory/retrieve" "$retrieve_payload" "$tmp_dir/retrieve-response.json"

assert_matches "$tmp_dir/retrieve-response.json" '"status"[[:space:]]*:[[:space:]]*"hit"' "Demo retrieval did not hit"
assert_matches "$tmp_dir/retrieve-response.json" '"chunk_id"[[:space:]]*:[[:space:]]*"ck-demo-actr"' "Demo retrieval returned the wrong chunk"
assert_matches "$tmp_dir/retrieve-response.json" '"candidates_examined"[[:space:]]*:[[:space:]]*1' "Demo retrieval examined an unexpected candidate set"

metrics="$tmp_dir/metrics.txt"
curl -fsS --connect-timeout 2 --max-time 10 "$api_url/metrics" -o "$metrics"
assert_matches "$metrics" '^actr_memory_retrieval_hits_total[[:space:]]+[1-9][0-9]*(\.[0-9]+)?$' "Retrieval hit metric was not incremented"

echo "Demo retrieval passed against $api_url"
