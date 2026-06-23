#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CypherOperation {
    Read,
    Write,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RepositoryCypher {
    pub name: &'static str,
    pub operation: CypherOperation,
    pub requires_transaction: bool,
    pub text: &'static str,
}

pub const UPSERT_CHUNK: &str = r#"
MERGE (a:Agent {agent_id: $agent_id})
MERGE (c:Chunk {agent_id: $agent_id, chunk_id: $chunk_id})
ON CREATE SET c.created_at_ms = $created_at_ms,
              c.created_at = datetime($created_at),
              c.retrieval_count = 0,
              c.practice_count = 0,
              c.version = 1
SET c.chunk_type = $chunk_type,
    c.updated_at_ms = $updated_at_ms,
    c.updated_at = datetime($updated_at),
    c.active = true,
    c.slot_hash = $slot_hash,
    c.base_bias = coalesce($base_bias, 0.0)
MERGE (a)-[:OWNS]->(c)
WITH c
OPTIONAL MATCH (c)-[old_slot:HAS_SLOT]->(:SlotValue)
DELETE old_slot
WITH c
FOREACH (slot IN $slots |
  MERGE (v:SlotValue {
    tenant_id: $agent_id,
    key: slot.key,
    value_hash: slot.value_hash
  })
  SET v.value_norm = slot.value_norm,
      v.value_type = slot.value_type
  MERGE (c)-[has_slot:HAS_SLOT {key: slot.key}]->(v)
  SET has_slot.value_type = slot.value_type
)
WITH c
CREATE (e:PracticeEvent {
  event_id: $initial_practice_event_id,
  agent_id: $agent_id,
  chunk_id: $chunk_id,
  occurred_at_ms: $created_at_ms,
  ts: datetime($created_at),
  kind: "encode",
  weight: 1.0
})
CREATE (c)-[:HAS_EVENT]->(e)
SET c.first_practiced_at = coalesce(c.first_practiced_at, datetime($created_at)),
    c.last_practiced_at = datetime($created_at),
    c.practice_count = coalesce(c.practice_count, 0) + 1
RETURN c.chunk_id AS chunk_id,
       c.chunk_type AS chunk_type,
       c.version AS version
"#;

pub const GET_CHUNK: &str = r#"
MATCH (:Agent {agent_id: $agent_id})-[:OWNS]->(c:Chunk {chunk_id: $chunk_id})
WHERE c.active = true
OPTIONAL MATCH (c)-[slot_edge:HAS_SLOT]->(slot_value:SlotValue)
WITH c, slot_edge, slot_value
ORDER BY slot_edge.key ASC
RETURN c.agent_id AS agent_id,
       c.chunk_id AS chunk_id,
       c.chunk_type AS chunk_type,
       c.created_at_ms AS created_at_ms,
       c.updated_at_ms AS updated_at_ms,
       coalesce(c.retrieval_count, 0) AS retrieval_count,
       coalesce(c.base_bias, 0.0) AS base_bias,
       collect({
         key: slot_edge.key,
         value_type: slot_edge.value_type,
         value_norm: slot_value.value_norm,
         value_hash: slot_value.value_hash
       }) AS slots
"#;

pub const UPDATE_CHUNK: &str = r#"
MATCH (:Agent {agent_id: $agent_id})-[:OWNS]->(c:Chunk {chunk_id: $chunk_id})
WHERE c.active = true
  AND c.version = $expected_version
SET c.updated_at_ms = $updated_at_ms,
    c.updated_at = datetime($updated_at),
    c.slot_hash = $slot_hash,
    c.version = c.version + 1
WITH c
OPTIONAL MATCH (c)-[old_slot:HAS_SLOT]->(:SlotValue)
DELETE old_slot
WITH c
FOREACH (slot IN $slots |
  MERGE (v:SlotValue {
    tenant_id: $agent_id,
    key: slot.key,
    value_hash: slot.value_hash
  })
  SET v.value_norm = slot.value_norm,
      v.value_type = slot.value_type
  MERGE (c)-[has_slot:HAS_SLOT {key: slot.key}]->(v)
  SET has_slot.value_type = slot.value_type
)
RETURN c.chunk_id AS chunk_id,
       c.version AS version
"#;

pub const SOFT_DELETE_CHUNK: &str = r#"
MATCH (:Agent {agent_id: $agent_id})-[:OWNS]->(c:Chunk {chunk_id: $chunk_id})
WHERE c.active = true
SET c.active = false,
    c.deleted_at_ms = $deleted_at_ms,
    c.deleted_at = datetime($deleted_at),
    c.updated_at_ms = $deleted_at_ms,
    c.updated_at = datetime($deleted_at)
RETURN c.chunk_id AS deleted_chunk_id
"#;

