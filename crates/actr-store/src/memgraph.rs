use actr_core::MemoryResult;

use crate::migrations::{
    SchemaMigration, embedded_migrations, is_already_applied_schema_error, migration_statements,
    validate_migrations,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemgraphRepositoryConfig {
    pub uri: String,
    pub user: String,
    pub database: String,
    pub max_connections: usize,
    pub schema_info_enabled: bool,
}

impl Default for MemgraphRepositoryConfig {
    fn default() -> Self {
        Self {
            uri: "bolt://127.0.0.1:7687".to_string(),
            user: "memgraph".to_string(),
            database: "memgraph".to_string(),
            max_connections: 16,
            schema_info_enabled: true,
        }
    }
}

impl MemgraphRepositoryConfig {
    pub fn bolt_address(&self) -> &str {
        self.uri
            .strip_prefix("bolt://")
            .unwrap_or(self.uri.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemgraphDriverPlan {
    pub driver: &'static str,
    pub rationale: &'static str,
    pub integration_goal: &'static str,
}

pub fn recommended_driver_plan() -> MemgraphDriverPlan {
    MemgraphDriverPlan {
        driver: "neo4rs",
        rationale: "async pooled Bolt access, explicit transactions, and a clear path to Tokio service wiring",
        integration_goal: "G04-memgraph-schema-repository",
    }
}

pub trait SchemaExecutor {
    fn execute_schema_statement(&self, cypher: &str) -> MemoryResult<()>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MigrationApplyReport {
    pub attempted: usize,
    pub applied: usize,
    pub skipped_already_applied: usize,
}

pub fn apply_embedded_migrations(
    executor: &impl SchemaExecutor,
) -> MemoryResult<MigrationApplyReport> {
    let migrations = embedded_migrations();
    apply_migrations(executor, &migrations)
}

pub fn apply_migrations(
    executor: &impl SchemaExecutor,
    migrations: &[SchemaMigration],
) -> MemoryResult<MigrationApplyReport> {
    validate_migrations(migrations)?;
    let mut report = MigrationApplyReport {
        attempted: 0,
        applied: 0,
        skipped_already_applied: 0,
    };

    for migration in migrations {
        for statement in migration_statements(migration)? {
            report.attempted += 1;
            match executor.execute_schema_statement(&statement.cypher) {
                Ok(()) => report.applied += 1,
                Err(error) if is_already_applied_schema_error(&error.to_string()) => {
                    report.skipped_already_applied += 1;
                }
                Err(error) => return Err(error),
            }
        }
    }

    Ok(report)
}

#[cfg(test)]
mod tests {
    use std::{
        cell::RefCell,
        process::Command,
        time::{SystemTime, UNIX_EPOCH},
    };

    use actr_core::{MemoryError, MemoryResult};

    use super::*;

    #[derive(Debug, Default)]
    struct RecordingExecutor {
        statements: RefCell<Vec<String>>,
        fail_with: RefCell<Option<MemoryError>>,
    }

    impl RecordingExecutor {
        fn already_applied_error() -> Self {
            Self {
                statements: RefCell::new(Vec::new()),
                fail_with: RefCell::new(Some(MemoryError::Conflict(
                    "Constraint already exists".to_string(),
                ))),
            }
        }
    }

    impl SchemaExecutor for RecordingExecutor {
        fn execute_schema_statement(&self, cypher: &str) -> MemoryResult<()> {
            self.statements.borrow_mut().push(cypher.to_string());
            if let Some(error) = self.fail_with.borrow().clone() {
                Err(error)
            } else {
                Ok(())
            }
        }
    }

    #[test]
    fn config_exposes_bolt_address_for_future_driver() {
        let config = MemgraphRepositoryConfig::default();

        assert_eq!(config.bolt_address(), "127.0.0.1:7687");
        assert_eq!(recommended_driver_plan().driver, "neo4rs");
    }

    #[test]
    fn migration_runner_applies_ordered_schema_statements() -> MemoryResult<()> {
        let executor = RecordingExecutor::default();
        let report = apply_embedded_migrations(&executor)?;

        assert!(report.attempted > 10);
        assert_eq!(report.applied, report.attempted);
        assert_eq!(report.skipped_already_applied, 0);
        assert_eq!(executor.statements.borrow().len(), report.attempted);
        Ok(())
    }

    #[test]
    fn migration_runner_skips_already_applied_schema_statements() -> MemoryResult<()> {
        let executor = RecordingExecutor::already_applied_error();
        let report = apply_embedded_migrations(&executor)?;

        assert!(report.attempted > 10);
        assert_eq!(report.applied, 0);
        assert_eq!(report.skipped_already_applied, report.attempted);
        Ok(())
    }

    #[test]
    fn live_memgraph_repository_contract_smoke() -> MemoryResult<()> {
        if std::env::var("ACTR_STORE_MEMGRAPH_TESTS").as_deref() != Ok("1") {
            return Ok(());
        }

        let test_id = format!("g04-{}", now_ms());
        let agent_id = format!("agent-{test_id}");
        let chunk_a = format!("ck-a-{test_id}");
        let chunk_b = format!("ck-b-{test_id}");
        let event_id = format!("event-{test_id}");
        let duplicate_id = format!("dup-{test_id}");
        let rule_id = format!("rule-{test_id}");
        let setup = format!(
            "
            MATCH (n)
            WHERE n.agent_id = '{agent_id}' OR n.tenant_id = '{agent_id}'
            DETACH DELETE n;

            MERGE (a:Agent {{agent_id: '{agent_id}'}})
            CREATE (c1:Chunk {{
              agent_id: '{agent_id}',
              chunk_id: '{chunk_a}',
              chunk_type: 'fact',
              active: true,
              retrieval_count: 0,
              created_at_ms: 1,
              updated_at_ms: 1,
              slot_hash: 'slot-a',
              base_bias: 0.0,
              version: 1
            }})
            CREATE (c2:Chunk {{
              agent_id: '{agent_id}',
              chunk_id: '{chunk_b}',
              chunk_type: 'fact',
              active: true,
              retrieval_count: 0,
              created_at_ms: 1,
              updated_at_ms: 1,
              slot_hash: 'slot-b',
              base_bias: 0.0,
              version: 1
            }})
            MERGE (a)-[:OWNS]->(c1)
            MERGE (a)-[:OWNS]->(c2)
            MERGE (v:SlotValue {{
              tenant_id: '{agent_id}',
              key: 'topic',
              value_hash: 'memgraph'
            }})
            SET v.value_norm = 'memgraph',
                v.value_type = 'symbol'
            CREATE (c1)-[:HAS_SLOT {{key: 'topic', value_type: 'symbol'}}]->(v)
            CREATE (e:PracticeEvent {{
              event_id: '{event_id}',
              agent_id: '{agent_id}',
              chunk_id: '{chunk_a}',
              occurred_at_ms: 1,
              kind: 'encode',
              weight: 1.0
            }})
            CREATE (c1)-[:HAS_EVENT]->(e)
            CREATE (c2)-[:ASSOCIATED {{source: 'test', strength: 0.7, fan: 1}}]->(c1)
            MERGE (b:Buffer {{agent_id: '{agent_id}', buffer_name: 'goal'}})
            CREATE (b)-[:CURRENT {{set_at_ms: 1}}]->(c1)
            CREATE (p:ProductionRule {{
              agent_id: '{agent_id}',
              rule_id: '{rule_id}',
              name: 'retrieve fact',
              enabled: true,
              utility: 1.0,
              version: 1,
              success_count: 1,
              failure_count: 0,
              avg_reward: 1.0
            }})
            MERGE (a)-[:OWNS_RULE]->(p);
            ",
            agent_id = cypher_string(&agent_id),
            chunk_a = cypher_string(&chunk_a),
            chunk_b = cypher_string(&chunk_b),
            event_id = cypher_string(&event_id),
            rule_id = cypher_string(&rule_id)
        );
        run_mgconsole(&setup)?;

        let fetch = format!(
            "
            MATCH (:Agent {{agent_id: '{agent_id}'}})-[:OWNS]->(c:Chunk {{chunk_id: '{chunk_a}'}})
            MATCH (p:ProductionRule {{agent_id: '{agent_id}', rule_id: '{rule_id}'}})
            OPTIONAL MATCH (c)-[:HAS_SLOT]->(v:SlotValue)
            OPTIONAL MATCH (c)-[:HAS_EVENT]->(e:PracticeEvent)
            OPTIONAL MATCH (:Chunk {{agent_id: '{agent_id}', chunk_id: '{chunk_b}'}})-[assoc:ASSOCIATED]->(c)
            OPTIONAL MATCH (:Buffer {{agent_id: '{agent_id}', buffer_name: 'goal'}})-[current:CURRENT]->(c)
            RETURN c.chunk_id AS chunk_id,
                   count(DISTINCT v) AS slots,
                   count(DISTINCT e) AS events,
                   count(DISTINCT assoc) AS associations,
                   count(DISTINCT current) AS buffers,
                   p.rule_id AS rule_id;
            ",
            agent_id = cypher_string(&agent_id),
            chunk_a = cypher_string(&chunk_a),
            chunk_b = cypher_string(&chunk_b),
            rule_id = cypher_string(&rule_id)
        );
        let output = run_mgconsole(&fetch)?;
        let row = output
            .lines()
            .find(|line| line.contains(&format!("\"{chunk_a}\"")))
            .ok_or_else(|| {
                MemoryError::StoreUnavailable(format!(
                    "expected live Memgraph row for {chunk_a}, got: {output}"
                ))
            })?;
        let cells = row
            .split('|')
            .map(str::trim)
            .filter(|cell| !cell.is_empty())
            .collect::<Vec<_>>();
        let expected_chunk = format!("\"{chunk_a}\"");
        let expected_rule = format!("\"{rule_id}\"");

        assert_eq!(cells.first(), Some(&expected_chunk.as_str()));
        assert_eq!(cells.get(1), Some(&"1"));
        assert_eq!(cells.get(2), Some(&"1"));
        assert_eq!(cells.get(3), Some(&"1"));
        assert_eq!(cells.get(4), Some(&"1"));
        assert_eq!(cells.get(5), Some(&expected_rule.as_str()));

        let duplicate = format!(
            "
            CREATE (:PracticeEvent {{
              event_id: '{duplicate_id}',
              agent_id: '{agent_id}',
              chunk_id: '{chunk_a}',
              occurred_at_ms: 2,
              kind: 'retrieve',
              weight: 1.0
            }});
            CREATE (:PracticeEvent {{
              event_id: '{duplicate_id}',
              agent_id: '{agent_id}',
              chunk_id: '{chunk_a}',
              occurred_at_ms: 3,
              kind: 'retrieve',
              weight: 1.0
            }});
            ",
            agent_id = cypher_string(&agent_id),
            chunk_a = cypher_string(&chunk_a),
            duplicate_id = cypher_string(&duplicate_id)
        );
        assert!(run_mgconsole(&duplicate).is_err());

        let cleanup = format!(
            "
            MATCH (n)
            WHERE n.agent_id = '{agent_id}' OR n.tenant_id = '{agent_id}'
            DETACH DELETE n;
            ",
            agent_id = cypher_string(&agent_id)
        );
        run_mgconsole(&cleanup)?;

        Ok(())
    }

    fn run_mgconsole(cypher: &str) -> MemoryResult<String> {
        let mut child = Command::new("docker")
            .args(["compose", "exec", "-T", "memgraph", "mgconsole"])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|error| MemoryError::StoreUnavailable(error.to_string()))?;

        if let Some(mut stdin) = child.stdin.take() {
            use std::io::Write;
            stdin
                .write_all(cypher.as_bytes())
                .map_err(|error| MemoryError::StoreUnavailable(error.to_string()))?;
        }

        let output = child
            .wait_with_output()
            .map_err(|error| MemoryError::StoreUnavailable(error.to_string()))?;
        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            Err(MemoryError::StoreUnavailable(format!(
                "{}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            )))
        }
    }

    fn cypher_string(value: &str) -> String {
        value.replace('\\', "\\\\").replace('\'', "\\'")
    }

    fn now_ms() -> u128 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |duration| duration.as_millis())
    }
}
