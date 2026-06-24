use std::collections::{BTreeMap, HashMap};

use chrono::{DateTime, Utc};
use neo4rs::{BoltType, ConfigBuilder, Graph, Query, Row, query};
use nestor_core::{
    AgentId, Chunk, ChunkId, ChunkType, MemoryError, MemoryResult, PracticeEvent, Slot, SlotValue,
};
use nestor_rules::{BufferCondition, ProductionRule, RetrievedChunkCondition, RuleId};
use nestor_session::BufferName;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::{
    cypher::{
        APPEND_PRACTICE_EVENT, ASSOCIATION_LINK_COUNT, FETCH_CANDIDATES, GET_CHUNK,
        GET_PRODUCTION_RULE, RECORD_SUCCESSFUL_RETRIEVAL, SET_BUFFER_CURRENT, SET_CHUNK_SLOT,
        SOFT_DELETE_CHUNK, UPDATE_CHUNK, UPSERT_ASSOCIATION, UPSERT_CHUNK, UPSERT_PRODUCTION_RULE,
    },
    memgraph::MemgraphRepositoryConfig,
    repository::{
        AssociationWrite, BufferSetCurrent, CandidateQuery, ChunkWithHistory, ConsolidateRequest,
        ConsolidationGroupReport, ConsolidationReport, CreateChunk, ForgetReport, ForgetRequest,
        MAX_CANDIDATE_LIMIT, MemoryRepository, PracticeEventWrite, ProductionRuleRecord,
        RetrievalPracticeWrite, StoredSlot, UpdateChunk, chunk_slot_hash, stored_slots_from_chunk,
        stored_slots_from_slots,
    },
};

#[derive(Clone)]
pub struct MemgraphRepository {
    graph: Graph,
}

impl MemgraphRepository {
    pub async fn connect(config: MemgraphRepositoryConfig) -> MemoryResult<Self> {
        let mut builder = ConfigBuilder::default()
            .uri(config.uri)
            .user(config.user)
            .password(config.password)
            .db(config.database)
            .max_connections(config.max_connections);
        if let Some(ca_file) = config.tls_ca_file {
            builder = builder.with_client_certificate(ca_file);
        }
        let graph = Graph::connect(builder.build().map_err(map_neo4rs_error)?)
            .await
            .map_err(map_neo4rs_error)?;
        Ok(Self { graph })
    }

    async fn read_one(&self, query: Query) -> MemoryResult<Option<Row>> {
        let mut stream = self.graph.execute(query).await.map_err(map_neo4rs_error)?;
        stream.next().await.map_err(map_neo4rs_error)
    }

    async fn read_all(&self, query: Query) -> MemoryResult<Vec<Row>> {
        let mut stream = self.graph.execute(query).await.map_err(map_neo4rs_error)?;
        let mut rows = Vec::new();
        while let Some(row) = stream.next().await.map_err(map_neo4rs_error)? {
            rows.push(row);
        }
        Ok(rows)
    }

    async fn write_one(&self, query: Query) -> MemoryResult<Option<Row>> {
        let mut txn = self.graph.start_txn().await.map_err(map_neo4rs_error)?;
        let mut stream = txn.execute(query).await.map_err(map_neo4rs_error)?;
        let row = stream.next(&mut txn).await.map_err(map_neo4rs_error)?;
        drop(stream);
        txn.commit().await.map_err(map_neo4rs_error)?;
        Ok(row)
    }

    async fn set_slot_value(
        &self,
        agent_id: &AgentId,
        chunk_id: &ChunkId,
        slot: Slot,
        updated_at_ms: u64,
    ) -> MemoryResult<()> {
        let mut chunk = self
            .get_chunk(agent_id, chunk_id)
            .await?
            .ok_or_else(|| MemoryError::NotFound(format!("chunk {}", chunk_id.as_str())))?;
        chunk.upsert_slot(slot.key.clone(), slot.value.clone());
        let stored = StoredSlot::from_slot(&slot);
        let row = self
            .write_one(
                query(SET_CHUNK_SLOT)
                    .param("agent_id", agent_id.as_str())
                    .param("chunk_id", chunk_id.as_str())
                    .param("key", stored.key)
                    .param("value_type", stored.value_type)
                    .param("value_norm", stored.value_norm)
                    .param("value_hash", stored.value_hash)
                    .param("value_symbol", stored.value_symbol)
                    .param("value_text", stored.value_text)
                    .param("value_number", stored.value_number)
                    .param("value_bool", stored.value_bool)
                    .param("updated_at_ms", i64_from_u64(updated_at_ms)?)
                    .param("updated_at", datetime_ms(updated_at_ms))
                    .param("slot_hash", chunk_slot_hash(&chunk.slots)),
            )
            .await?;
        require_row(row, "chunk")?;
        Ok(())
    }

