pub const CREATE_CHUNK: &str = r#"
MERGE (a:Agent {agent_id: $agent_id})
MERGE (c:Chunk {agent_id: $agent_id, chunk_id: $chunk_id})
SET c.chunk_type = $chunk_type,
    c.updated_at = datetime($updated_at),
    c.active = true,
    c.slot_hash = $slot_hash,
    c.base_bias = coalesce($base_bias, 0.0)
ON CREATE SET c.created_at = datetime($created_at),
              c.retrieval_count = 0,
              c.practice_count = 0
MERGE (a)-[:OWNS]->(c)
RETURN c.chunk_id AS chunk_id
"#;

pub const FETCH_CANDIDATES: &str = r#"
UNWIND $cue_slots AS cue
MATCH (v:SlotValue {tenant_id: $agent_id, key: cue.key, value_hash: cue.value_hash})<-[:HAS_SLOT]-(c:Chunk)
WHERE c.active = true
  AND ($chunk_type IS NULL OR c.chunk_type = $chunk_type)
WITH c, count(*) AS cue_matches
OPTIONAL MATCH (ctx:Chunk {agent_id: $agent_id, chunk_id: $context_chunk_id})-[a:ASSOCIATED]->(c)
WITH c, cue_matches, coalesce(max(a.strength), 0.0) AS assoc_boost
ORDER BY cue_matches DESC, assoc_boost DESC, c.last_access_at DESC
LIMIT $candidate_limit
RETURN c.chunk_id AS chunk_id,
       c.chunk_type AS chunk_type,
       c.retrieval_count AS retrieval_count,
       c.last_access_at AS last_access_at,
       c.base_bias AS base_bias
"#;

pub const APPEND_PRACTICE_EVENT: &str = r#"
MATCH (c:Chunk {agent_id: $agent_id, chunk_id: $chunk_id, active: true})
CREATE (e:PracticeEvent {
  event_id: $event_id,
  agent_id: $agent_id,
  chunk_id: $chunk_id,
  ts: datetime($occurred_at),
  kind: $kind,
  weight: $weight
})
CREATE (c)-[:HAS_EVENT]->(e)
SET c.last_practiced_at = datetime($occurred_at),
    c.practice_count = coalesce(c.practice_count, 0) + 1,
    c.updated_at = datetime($occurred_at)
RETURN e.event_id AS event_id
"#;

pub const UPSERT_ASSOCIATION: &str = r#"
MATCH (src:Chunk {agent_id: $agent_id, chunk_id: $src_chunk_id, active: true})
MATCH (dst:Chunk {agent_id: $agent_id, chunk_id: $dst_chunk_id, active: true})
MERGE (src)-[r:ASSOCIATED {source: $source}]->(dst)
SET r.strength = $strength,
    r.fan = $fan,
    r.updated_at = datetime($updated_at),
    r.created_at = coalesce(r.created_at, datetime($updated_at))
RETURN src.chunk_id AS src_chunk_id, dst.chunk_id AS dst_chunk_id
"#;

pub const SET_BUFFER_CURRENT: &str = r#"
MERGE (b:Buffer {agent_id: $agent_id, buffer_name: $buffer_name})
WITH b
OPTIONAL MATCH (b)-[old:CURRENT]->(:Chunk)
DELETE old
WITH b
MATCH (c:Chunk {agent_id: $agent_id, chunk_id: $chunk_id, active: true})
CREATE (b)-[:CURRENT {set_at: datetime($set_at)}]->(c)
RETURN b.buffer_name AS buffer_name, c.chunk_id AS chunk_id
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retrieval_query_keeps_candidate_limit() {
        assert!(FETCH_CANDIDATES.contains("LIMIT $candidate_limit"));
    }

    #[test]
    fn buffer_query_replaces_current_edge() {
        assert!(SET_BUFFER_CURRENT.contains("DELETE old"));
        assert!(SET_BUFFER_CURRENT.contains("CREATE (b)-[:CURRENT"));
    }
}
