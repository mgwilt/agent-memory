use std::{
    process::{Command, Stdio},
    time::{SystemTime, UNIX_EPOCH},
};

use nestor_core::{MemoryError, MemoryResult};

#[test]
fn live_memgraph_retrieval_fixture_orders_by_spreading_strength() -> MemoryResult<()> {
    if std::env::var("NESTOR_STORE_MEMGRAPH_TESTS").as_deref() != Ok("1") {
        return Ok(());
    }

    let test_id = format!("g09-{}", now_ms());
    let agent_id = format!("agent-{test_id}");
    let context = format!("ck-context-{test_id}");
    let weak = format!("ck-weak-{test_id}");
    let strong = format!("ck-strong-{test_id}");
    let event_weak = format!("event-weak-{test_id}");
    let event_strong = format!("event-strong-{test_id}");
    let slot_hash = format!("slot-nestor-{test_id}");
    let setup = format!(
        "
        MATCH (n)
        WHERE n.agent_id = '{agent_id}' OR n.tenant_id = '{agent_id}'
        DETACH DELETE n;

        MERGE (agent:Agent {{agent_id: '{agent_id}'}})
        CREATE (context:Chunk {{
          agent_id: '{agent_id}',
          chunk_id: '{context}',
          chunk_type: 'goal',
          active: true,
          retrieval_count: 0,
          created_at_ms: 1,
          updated_at_ms: 1,
          slot_hash: 'context',
          base_bias: 0.0,
          version: 1
        }})
        CREATE (weak:Chunk {{
          agent_id: '{agent_id}',
          chunk_id: '{weak}',
          chunk_type: 'fact',
          active: true,
          retrieval_count: 0,
          created_at_ms: 1,
          updated_at_ms: 1,
          slot_hash: 'weak',
          base_bias: 0.0,
          version: 1
        }})
        CREATE (strong:Chunk {{
          agent_id: '{agent_id}',
          chunk_id: '{strong}',
          chunk_type: 'fact',
          active: true,
          retrieval_count: 0,
          created_at_ms: 1,
          updated_at_ms: 1,
          slot_hash: 'strong',
          base_bias: 0.0,
          version: 1
        }})
        MERGE (agent)-[:OWNS]->(context)
        MERGE (agent)-[:OWNS]->(weak)
        MERGE (agent)-[:OWNS]->(strong)
        MERGE (topic:SlotValue {{
          tenant_id: '{agent_id}',
          key: 'topic',
          value_hash: '{slot_hash}'
        }})
        SET topic.value_norm = 'act-r',
            topic.value_type = 'symbol'
        CREATE (weak)-[:HAS_SLOT {{key: 'topic', value_type: 'symbol'}}]->(topic)
        CREATE (strong)-[:HAS_SLOT {{key: 'topic', value_type: 'symbol'}}]->(topic)
        CREATE (weak_event:PracticeEvent {{
          event_id: '{event_weak}',
          agent_id: '{agent_id}',
          chunk_id: '{weak}',
          occurred_at_ms: 1,
          kind: 'encode',
          weight: 1.0
        }})
        CREATE (strong_event:PracticeEvent {{
          event_id: '{event_strong}',
          agent_id: '{agent_id}',
          chunk_id: '{strong}',
          occurred_at_ms: 1,
          kind: 'encode',
          weight: 1.0
        }})
        CREATE (weak)-[:HAS_EVENT]->(weak_event)
        CREATE (strong)-[:HAS_EVENT]->(strong_event)
        CREATE (context)-[:ASSOCIATED {{source: 'goal', strength: 0.25, fan: 2}}]->(weak)
        CREATE (context)-[:ASSOCIATED {{source: 'goal', strength: 2.0, fan: 2}}]->(strong);
        ",
        agent_id = cypher_string(&agent_id),
        context = cypher_string(&context),
        weak = cypher_string(&weak),
        strong = cypher_string(&strong),
        event_weak = cypher_string(&event_weak),
        event_strong = cypher_string(&event_strong),
        slot_hash = cypher_string(&slot_hash),
    );
    run_mgconsole(&setup)?;

    let query = format!(
        "
        MATCH (source:Chunk {{agent_id: '{agent_id}', chunk_id: '{context}'}})-[association]->(candidate:Chunk {{agent_id: '{agent_id}', active: true}})
        WHERE type(association) = 'ASSOCIATED'
        MATCH (candidate)-[:HAS_SLOT {{key: 'topic'}}]->(:SlotValue {{tenant_id: '{agent_id}', key: 'topic', value_norm: 'act-r'}})
        WITH candidate, max(association.strength) AS strength
        OPTIONAL MATCH (candidate)-[:HAS_EVENT]->(event:PracticeEvent)
        RETURN candidate.chunk_id AS chunk_id,
               strength AS strength,
               count(DISTINCT event) AS events
        ORDER BY strength DESC, chunk_id ASC;
        ",
        agent_id = cypher_string(&agent_id),
        context = cypher_string(&context),
    );
    let output = run_mgconsole(&query);
    let cleanup = format!(
        "
        MATCH (n)
        WHERE n.agent_id = '{agent_id}' OR n.tenant_id = '{agent_id}'
        DETACH DELETE n;
        ",
        agent_id = cypher_string(&agent_id),
    );
    run_mgconsole(&cleanup)?;
    let output = output?;
    let rows = matching_rows(&output, &[strong.as_str(), weak.as_str()]);
    let expected_strong = format!("\"{strong}\"");
    let expected_weak = format!("\"{weak}\"");

    assert_eq!(
        rows.len(),
        2,
        "expected strong and weak candidate rows in Memgraph output: {output}"
    );
    assert_eq!(
        rows.first().and_then(|row| row.first()).map(String::as_str),
        Some(expected_strong.as_str())
    );
    assert_eq!(
        rows.get(1).and_then(|row| row.first()).map(String::as_str),
        Some(expected_weak.as_str())
    );
    assert_eq!(
        rows.first().and_then(|row| row.get(2)).map(String::as_str),
        Some("1")
    );
    assert_eq!(
        rows.get(1).and_then(|row| row.get(2)).map(String::as_str),
        Some("1")
    );

    Ok(())
}

fn matching_rows(output: &str, chunk_ids: &[&str]) -> Vec<Vec<String>> {
    output
        .lines()
        .filter(|line| {
            chunk_ids
                .iter()
                .any(|chunk_id| line.contains(&format!("\"{chunk_id}\"")))
        })
        .map(|line| {
            line.split('|')
                .map(str::trim)
                .filter(|cell| !cell.is_empty())
                .map(ToString::to_string)
                .collect()
        })
        .collect()
}

fn run_mgconsole(cypher: &str) -> MemoryResult<String> {
    let mut child = Command::new("docker")
        .args(["compose", "exec", "-T", "memgraph", "mgconsole"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| MemoryError::StoreUnavailable(error.to_string()))?;

    let Some(mut stdin) = child.stdin.take() else {
        return Err(MemoryError::StoreUnavailable(
            "failed to open mgconsole stdin".to_string(),
        ));
    };
    {
        use std::io::Write;
        stdin
            .write_all(cypher.as_bytes())
            .map_err(|error| MemoryError::StoreUnavailable(error.to_string()))?;
    }
    drop(stdin);

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