    async fn association_link_count(
        &self,
        agent_id: &AgentId,
        chunk_id: &ChunkId,
    ) -> MemoryResult<usize> {
        let row = self
            .read_one(
                query(ASSOCIATION_LINK_COUNT)
                    .param("agent_id", agent_id.as_str())
                    .param("chunk_id", chunk_id.as_str()),
            )
            .await?
            .ok_or_else(|| MemoryError::NotFound(format!("chunk {}", chunk_id.as_str())))?;
        let count: i64 = row_get(&row, "link_count")?;
        usize_from_i64(count, "link_count")
    }
}

#[async_trait::async_trait]
impl MemoryRepository for MemgraphRepository {
    async fn health_check(&self) -> MemoryResult<()> {
        self.read_one(query("RETURN 1 AS ok")).await?;
        Ok(())
    }

    async fn create_chunk(&self, req: CreateChunk) -> MemoryResult<Chunk> {
        req.validate()?;
        if self
            .get_chunk(&req.chunk.agent_id, &req.chunk.chunk_id)
            .await?
            .is_some()
        {
            return Err(MemoryError::Conflict(format!(
                "chunk {} already exists",
                req.chunk.chunk_id.as_str()
            )));
        }
        self.upsert_chunk(req).await
    }

    async fn upsert_chunk(&self, req: CreateChunk) -> MemoryResult<Chunk> {
        req.validate()?;
        let row = self.write_one(upsert_chunk_query(&req)?).await?;
        require_row(row, "chunk")?;
        self.get_chunk(&req.chunk.agent_id, &req.chunk.chunk_id)
            .await?
            .ok_or_else(|| MemoryError::NotFound(format!("chunk {}", req.chunk.chunk_id.as_str())))
    }

    async fn get_chunk(
        &self,
        agent_id: &AgentId,
        chunk_id: &ChunkId,
    ) -> MemoryResult<Option<Chunk>> {
        self.read_one(
            query(GET_CHUNK)
                .param("agent_id", agent_id.as_str())
                .param("chunk_id", chunk_id.as_str()),
        )
        .await?
        .map(|row| row_to_chunk(&row))
        .transpose()
    }

    async fn update_chunk(&self, req: UpdateChunk) -> MemoryResult<Chunk> {
        req.validate()?;
        let current = self
            .get_chunk(&req.agent_id, &req.chunk_id)
            .await?
            .ok_or_else(|| MemoryError::NotFound(format!("chunk {}", req.chunk_id.as_str())))?;
        let updated_at_ms = current.updated_at_ms.saturating_add(1);
        let row = self
            .write_one(update_chunk_query(&req, updated_at_ms)?)
            .await?;
        if row.is_none() {
            return Err(MemoryError::Conflict(format!(
                "chunk {} version conflict",
                req.chunk_id.as_str()
            )));
        }
        self.get_chunk(&req.agent_id, &req.chunk_id)
            .await?
            .ok_or_else(|| MemoryError::NotFound(format!("chunk {}", req.chunk_id.as_str())))
    }

    async fn soft_delete_chunk(&self, agent_id: &AgentId, chunk_id: &ChunkId) -> MemoryResult<()> {
        let now_ms = now_ms();
        let row = self
            .write_one(
                query(SOFT_DELETE_CHUNK)
                    .param("agent_id", agent_id.as_str())
                    .param("chunk_id", chunk_id.as_str())
                    .param("deleted_at_ms", i64_from_u64(now_ms)?)
                    .param("deleted_at", datetime_ms(now_ms)),
            )
            .await?;
        require_row(row, "chunk")?;
        Ok(())
    }

    async fn append_practice_event(&self, req: PracticeEventWrite) -> MemoryResult<()> {
        req.validate()?;
        let row = self
            .write_one(
                query(APPEND_PRACTICE_EVENT)
                    .param("agent_id", req.agent_id.as_str())
                    .param("chunk_id", req.chunk_id.as_str())
                    .param("event_id", req.event_id)
                    .param("occurred_at_ms", i64_from_u64(req.occurred_at_ms)?)
                    .param("occurred_at", datetime_ms(req.occurred_at_ms))
                    .param("kind", req.kind)
                    .param("weight", req.weight),
            )
            .await?;
        require_row(row, "practice event")?;
        Ok(())
    }