pub const APPEND_PRACTICE_EVENT: &str = r#"
MATCH (c:Chunk {agent_id: $agent_id, chunk_id: $chunk_id, active: true})
CREATE (e:PracticeEvent {
  event_id: $event_id,
  agent_id: $agent_id,
  chunk_id: $chunk_id,
  occurred_at_ms: $occurred_at_ms,
  ts: datetime($occurred_at),
  kind: $kind,
  weight: $weight
})
CREATE (c)-[:HAS_EVENT]->(e)
SET c.last_practiced_at = datetime($occurred_at),
    c.practice_count = coalesce(c.practice_count, 0) + 1,
    c.updated_at_ms = $occurred_at_ms,
    c.updated_at = datetime($occurred_at)
RETURN e.event_id AS event_id
"#;

pub const FETCH_CANDIDATES: &str = r#"
MATCH (:Agent {agent_id: $agent_id})-[:OWNS]->(c:Chunk)
WHERE c.active = true
  AND ($chunk_type IS NULL OR c.chunk_type = $chunk_type)
OPTIONAL MATCH (c)-[matched_slot:HAS_SLOT]->(matched_value:SlotValue)
WHERE any(cue IN $cue_slots WHERE
  matched_slot.key = cue.key AND matched_value.value_hash = cue.value_hash
)
WITH c, count(DISTINCT matched_slot.key) AS cue_matches
WHERE size($cue_slots) = 0 OR cue_matches > 0
OPTIONAL MATCH (ctx:Chunk {agent_id: $agent_id})-[assoc:ASSOCIATED]->(c)
WHERE ctx.chunk_id IN $context_chunk_ids
WITH c, cue_matches, coalesce(sum(assoc.strength), 0.0) AS spread_score
OPTIONAL MATCH (c)-[:HAS_EVENT]->(event:PracticeEvent)
WITH c, cue_matches, spread_score, event
ORDER BY event.occurred_at_ms DESC
WITH c,
     cue_matches,
     spread_score,
     collect({
       occurred_at_ms: event.occurred_at_ms,
       weight: coalesce(event.weight, 1.0),
       kind: event.kind
     }) AS practice_events
OPTIONAL MATCH (c)-[slot_edge:HAS_SLOT]->(slot_value:SlotValue)
WITH c, cue_matches, spread_score, practice_events, slot_edge, slot_value
ORDER BY slot_edge.key ASC
WITH c,
     cue_matches,
     spread_score,
     practice_events,
     collect({
       key: slot_edge.key,
       value_type: slot_edge.value_type,
       value_norm: slot_value.value_norm,
       value_hash: slot_value.value_hash
     }) AS slots
RETURN c.agent_id AS agent_id,
       c.chunk_id AS chunk_id,
       c.chunk_type AS chunk_type,
       c.created_at_ms AS created_at_ms,
       c.updated_at_ms AS updated_at_ms,
       coalesce(c.retrieval_count, 0) AS retrieval_count,
       coalesce(c.base_bias, 0.0) AS base_bias,
       slots AS slots,
       practice_events AS practice_events,
       spread_score AS spread_score
ORDER BY cue_matches DESC,
         spread_score DESC,
         c.last_practiced_at DESC,
         c.chunk_id ASC
LIMIT $candidate_limit
"#;

pub const UPSERT_ASSOCIATION: &str = r#"
MATCH (src:Chunk {agent_id: $agent_id, chunk_id: $src_chunk_id, active: true})
MATCH (dst:Chunk {agent_id: $agent_id, chunk_id: $dst_chunk_id, active: true})
MERGE (src)-[r:ASSOCIATED {source: $source}]->(dst)
SET r.strength = $strength,
    r.fan = $fan,
    r.updated_at_ms = $updated_at_ms,
    r.updated_at = datetime($updated_at),
    r.created_at_ms = coalesce(r.created_at_ms, $updated_at_ms),
    r.created_at = coalesce(r.created_at, datetime($updated_at))
RETURN src.chunk_id AS src_chunk_id,
       dst.chunk_id AS dst_chunk_id,
       r.source AS source
"#;

pub const SET_BUFFER_CURRENT: &str = r#"
MERGE (b:Buffer {agent_id: $agent_id, buffer_name: $buffer_name})
WITH b
OPTIONAL MATCH (b)-[old:CURRENT]->(:Chunk)
DELETE old
WITH b
MATCH (c:Chunk {agent_id: $agent_id, chunk_id: $chunk_id, active: true})
CREATE (b)-[:CURRENT {set_at_ms: $set_at_ms, set_at: datetime($set_at)}]->(c)
RETURN b.buffer_name AS buffer_name,
       c.chunk_id AS chunk_id
"#;

pub const UPSERT_PRODUCTION_RULE: &str = r#"
MERGE (a:Agent {agent_id: $agent_id})
MERGE (p:ProductionRule {agent_id: $agent_id, rule_id: $rule_id})
ON CREATE SET p.created_at_ms = $updated_at_ms,
              p.version = $version
SET p.name = $name,
    p.enabled = $enabled,
    p.utility = $utility,
    p.success_count = $success_count,
    p.failure_count = $failure_count,
    p.avg_reward = $avg_reward,
    p.updated_at_ms = $updated_at_ms,
    p.version = $version
