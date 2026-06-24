#!/usr/bin/env sh
set -eu

migration_dir="${MIGRATION_DIR:-crates/nestor-store/migrations}"
service="${MEMGRAPH_SERVICE:-memgraph}"

apply_statement() {
  statement="$1"
  output_file="$(mktemp)"

  if printf '%s;\n' "$statement" | docker compose exec -T "$service" mgconsole >"$output_file" 2>&1; then
    rm -f "$output_file"
    return 0
  fi

  output="$(cat "$output_file")"
  rm -f "$output_file"

  case "$output" in
    *"already exists"*)
      echo "Skipping already-applied schema statement"
      ;;
    *)
      printf '%s\n' "$output" >&2
      exit 1
      ;;
  esac
}

for migration in "$migration_dir"/*.cypher; do
  if [ ! -f "$migration" ]; then
    echo "No Cypher migrations found in $migration_dir" >&2
    exit 1
  fi

  echo "Applying $migration"

  statement=""
  while IFS= read -r line || [ -n "$line" ]; do
    case "$line" in
      "" | --*)
        continue
        ;;
    esac

    if [ -n "$statement" ]; then
      statement="${statement}
${line}"
    else
      statement="$line"
    fi

    case "$line" in
      *";")
        statement="${statement%;}"
        apply_statement "$statement"
        statement=""
        ;;
    esac
  done < "$migration"

  if [ -n "$statement" ]; then
    echo "Migration $migration ended without a terminating semicolon" >&2
    exit 1
  fi
done