    async fn record_successful_retrieval(&self, req: RetrievalPracticeWrite) -> MemoryResult<()> {
        req.validate()?;
        let row = self
            .write_one(
                query(RECORD_SUCCESSFUL_RETRIEVAL)
                    .param("agent_id", req.agent_id.as_str())
                    .param("chunk_id", req.chunk_id.as_str())
                    .param("event_id", req.event_id)
                    .param("occurred_at_ms", i64_from_u64(req.occurred_at_ms)?)
                    .param("occurred_at", datetime_ms(req.occurred_at_ms))
                    .param("weight", req.weight),
            )
            .await?;
        require_row(row, "retrieval event")?;
        Ok(())
    }

    async fn fetch_candidates(&self, req: CandidateQuery) -> MemoryResult<Vec<ChunkWithHistory>> {
        req.validate()?;
        let cue_slots = req.normalized_cue_slots();
        let rows = self
            .read_all(
                query(FETCH_CANDIDATES)
                    .param("agent_id", req.agent_id.as_str())
                    .param("chunk_type", req.chunk_type.clone())
                    .param("cue_slots", stored_slot_params(cue_slots)?)
                    .param("context_chunk_ids", string_list(req.context_chunk_ids))
                    .param("candidate_limit", i64_from_usize(req.candidate_limit)?),
            )
            .await?;
        rows.iter().map(row_to_candidate).collect()
    }

    async fn upsert_association(&self, req: AssociationWrite) -> MemoryResult<()> {
        req.validate()?;
        let row = self
            .write_one(
                query(UPSERT_ASSOCIATION)
                    .param("agent_id", req.agent_id.as_str())
                    .param("src_chunk_id", req.src_chunk_id.as_str())
                    .param("dst_chunk_id", req.dst_chunk_id.as_str())
                    .param("source", req.source)
                    .param("strength", req.strength)
                    .param("fan", i64_from_u64(req.fan)?)
                    .param("updated_at_ms", i64_from_u64(req.updated_at_ms)?)
                    .param("updated_at", datetime_ms(req.updated_at_ms)),
            )
            .await?;
        require_row(row, "association")?;
        Ok(())
    }

    async fn set_buffer_current(&self, req: BufferSetCurrent) -> MemoryResult<()> {
        req.validate()?;
        let row = self
            .write_one(
                query(SET_BUFFER_CURRENT)
                    .param("agent_id", req.agent_id.as_str())
                    .param("buffer_name", req.buffer_name.as_str())
                    .param("chunk_id", req.chunk_id.as_str())
                    .param("set_at_ms", i64_from_u64(req.set_at_ms)?)
                    .param("set_at", datetime_ms(req.set_at_ms)),
            )
            .await?;
        require_row(row, "buffer")?;
        Ok(())
    }

    async fn upsert_production_rule(
        &self,
        req: ProductionRuleRecord,
    ) -> MemoryResult<ProductionRuleRecord> {
        req.validate()?;
        let row = self.write_one(upsert_rule_query(&req)?).await?;
        require_row(row, "production rule")?;
        Ok(req)
    }

    async fn get_production_rule(
        &self,
        agent_id: &AgentId,
        rule_id: &RuleId,
    ) -> MemoryResult<Option<ProductionRuleRecord>> {
        self.read_one(
            query(GET_PRODUCTION_RULE)
                .param("agent_id", agent_id.as_str())
                .param("rule_id", rule_id.as_str()),
        )
        .await?
        .map(|row| row_to_production_rule(&row))
        .transpose()
    }