MERGE (a)-[:OWNS_RULE]->(p)
RETURN p.rule_id AS rule_id,
       p.version AS version
"#;

pub const GET_PRODUCTION_RULE: &str = r#"
MATCH (:Agent {agent_id: $agent_id})-[:OWNS_RULE]->(p:ProductionRule {rule_id: $rule_id})
RETURN p.agent_id AS agent_id,
       p.rule_id AS rule_id,
       p.name AS name,
       p.enabled AS enabled,
       p.utility AS utility,
       p.version AS version,
       coalesce(p.success_count, 0) AS success_count,
       coalesce(p.failure_count, 0) AS failure_count,
       coalesce(p.avg_reward, 0.0) AS avg_reward
"#;

pub const REPOSITORY_CYPHER: &[RepositoryCypher] = &[
    RepositoryCypher {
        name: "upsert_chunk",
        operation: CypherOperation::Write,
        requires_transaction: true,
        text: UPSERT_CHUNK,
    },
    RepositoryCypher {
        name: "get_chunk",
        operation: CypherOperation::Read,
        requires_transaction: false,
        text: GET_CHUNK,
    },
    RepositoryCypher {
        name: "update_chunk",
        operation: CypherOperation::Write,
        requires_transaction: true,
        text: UPDATE_CHUNK,
    },
    RepositoryCypher {
        name: "soft_delete_chunk",
        operation: CypherOperation::Write,
        requires_transaction: true,
        text: SOFT_DELETE_CHUNK,
    },
    RepositoryCypher {
        name: "append_practice_event",
        operation: CypherOperation::Write,
        requires_transaction: true,
        text: APPEND_PRACTICE_EVENT,
    },
    RepositoryCypher {
        name: "fetch_candidates",
        operation: CypherOperation::Read,
        requires_transaction: false,
        text: FETCH_CANDIDATES,
    },
    RepositoryCypher {
        name: "upsert_association",
        operation: CypherOperation::Write,
        requires_transaction: true,
        text: UPSERT_ASSOCIATION,
    },
    RepositoryCypher {
        name: "set_buffer_current",
        operation: CypherOperation::Write,
        requires_transaction: true,
        text: SET_BUFFER_CURRENT,
    },
    RepositoryCypher {
        name: "upsert_production_rule",
        operation: CypherOperation::Write,
        requires_transaction: true,
        text: UPSERT_PRODUCTION_RULE,
    },
    RepositoryCypher {
        name: "get_production_rule",
        operation: CypherOperation::Read,
        requires_transaction: false,
        text: GET_PRODUCTION_RULE,
    },
];

pub fn repository_cypher() -> &'static [RepositoryCypher] {
    REPOSITORY_CYPHER
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repository_cypher_covers_all_persistence_shapes() {
        let names = repository_cypher()
            .iter()
            .map(|statement| statement.name)
            .collect::<Vec<_>>();

        assert!(names.contains(&"upsert_chunk"));
        assert!(names.contains(&"append_practice_event"));
        assert!(names.contains(&"upsert_association"));
        assert!(names.contains(&"set_buffer_current"));
        assert!(names.contains(&"upsert_production_rule"));
    }

    #[test]
    fn write_queries_are_marked_transactional() {
        for statement in repository_cypher()
            .iter()
            .filter(|statement| statement.operation == CypherOperation::Write)
        {
            assert!(
                statement.requires_transaction,
                "{} must run in a request-scoped transaction",
                statement.name
            );
        }
    }

    #[test]
    fn retrieval_query_keeps_candidate_limit_and_symbolic_filtering() {
        assert!(FETCH_CANDIDATES.contains("LIMIT $candidate_limit"));
        assert!(FETCH_CANDIDATES.contains("HAS_SLOT"));
        assert!(FETCH_CANDIDATES.contains("value_hash"));
        assert!(FETCH_CANDIDATES.contains("spread_score"));
    }

    #[test]
    fn chunk_write_persists_slot_values_and_initial_practice_event() {
        assert!(UPSERT_CHUNK.contains("SlotValue"));
        assert!(UPSERT_CHUNK.contains("HAS_SLOT"));
        assert!(UPSERT_CHUNK.contains("PracticeEvent"));
        assert!(UPSERT_CHUNK.contains("initial_practice_event_id"));
    }

    #[test]
    fn activation_scoring_is_not_embedded_in_cypher() {
        for statement in repository_cypher() {
            let text = statement.text.to_ascii_lowercase();
            assert!(!text.contains("pow("));
            assert!(!text.contains("exp("));
            assert!(!text.contains("base_level_activation"));
            assert!(!text.contains("retrieval_probability"));
        }
    }

    #[test]
    fn buffer_query_replaces_current_edge() {
        assert!(SET_BUFFER_CURRENT.contains("DELETE old"));
        assert!(SET_BUFFER_CURRENT.contains("CREATE (b)-[:CURRENT"));
    }
}
