#!/usr/bin/env sh
set -eu

api_url="${NESTOR_API_URL:-http://127.0.0.1:8080}"
agent_id="${NESTOR_DEMO_AGENT_ID:-demo-lifecycle-$(date +%s)-$$}"
max_attempts="${NESTOR_DEMO_MAX_ATTEMPTS:-30}"
restart_api="${NESTOR_DEMO_RESTART_API:-1}"
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
    sed -n '1,160p' "$file" >&2
    exit 1
  fi
}

get_json() {
  path="$1"
  output="$2"

  curl -fsS \
    --connect-timeout 2 \
    --max-time 10 \
    "$api_url$path" \
    -o "$output"
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

wait_ready() {
  ready="$tmp_dir/ready.json"
  attempt=1
  while [ "$attempt" -le "$max_attempts" ]; do
    if get_json "/readyz" "$ready"; then
      if grep -Eq '"status"[[:space:]]*:[[:space:]]*"pass"' "$ready"; then
        assert_matches "$ready" '"name"[[:space:]]*:[[:space:]]*"memgraph"' "Readiness did not include Memgraph"
        return
      fi
    fi

    if [ "$attempt" -eq "$max_attempts" ]; then
      echo "Timed out waiting for API readiness at $api_url" >&2
      sed -n '1,160p' "$ready" >&2 || true
      exit 1
    fi

    sleep 1
    attempt=$((attempt + 1))
  done
}

restart_api_if_available() {
  if [ "$restart_api" != "1" ]; then
    return
  fi
  if command -v docker >/dev/null 2>&1 && docker compose ps api >/dev/null 2>&1; then
    docker compose restart api >/dev/null
    wait_ready
  else
    echo "Skipping API restart; docker compose api service is not available" >&2
  fi
}

write_chunk_payload() {
  output="$1"
  chunk_id="$2"
  chunk_type="$3"
  now_ms="$4"
  slots="$5"

  cat >"$output" <<EOF
{
  "agent_id": "$agent_id",
  "chunk_id": "$chunk_id",
  "chunk_type": "$chunk_type",
  "now_ms": $now_ms,
  "slots": $slots
}
EOF
}

require_command curl
require_command date
require_command grep
require_command sed

wait_ready

write_chunk_payload "$tmp_dir/context.json" "ck-lifecycle-context" "goal" 1000 '{
  "topic": { "type": "symbol", "value": "memory" }
}'
write_chunk_payload "$tmp_dir/fact.json" "ck-lifecycle-fact" "fact" 1000 '{
  "topic": { "type": "symbol", "value": " ACT-R " },
  "detail": { "type": "text", "value": "Durable lifecycle payload" },
  "confidence": { "type": "number", "value": 0.875 },
  "protected": { "type": "bool", "value": false }
}'
write_chunk_payload "$tmp_dir/episode-a.json" "ck-lifecycle-episode-a" "episode" 1000 '{
  "topic": { "type": "symbol", "value": "preference" },
  "subject": { "type": "symbol", "value": "eli" },
  "detail": { "type": "text", "value": "coffee-a" }
}'
write_chunk_payload "$tmp_dir/episode-b.json" "ck-lifecycle-episode-b" "episode" 1100 '{
  "topic": { "type": "symbol", "value": "preference" },
  "subject": { "type": "symbol", "value": "eli" },
  "detail": { "type": "text", "value": "coffee-b" }
}'
write_chunk_payload "$tmp_dir/forget-old.json" "ck-lifecycle-forget-old" "stale" 100 '{
  "topic": { "type": "symbol", "value": "old" }
}'
write_chunk_payload "$tmp_dir/forget-protected.json" "ck-lifecycle-forget-protected" "stale" 100 '{
  "topic": { "type": "symbol", "value": "old" },
  "protected": { "type": "bool", "value": true }
}'

post_json "/v1/memory/chunks" "$tmp_dir/context.json" "$tmp_dir/context-response.json"
post_json "/v1/memory/chunks" "$tmp_dir/fact.json" "$tmp_dir/fact-response.json"
post_json "/v1/memory/chunks" "$tmp_dir/episode-a.json" "$tmp_dir/episode-a-response.json"
post_json "/v1/memory/chunks" "$tmp_dir/episode-b.json" "$tmp_dir/episode-b-response.json"
post_json "/v1/memory/chunks" "$tmp_dir/forget-old.json" "$tmp_dir/forget-old-response.json"
post_json "/v1/memory/chunks" "$tmp_dir/forget-protected.json" "$tmp_dir/forget-protected-response.json"

cat >"$tmp_dir/rehearse.json" <<EOF
{
  "agent_id": "$agent_id",
  "chunk_id": "ck-lifecycle-fact",
  "event_id": "rehearse-$agent_id",
  "weight": 2.0,
  "occurred_at_ms": 1500
}
EOF
post_json "/v1/memory/rehearse" "$tmp_dir/rehearse.json" "$tmp_dir/rehearse-response.json"
assert_matches "$tmp_dir/rehearse-response.json" '"kind"[[:space:]]*:[[:space:]]*"rehearse"' "Rehearsal did not record a rehearse event"