    async fn consolidate(&self, req: ConsolidateRequest) -> MemoryResult<ConsolidationReport> {
        req.validate()?;
        let group_keys = effective_group_slot_keys(&req.group_slot_keys);
        let candidates = self
            .fetch_candidates(CandidateQuery {
                agent_id: req.agent_id.clone(),
                chunk_type: req
                    .chunk_type
                    .as_ref()
                    .map(|chunk_type| chunk_type.0.clone()),
                cue_slots: Vec::new(),
                context_chunk_ids: Vec::new(),
                candidate_limit: MAX_CANDIDATE_LIMIT,
            })
            .await?;
        let mut groups = BTreeMap::<String, Vec<Chunk>>::new();
        for candidate in candidates {
            if is_bool_slot(&candidate.chunk, "consolidated") {
                continue;
            }
            if let Some(key) = consolidation_group_key(&candidate.chunk, &group_keys) {
                groups.entry(key).or_default().push(candidate.chunk);
            }
        }

        let groups_considered = groups.len();
        let mut summaries = Vec::new();
        for (group_key, mut group_chunks) in groups {
            if group_chunks.len() < req.min_group_size {
                continue;
            }
            group_chunks.sort_by(|left, right| left.chunk_id.cmp(&right.chunk_id));
            let source_chunk_ids = group_chunks
                .iter()
                .map(|chunk| chunk.chunk_id.clone())
                .collect::<Vec<_>>();
            let summary_id = ChunkId::from(format!(
                "summary-{}",
                stable_hex_hash(&[
                    req.agent_id.as_str(),
                    req.summary_chunk_type.as_str(),
                    &group_key,
                ])
            ));
            let mut summary = Chunk::new(
                req.agent_id.clone(),
                summary_id.clone(),
                req.summary_chunk_type.clone(),
                req.now_ms,
            );
            for key in &group_keys {
                if let Some(value) = group_chunks[0].slot(key).cloned() {
                    summary.upsert_slot(key.clone(), value);
                }
            }
            summary.upsert_slot(
                "source_count",
                SlotValue::Number(source_chunk_ids.len() as f64),
            );
            summary.upsert_slot("consolidation_key", SlotValue::Text(group_key));
            self.upsert_chunk(CreateChunk {
                chunk: summary,
                initial_practice_event_id: format!(
                    "consolidate-{}-{}-{}",
                    req.agent_id.as_str(),
                    summary_id.as_str(),
                    req.now_ms
                ),
            })
            .await?;

            for chunk_id in &source_chunk_ids {
                self.set_slot_value(
                    &req.agent_id,
                    chunk_id,
                    Slot::new("consolidated", SlotValue::Bool(true)),
                    req.now_ms,
                )
                .await?;
                self.upsert_association(AssociationWrite {
                    agent_id: req.agent_id.clone(),
                    src_chunk_id: summary_id.clone(),
                    dst_chunk_id: chunk_id.clone(),
                    source: "consolidation".to_string(),
                    strength: 1.0,
                    fan: source_chunk_ids.len() as u64,
                    updated_at_ms: req.now_ms,
                })
                .await?;
            }
            summaries.push(ConsolidationGroupReport {
                summary_chunk_id: summary_id,
                source_chunk_ids,
            });
        }

        Ok(ConsolidationReport {
            agent_id: req.agent_id,
            groups_considered,
            groups_consolidated: summaries.len(),
            summaries,
        })
    }

    async fn forget(&self, req: ForgetRequest) -> MemoryResult<ForgetReport> {
        req.validate()?;
        let candidates = self
            .fetch_candidates(CandidateQuery {
                agent_id: req.agent_id.clone(),
                chunk_type: req
                    .chunk_type
                    .as_ref()
                    .map(|chunk_type| chunk_type.0.clone()),
                cue_slots: Vec::new(),
                context_chunk_ids: Vec::new(),
                candidate_limit: MAX_CANDIDATE_LIMIT,
            })
            .await?;
        let mut forgotten_chunk_ids = Vec::new();
        let mut archived_chunk_ids = Vec::new();
        let mut protected_chunk_ids = Vec::new();

        for candidate in &candidates {
            let chunk = &candidate.chunk;
            if is_bool_slot(chunk, "protected") {
                protected_chunk_ids.push(chunk.chunk_id.clone());
                continue;
            }
            let last_practiced_at = candidate
                .practice_events
                .iter()
                .map(|event| event.occurred_at_ms)
                .max()
                .unwrap_or(chunk.updated_at_ms);
            if last_practiced_at > req.recency_cutoff_ms {
                continue;
            }
            let base_level =
                nestor_core::base_level_activation(&candidate.practice_events, req.now_ms, 0.5);
            if base_level >= req.base_level_cutoff {
                continue;
            }
            let linked = self
                .association_link_count(&req.agent_id, &chunk.chunk_id)
                .await?
                > 0;
            if linked && !req.allow_linked_forget {
                self.set_slot_value(
                    &req.agent_id,
                    &chunk.chunk_id,
                    Slot::new("archived", SlotValue::Bool(true)),
                    req.now_ms,
                )
                .await?;
                archived_chunk_ids.push(chunk.chunk_id.clone());
            } else {
                self.soft_delete_chunk(&req.agent_id, &chunk.chunk_id)
                    .await?;
                forgotten_chunk_ids.push(chunk.chunk_id.clone());
            }
        }

        Ok(ForgetReport {
            agent_id: req.agent_id,
            examined: candidates.len(),
            forgotten_chunk_ids,
            archived_chunk_ids,
            protected_chunk_ids,
        })
    }
}

