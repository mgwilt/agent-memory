#!/usr/bin/env sh
set -eu

max_attempts="${MAX_ATTEMPTS:-60}"
service="${MEMGRAPH_SERVICE:-memgraph}"
attempt=1

while [ "$attempt" -le "$max_attempts" ]; do
  if printf '%s\n' "RETURN 1;" | docker compose exec -T "$service" mgconsole >/dev/null 2>&1; then
    echo "Memgraph is ready"
    exit 0
  fi

  sleep 1
  attempt=$((attempt + 1))
done

echo "Timed out waiting for Memgraph" >&2
exit 1