cat >"$tmp_dir/association.json" <<EOF
{
  "agent_id": "$agent_id",
  "src_chunk_id": "ck-lifecycle-context",
  "dst_chunk_id": "ck-lifecycle-fact",
  "source": "demo",
  "strength": 1.5,
  "fan": 1,
  "updated_at_ms": 2000
}
EOF
post_json "/v1/memory/associate" "$tmp_dir/association.json" "$tmp_dir/association-response.json"

cat >"$tmp_dir/retrieve.json" <<EOF
{
  "agent_id": "$agent_id",
  "chunk_type": "fact",
  "now_ms": 3000,
  "activation_threshold": -10.0,
  "noise_s": 0.0,
  "deterministic_seed": 42,
  "return_diagnostics": true,
  "commit_on_hit": true,
  "context_chunk_ids": ["ck-lifecycle-context"],
  "cue_slots": [
    { "key": "topic", "value": { "type": "symbol", "value": "act-r" } }
  ]
}
EOF
post_json "/v1/memory/retrieve" "$tmp_dir/retrieve.json" "$tmp_dir/retrieve-response.json"
assert_matches "$tmp_dir/retrieve-response.json" '"status"[[:space:]]*:[[:space:]]*"hit"' "Lifecycle retrieval did not hit"
assert_matches "$tmp_dir/retrieve-response.json" '"chunk_id"[[:space:]]*:[[:space:]]*"ck-lifecycle-fact"' "Lifecycle retrieval returned the wrong chunk"
assert_matches "$tmp_dir/retrieve-response.json" '"exact_practice_event_count"[[:space:]]*:[[:space:]]*2' "Retrieval did not use encode plus rehearse practice history before commit"

cat >"$tmp_dir/consolidate.json" <<EOF
{
  "agent_id": "$agent_id",
  "chunk_type": "episode",
  "summary_chunk_type": "semantic",
  "group_slot_keys": ["topic", "subject"],
  "min_group_size": 2,
  "now_ms": 4000
}
EOF
post_json "/v1/memory/consolidate" "$tmp_dir/consolidate.json" "$tmp_dir/consolidate-response.json"
assert_matches "$tmp_dir/consolidate-response.json" '"groups_consolidated"[[:space:]]*:[[:space:]]*1' "Consolidation did not create a semantic summary"

cat >"$tmp_dir/forget.json" <<EOF
{
  "agent_id": "$agent_id",
  "chunk_type": "stale",
  "now_ms": 1000000,
  "recency_cutoff_ms": 500,
  "base_level_cutoff": 0.0,
  "allow_linked_forget": false
}
EOF
post_json "/v1/memory/forget" "$tmp_dir/forget.json" "$tmp_dir/forget-response.json"
assert_matches "$tmp_dir/forget-response.json" '"forgotten_chunk_ids"[[:space:]]*:[[:space:]]*\[[^]]*"ck-lifecycle-forget-old"' "Forget did not soft-delete the old chunk"
assert_matches "$tmp_dir/forget-response.json" '"protected_chunk_ids"[[:space:]]*:[[:space:]]*\[[^]]*"ck-lifecycle-forget-protected"' "Forget did not preserve the protected chunk"

get_json "/v1/memory/chunks/ck-lifecycle-fact?agent_id=$agent_id" "$tmp_dir/fact-before-restart.json"
assert_matches "$tmp_dir/fact-before-restart.json" '"retrieval_count"[[:space:]]*:[[:space:]]*1' "Retrieval commit did not increment retrieval_count"
assert_matches "$tmp_dir/fact-before-restart.json" '"value"[[:space:]]*:[[:space:]]*"Durable lifecycle payload"' "Typed text payload was not stored"

restart_api_if_available

get_json "/v1/memory/chunks/ck-lifecycle-fact?agent_id=$agent_id" "$tmp_dir/fact-after-restart.json"
assert_matches "$tmp_dir/fact-after-restart.json" '"retrieval_count"[[:space:]]*:[[:space:]]*1' "Retrieval count did not survive API restart"
assert_matches "$tmp_dir/fact-after-restart.json" '"value"[[:space:]]*:[[:space:]]*" ACT-R "' "Original symbol payload did not survive API restart"

cat >"$tmp_dir/retrieve-after-restart.json" <<EOF
{
  "agent_id": "$agent_id",
  "chunk_type": "fact",
  "now_ms": 5000,
  "activation_threshold": -10.0,
  "noise_s": 0.0,
  "deterministic_seed": 42,
  "return_diagnostics": true,
  "commit_on_hit": false,
  "context_chunk_ids": ["ck-lifecycle-context"],
  "cue_slots": [
    { "key": "topic", "value": { "type": "symbol", "value": "ACT-R" } }
  ]
}
EOF
post_json "/v1/memory/retrieve" "$tmp_dir/retrieve-after-restart.json" "$tmp_dir/retrieve-after-restart-response.json"
assert_matches "$tmp_dir/retrieve-after-restart-response.json" '"status"[[:space:]]*:[[:space:]]*"hit"' "Post-restart retrieval did not hit"
assert_matches "$tmp_dir/retrieve-after-restart-response.json" '"exact_practice_event_count"[[:space:]]*:[[:space:]]*3' "Post-restart retrieval did not use durable encode, rehearse, and retrieve history"

echo "Lifecycle demo passed against $api_url with agent $agent_id"