#[derive(Debug, Deserialize)]
struct SlotRow {
    key: Option<String>,
    value_type: Option<String>,
    value_norm: Option<String>,
    value_hash: Option<String>,
    value_symbol: Option<String>,
    value_text: Option<String>,
    value_number: Option<String>,
    value_bool: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct PracticeEventRow {
    occurred_at_ms: Option<i64>,
    weight: Option<f64>,
}

fn upsert_chunk_query(req: &CreateChunk) -> MemoryResult<Query> {
    Ok(query(UPSERT_CHUNK)
        .param("agent_id", req.chunk.agent_id.as_str())
        .param("chunk_id", req.chunk.chunk_id.as_str())
        .param("chunk_type", req.chunk.chunk_type.as_str())
        .param("created_at_ms", i64_from_u64(req.chunk.created_at_ms)?)
        .param("created_at", datetime_ms(req.chunk.created_at_ms))
        .param("updated_at_ms", i64_from_u64(req.chunk.updated_at_ms)?)
        .param("updated_at", datetime_ms(req.chunk.updated_at_ms))
        .param("slot_hash", chunk_slot_hash(&req.chunk.slots))
        .param("base_bias", req.chunk.base_bias)
        .param(
            "initial_practice_event_id",
            req.initial_practice_event_id.clone(),
        )
        .param(
            "slots",
            stored_slot_params(stored_slots_from_chunk(&req.chunk))?,
        ))
}

fn update_chunk_query(req: &UpdateChunk, updated_at_ms: u64) -> MemoryResult<Query> {
    let mut slots = BTreeMap::new();
    for slot in &req.slots {
        slots.insert(slot.key.clone(), slot.value.clone());
    }
    Ok(query(UPDATE_CHUNK)
        .param("agent_id", req.agent_id.as_str())
        .param("chunk_id", req.chunk_id.as_str())
        .param("expected_version", i64_from_u64(req.expected_version)?)
        .param("updated_at_ms", i64_from_u64(updated_at_ms)?)
        .param("updated_at", datetime_ms(updated_at_ms))
        .param("slot_hash", chunk_slot_hash(&slots))
        .param(
            "slots",
            stored_slot_params(stored_slots_from_slots(&req.slots))?,
        ))
}

fn upsert_rule_query(req: &ProductionRuleRecord) -> MemoryResult<Query> {
    Ok(query(UPSERT_PRODUCTION_RULE)
        .param("agent_id", req.agent_id.as_str())
        .param("rule_id", req.rule.rule_id.as_str())
        .param("name", req.rule.name.clone())
        .param("enabled", req.rule.enabled)
        .param("utility", req.rule.utility)
        .param("version", i64_from_u64(req.rule.version)?)
        .param("conditions_json", conditions_json(&req.rule.conditions)?)
        .param(
            "retrieved_chunk_json",
            retrieved_chunk_json(&req.rule.retrieved_chunk)?,
        )
        .param("success_count", i64_from_u64(req.success_count)?)
        .param("failure_count", i64_from_u64(req.failure_count)?)
        .param("avg_reward", req.avg_reward)
        .param("updated_at_ms", i64_from_u64(now_ms())?))
}

fn stored_slot_params(slots: Vec<StoredSlot>) -> MemoryResult<Vec<HashMap<String, BoltType>>> {
    slots
        .into_iter()
        .map(|slot| {
            let mut map = HashMap::new();
            map.insert("key".to_string(), slot.key.into());
            map.insert("value_type".to_string(), slot.value_type.into());
            map.insert("value_norm".to_string(), slot.value_norm.into());
            map.insert("value_hash".to_string(), slot.value_hash.into());
            map.insert("value_symbol".to_string(), slot.value_symbol.into());
            map.insert("value_text".to_string(), slot.value_text.into());
            map.insert("value_number".to_string(), slot.value_number.into());
            map.insert("value_bool".to_string(), slot.value_bool.into());
            Ok(map)
        })
        .collect()
}

fn row_to_chunk(row: &Row) -> MemoryResult<Chunk> {
    let agent_id: String = row_get(row, "agent_id")?;
    let chunk_id: String = row_get(row, "chunk_id")?;
    let chunk_type: String = row_get(row, "chunk_type")?;
    let created_at_ms: i64 = row_get(row, "created_at_ms")?;
    let updated_at_ms: i64 = row_get(row, "updated_at_ms")?;
    let retrieval_count: i64 = row_get(row, "retrieval_count")?;
    let base_bias: f64 = row_get(row, "base_bias")?;
    let slots: Vec<SlotRow> = row_get(row, "slots")?;

    let mut chunk = Chunk::new(
        AgentId::from(agent_id),
        ChunkId::from(chunk_id),
        ChunkType::from(chunk_type),
        u64_from_i64(created_at_ms, "created_at_ms")?,
    );
    chunk.updated_at_ms = u64_from_i64(updated_at_ms, "updated_at_ms")?;
    chunk.retrieval_count = u64_from_i64(retrieval_count, "retrieval_count")?;
    chunk.base_bias = base_bias;
    for slot in slots {
        if let Some(stored) = slot.try_into_stored()? {
            chunk.upsert_slot(stored.key.clone(), stored.slot_value()?);
        }
    }
    Ok(chunk)
}

fn row_to_candidate(row: &Row) -> MemoryResult<ChunkWithHistory> {
    let chunk = row_to_chunk(row)?;
    let spread_score: f64 = row_get(row, "spread_score")?;
    let base_level_cache_stale: bool = row_get(row, "base_level_cache_stale")?;
    let practice_rows: Vec<PracticeEventRow> = row_get(row, "practice_events")?;
    let practice_events = practice_rows
        .into_iter()
        .filter_map(|event| {
            Some(PracticeEvent {
                occurred_at_ms: u64_from_i64(event.occurred_at_ms?, "occurred_at_ms").ok()?,
                weight: event.weight.unwrap_or(1.0),
            })
        })
        .collect();
    Ok(ChunkWithHistory {
        chunk,
        practice_events,
        spread_score,
        base_level_cache_stale,
    })
}

fn row_to_production_rule(row: &Row) -> MemoryResult<ProductionRuleRecord> {
    let agent_id: String = row_get(row, "agent_id")?;
    let rule_id: String = row_get(row, "rule_id")?;
    let name: String = row_get(row, "name")?;
    let enabled: bool = row_get(row, "enabled")?;
    let utility: f64 = row_get(row, "utility")?;
    let version: i64 = row_get(row, "version")?;
    let success_count: i64 = row_get(row, "success_count")?;
    let failure_count: i64 = row_get(row, "failure_count")?;
    let avg_reward: f64 = row_get(row, "avg_reward")?;
    let conditions_json: String = row_get(row, "conditions_json")?;
    let retrieved_chunk_json: Option<String> = row_get(row, "retrieved_chunk_json")?;
    let conditions = parse_conditions_json(&conditions_json)?;
    let retrieved_chunk = match retrieved_chunk_json {
        Some(value) if !value.trim().is_empty() => Some(parse_retrieved_chunk_json(&value)?),
        _ => None,
    };

    Ok(ProductionRuleRecord {
        agent_id: AgentId::from(agent_id),
        rule: ProductionRule {
            rule_id: RuleId::from(rule_id),
            name,
            enabled,
            utility,
            version: u64_from_i64(version, "version")?,
            conditions,
            retrieved_chunk,
        },
        success_count: u64_from_i64(success_count, "success_count")?,
        failure_count: u64_from_i64(failure_count, "failure_count")?,
        avg_reward,
    })
}

impl SlotRow {
    fn try_into_stored(self) -> MemoryResult<Option<StoredSlot>> {
        let Some(key) = self.key else {
            return Ok(None);
        };
        let Some(value_type) = self.value_type else {
            return Ok(None);
        };
        let Some(value_norm) = self.value_norm else {
            return Ok(None);
        };
        let Some(value_hash) = self.value_hash else {
            return Ok(None);
        };
        let value_type: &'static str = match value_type.as_str() {
            "symbol" => "symbol",
            "text" => "text",
            "number" => "number",
            "bool" => "bool",
            other => {
                return Err(MemoryError::Serialization(format!(
                    "unknown slot value type {other}"
                )));
            }
        };
        Ok(Some(StoredSlot {
            key,
            value_type,
            value_norm,
            value_hash,
            value_symbol: self.value_symbol,
            value_text: self.value_text,
            value_number: self.value_number,
            value_bool: self.value_bool,
        }))
    }
}

