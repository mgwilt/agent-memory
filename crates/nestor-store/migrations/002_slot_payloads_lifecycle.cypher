CREATE CONSTRAINT ON (v:SlotValue) ASSERT v.value_type IS TYPED STRING;
CREATE CONSTRAINT ON (v:SlotValue) ASSERT v.value_norm IS TYPED STRING;
CREATE CONSTRAINT ON (c:Chunk) ASSERT c.base_level_cache_stale IS TYPED BOOLEAN;

CREATE INDEX ON :Chunk(agent_id, base_level_cache_stale);
CREATE INDEX ON :Chunk(agent_id, active, chunk_type);
CREATE INDEX ON :SlotValue(tenant_id, key, value_type);