fn conditions_json(conditions: &[BufferCondition]) -> MemoryResult<String> {
    serde_json::to_string(
        &conditions
            .iter()
            .map(|condition| {
                json!({
                    "buffer": condition.buffer.as_str(),
                    "chunk_id": condition.chunk_id.as_ref().map(|value| value.as_str()),
                    "chunk_type": condition.chunk_type.as_ref().map(|value| value.as_str()),
                })
            })
            .collect::<Vec<_>>(),
    )
    .map_err(|error| MemoryError::Serialization(error.to_string()))
}

fn retrieved_chunk_json(
    condition: &Option<RetrievedChunkCondition>,
) -> MemoryResult<Option<String>> {
    condition
        .as_ref()
        .map(|condition| {
            serde_json::to_string(&json!({
                "chunk_id": condition.chunk_id.as_ref().map(|value| value.as_str()),
                "chunk_type": condition.chunk_type.as_ref().map(|value| value.as_str()),
                "slots": condition.slots.iter().map(slot_json).collect::<Vec<_>>(),
            }))
            .map_err(|error| MemoryError::Serialization(error.to_string()))
        })
        .transpose()
}

fn parse_conditions_json(raw: &str) -> MemoryResult<Vec<BufferCondition>> {
    let values: Vec<Value> =
        serde_json::from_str(raw).map_err(|error| MemoryError::Serialization(error.to_string()))?;
    values
        .into_iter()
        .map(|value| {
            let buffer = value.get("buffer").and_then(Value::as_str).ok_or_else(|| {
                MemoryError::Serialization("rule condition missing buffer".to_string())
            })?;
            Ok(BufferCondition {
                buffer: parse_buffer_name(buffer),
                chunk_id: value
                    .get("chunk_id")
                    .and_then(Value::as_str)
                    .map(ChunkId::from),
                chunk_type: value
                    .get("chunk_type")
                    .and_then(Value::as_str)
                    .map(ChunkType::from),
            })
        })
        .collect()
}

fn parse_retrieved_chunk_json(raw: &str) -> MemoryResult<RetrievedChunkCondition> {
    let value: Value =
        serde_json::from_str(raw).map_err(|error| MemoryError::Serialization(error.to_string()))?;
    let slots = value
        .get("slots")
        .and_then(Value::as_array)
        .map(|slots| slots.iter().map(parse_slot_json).collect())
        .transpose()?
        .unwrap_or_default();
    Ok(RetrievedChunkCondition {
        chunk_id: value
            .get("chunk_id")
            .and_then(Value::as_str)
            .map(ChunkId::from),
        chunk_type: value
            .get("chunk_type")
            .and_then(Value::as_str)
            .map(ChunkType::from),
        slots,
    })
}

fn slot_json(slot: &Slot) -> Value {
    json!({
        "key": slot.key,
        "value": match &slot.value {
            SlotValue::Symbol(value) => json!({"type": "symbol", "value": value}),
            SlotValue::Text(value) => json!({"type": "text", "value": value}),
            SlotValue::Number(value) => json!({"type": "number", "value": value}),
            SlotValue::Bool(value) => json!({"type": "bool", "value": value}),
        }
    })
}

fn parse_slot_json(value: &Value) -> MemoryResult<Slot> {
    let key = value
        .get("key")
        .and_then(Value::as_str)
        .ok_or_else(|| MemoryError::Serialization("slot missing key".to_string()))?;
    let typed_value = value
        .get("value")
        .ok_or_else(|| MemoryError::Serialization("slot missing value".to_string()))?;
    let value_type = typed_value
        .get("type")
        .and_then(Value::as_str)
        .ok_or_else(|| MemoryError::Serialization("slot value missing type".to_string()))?;
    let value = typed_value
        .get("value")
        .ok_or_else(|| MemoryError::Serialization("slot value missing payload".to_string()))?;
    let slot_value = match value_type {
        "symbol" => SlotValue::Symbol(
            value
                .as_str()
                .ok_or_else(|| {
                    MemoryError::Serialization("symbol slot must be string".to_string())
                })?
                .to_string(),
        ),
        "text" => SlotValue::Text(
            value
                .as_str()
                .ok_or_else(|| MemoryError::Serialization("text slot must be string".to_string()))?
                .to_string(),
        ),
        "number" => SlotValue::Number(value.as_f64().ok_or_else(|| {
            MemoryError::Serialization("number slot must be numeric".to_string())
        })?),
        "bool" => {
            SlotValue::Bool(value.as_bool().ok_or_else(|| {
                MemoryError::Serialization("bool slot must be boolean".to_string())
            })?)
        }
        other => {
            return Err(MemoryError::Serialization(format!(
                "unknown slot value type {other}"
            )));
        }
    };
    Ok(Slot::new(key, slot_value))
}

fn parse_buffer_name(value: &str) -> BufferName {
    match value {
        "goal" => BufferName::Goal,
        "retrieval" => BufferName::Retrieval,
        "imaginal" => BufferName::Imaginal,
        "task" => BufferName::Task,
        other => BufferName::Custom(other.to_string()),
    }
}

fn effective_group_slot_keys(keys: &[String]) -> Vec<String> {
    if keys.is_empty() {
        vec!["topic".to_string()]
    } else {
        keys.to_vec()
    }
}

fn consolidation_group_key(chunk: &Chunk, keys: &[String]) -> Option<String> {
    let mut parts = Vec::new();
    for key in keys {
        let value = chunk.slot(key)?;
        parts.push(format!(
            "{key}={}:{}",
            value.value_type(),
            value.normalized()
        ));
    }
    Some(parts.join("|"))
}

fn is_bool_slot(chunk: &Chunk, key: &str) -> bool {
    matches!(chunk.slot(key), Some(SlotValue::Bool(true)))
}

fn string_list(values: Vec<ChunkId>) -> Vec<String> {
    values.into_iter().map(|value| value.0).collect()
}

fn row_get<'a, T>(row: &'a Row, key: &str) -> MemoryResult<T>
where
    T: serde::Deserialize<'a>,
{
    row.get(key).map_err(|error| {
        MemoryError::Serialization(format!("failed to read Memgraph column {key}: {error}"))
    })
}

fn require_row(row: Option<Row>, entity: &str) -> MemoryResult<Row> {
    row.ok_or_else(|| MemoryError::NotFound(format!("{entity} was not found")))
}

fn i64_from_u64(value: u64) -> MemoryResult<i64> {
    i64::try_from(value)
        .map_err(|_| MemoryError::Validation("value exceeds Memgraph integer range".to_string()))
}

fn i64_from_usize(value: usize) -> MemoryResult<i64> {
    i64::try_from(value)
        .map_err(|_| MemoryError::Validation("value exceeds Memgraph integer range".to_string()))
}

fn u64_from_i64(value: i64, field: &str) -> MemoryResult<u64> {
    u64::try_from(value).map_err(|_| {
        MemoryError::Serialization(format!("{field} must be non-negative, got {value}"))
    })
}

fn usize_from_i64(value: i64, field: &str) -> MemoryResult<usize> {
    usize::try_from(value).map_err(|_| {
        MemoryError::Serialization(format!("{field} must be non-negative, got {value}"))
    })
}

fn datetime_ms(value: u64) -> String {
    DateTime::<Utc>::from_timestamp_millis(value as i64)
        .or_else(|| DateTime::<Utc>::from_timestamp_millis(0))
        .expect("unix epoch timestamp must be representable")
        .to_rfc3339()
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis().min(u128::from(u64::MAX)) as u64)
        .unwrap_or(0)
}

fn stable_hex_hash(parts: &[&str]) -> String {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for part in parts {
        for byte in part.as_bytes() {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    format!("{hash:016x}")
}

fn map_neo4rs_error(error: impl std::fmt::Display) -> MemoryError {
    let message = error.to_string();
    let lower = message.to_ascii_lowercase();
    if lower.contains("constraint")
        || lower.contains("unique")
        || lower.contains("already exists")
        || lower.contains("conflict")
    {
        MemoryError::Conflict(message)
    } else {
        MemoryError::StoreUnavailable(message)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slot_json_round_trips_all_payload_types() -> MemoryResult<()> {
        let slots = vec![
            Slot::new("symbol", SlotValue::Symbol(" ACT-R ".to_string())),
            Slot::new("text", SlotValue::Text("Mixed Case".to_string())),
            Slot::new("number", SlotValue::Number(42.25)),
            Slot::new("bool", SlotValue::Bool(true)),
        ];

        for slot in slots {
            let parsed = parse_slot_json(&slot_json(&slot))?;

            assert_eq!(parsed, slot);
        }
        Ok(())
    }

    #[test]
    fn production_rule_condition_json_round_trips() -> MemoryResult<()> {
        let conditions = vec![
            BufferCondition::buffer_present(BufferName::Goal),
            BufferCondition::chunk_type(BufferName::Retrieval, ChunkType::from("fact")),
            BufferCondition::chunk_id(BufferName::Task, ChunkId::from("task-1")),
        ];

        let raw = conditions_json(&conditions)?;
        let parsed = parse_conditions_json(&raw)?;

        assert_eq!(parsed, conditions);
        Ok(())
    }

    #[test]
    fn retrieved_chunk_condition_json_round_trips_slots() -> MemoryResult<()> {
        let condition = Some(
            RetrievedChunkCondition::chunk_type(ChunkType::from("fact"))
                .with_slot("topic", SlotValue::Symbol("preference".to_string()))
                .with_slot("confidence", SlotValue::Number(0.75))
                .with_slot("protected", SlotValue::Bool(false)),
        );

        let raw = retrieved_chunk_json(&condition)?.expect("condition should serialize");
        let parsed = parse_retrieved_chunk_json(&raw)?;

        assert_eq!(Some(parsed), condition);
        Ok(())
    }

    #[test]
    fn neo4rs_error_mapping_distinguishes_conflicts_from_connectivity() {
        assert!(matches!(
            map_neo4rs_error("Constraint validation failed"),
            MemoryError::Conflict(_)
        ));
        assert!(matches!(
            map_neo4rs_error("connection refused"),
            MemoryError::StoreUnavailable(_)
        ));
    }
}
