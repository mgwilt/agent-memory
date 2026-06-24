use std::time::{SystemTime, UNIX_EPOCH};

use neo4rs::{ConfigBuilder, Graph, query};
use nestor_core::{AgentId, Chunk, ChunkId, ChunkType, MemoryError, Slot, SlotValue};
use nestor_rules::{BufferCondition, ProductionRule, RetrievedChunkCondition, RuleId};
use nestor_session::BufferName;
use nestor_store::{
    AssociationWrite, BufferSetCurrent, CandidateQuery, ConsolidateRequest, CreateChunk,
    ForgetRequest, MemgraphRepository, MemgraphRepositoryConfig, MemoryRepository,
    PracticeEventWrite, ProductionRuleRecord, RetrievalPracticeWrite, UpdateChunk,
};

type TestResult<T> = Result<T, Box<dyn std::error::Error>>;

#[tokio::test]
async fn live_memgraph_repository_persists_core_lifecycle_across_reconnect() -> TestResult<()> {
    if !live_memgraph_enabled() {
        return Ok(());
    }

    let config = live_config();
    let test_id = format!("repo-live-{}", now_ms());
    let agent_id = AgentId::from(format!("agent-{test_id}"));
    let context_id = ChunkId::from(format!("ctx-{test_id}"));
    let fact_id = ChunkId::from(format!("fact-{test_id}"));
    cleanup_agent(&config, agent_id.as_str()).await?;

    let repo = MemgraphRepository::connect(config.clone()).await?;
    repo.health_check().await?;

    repo.create_chunk(CreateChunk {
        chunk: Chunk::new(
            agent_id.clone(),
            context_id.clone(),
            ChunkType::from("goal"),
            1_000,
        )
        .with_slot("topic", SlotValue::Symbol("memory".to_string())),
        initial_practice_event_id: format!("encode-{test_id}-context"),
    })
    .await?;
    let fact = repo
        .create_chunk(CreateChunk {
            chunk: Chunk::new(
                agent_id.clone(),
                fact_id.clone(),
                ChunkType::from("fact"),
                1_000,
            )
            .with_slot("topic", SlotValue::Symbol(" ACT-R ".to_string()))
            .with_slot("detail", SlotValue::Text("Mixed Case Payload".to_string()))
            .with_slot("confidence", SlotValue::Number(0.75))
            .with_slot("protected", SlotValue::Bool(false)),
            initial_practice_event_id: format!("encode-{test_id}-fact"),
        })
        .await?;

    assert!(matches!(
        repo.create_chunk(CreateChunk {
            chunk: fact.clone(),
            initial_practice_event_id: format!("duplicate-{test_id}"),
        })
        .await,
        Err(MemoryError::Conflict(_))
    ));

    let fetched = repo
        .get_chunk(&agent_id, &fact_id)
        .await?
        .expect("fact should exist");
    assert_eq!(
        fetched.slot("topic"),
        Some(&SlotValue::Symbol(" ACT-R ".to_string()))
    );
    assert_eq!(
        fetched.slot("detail"),
        Some(&SlotValue::Text("Mixed Case Payload".to_string()))
    );
    assert_eq!(fetched.slot("confidence"), Some(&SlotValue::Number(0.75)));
    assert_eq!(fetched.slot("protected"), Some(&SlotValue::Bool(false)));

    repo.update_chunk(UpdateChunk {
        agent_id: agent_id.clone(),
        chunk_id: fact_id.clone(),
        expected_version: 1,
        slots: vec![
            Slot::new("topic", SlotValue::Symbol(" ACT-R ".to_string())),
            Slot::new("detail", SlotValue::Text("Mixed Case Payload".to_string())),
            Slot::new("confidence", SlotValue::Number(0.75)),
            Slot::new("verified", SlotValue::Bool(true)),
        ],
    })
    .await?;

    assert!(matches!(
        repo.update_chunk(UpdateChunk {
            agent_id: agent_id.clone(),
            chunk_id: fact_id.clone(),
            expected_version: 99,
            slots: Vec::new(),
        })
        .await,
        Err(MemoryError::Conflict(_))
    ));

    repo.append_practice_event(PracticeEventWrite {
        event_id: format!("rehearse-{test_id}"),
        agent_id: agent_id.clone(),
        chunk_id: fact_id.clone(),
        occurred_at_ms: 1_500,
        kind: "rehearse".to_string(),
        weight: 2.0,
    })
    .await?;
    repo.upsert_association(AssociationWrite {
        agent_id: agent_id.clone(),
        src_chunk_id: context_id.clone(),
        dst_chunk_id: fact_id.clone(),
        source: "goal".to_string(),
        strength: 1.5,
        fan: 1,
        updated_at_ms: 2_000,
    })
    .await?;
    repo.set_buffer_current(BufferSetCurrent {
        agent_id: agent_id.clone(),
        buffer_name: BufferName::Goal,
        chunk_id: context_id.clone(),
        set_at_ms: 2_500,
    })
    .await?;

    let candidates = repo
        .fetch_candidates(CandidateQuery {
            agent_id: agent_id.clone(),
            chunk_type: Some("fact".to_string()),
            cue_slots: vec![Slot::new("topic", SlotValue::Symbol("act-r".to_string()))],
            context_chunk_ids: vec![context_id.clone()],
            candidate_limit: 10,
        })
        .await?;
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].chunk.chunk_id, fact_id);
    assert_eq!(candidates[0].spread_score, 1.5);
    assert_eq!(candidates[0].practice_events.len(), 2);
    assert!(candidates[0].base_level_cache_stale);

    repo.record_successful_retrieval(RetrievalPracticeWrite {
        event_id: format!("retrieve-{test_id}"),
        agent_id: agent_id.clone(),
        chunk_id: fact_id.clone(),
        occurred_at_ms: 3_000,
        weight: 1.0,
    })
    .await?;

    let rule = ProductionRuleRecord {
        agent_id: agent_id.clone(),
        rule: ProductionRule::new(
            RuleId::from(format!("rule-{test_id}")),
            "answer retrieved fact",
            vec![BufferCondition::chunk_id(
                BufferName::Goal,
                context_id.clone(),
            )],
        )
        .with_utility(2.5)
        .with_retrieved_chunk(
            RetrievedChunkCondition::chunk_type(ChunkType::from("fact"))
                .with_slot("topic", SlotValue::Symbol("act-r".to_string())),
        ),
        success_count: 2,
        failure_count: 1,
        avg_reward: 0.5,
    };
    repo.upsert_production_rule(rule.clone()).await?;

    let reconnected = MemgraphRepository::connect(config.clone()).await?;
    let persisted = reconnected
        .get_chunk(&agent_id, &fact_id)
        .await?
        .expect("fact should survive repository reconnect");
    assert_eq!(persisted.retrieval_count, 1);
    assert_eq!(persisted.slot("verified"), Some(&SlotValue::Bool(true)));

    let persisted_candidates = reconnected
        .fetch_candidates(CandidateQuery {
            agent_id: agent_id.clone(),
            chunk_type: Some("fact".to_string()),
            cue_slots: vec![Slot::new("topic", SlotValue::Symbol("act-r".to_string()))],
            context_chunk_ids: vec![context_id],
            candidate_limit: 10,
        })
        .await?;
    assert_eq!(persisted_candidates[0].practice_events.len(), 3);

    let persisted_rule = reconnected
        .get_production_rule(&agent_id, &rule.rule.rule_id)
        .await?;
    assert_eq!(persisted_rule, Some(rule));

    reconnected.soft_delete_chunk(&agent_id, &fact_id).await?;
    assert!(reconnected.get_chunk(&agent_id, &fact_id).await?.is_none());

    cleanup_agent(&config, agent_id.as_str()).await?;
    Ok(())
}

#[tokio::test]
async fn live_memgraph_repository_consolidates_and_forgets_lifecycle_state() -> TestResult<()> {
    if !live_memgraph_enabled() {
        return Ok(());
    }

    let config = live_config();
    let test_id = format!("repo-lifecycle-{}", now_ms());
    let agent_id = AgentId::from(format!("agent-{test_id}"));
    cleanup_agent(&config, agent_id.as_str()).await?;

    let repo = MemgraphRepository::connect(config.clone()).await?;
    for suffix in ["a", "b"] {
        let chunk_id = format!("episode-{suffix}-{test_id}");
        repo.create_chunk(CreateChunk {
            chunk: Chunk::new(
                agent_id.clone(),
                ChunkId::from(chunk_id.clone()),
                ChunkType::from("episode"),
                1_000,
            )
            .with_slot("topic", SlotValue::Symbol("preference".to_string()))
            .with_slot("subject", SlotValue::Symbol("eli".to_string())),
            initial_practice_event_id: format!("encode-{chunk_id}"),
        })
        .await?;
    }

    let consolidation = repo
        .consolidate(ConsolidateRequest {
            agent_id: agent_id.clone(),
            chunk_type: Some(ChunkType::from("episode")),
            summary_chunk_type: ChunkType::from("semantic"),
            group_slot_keys: vec!["topic".to_string(), "subject".to_string()],
            min_group_size: 2,
            now_ms: 5_000,
        })
        .await?;
    assert_eq!(consolidation.groups_consolidated, 1);
    assert_eq!(consolidation.summaries[0].source_chunk_ids.len(), 2);
    let summary = repo
        .get_chunk(&agent_id, &consolidation.summaries[0].summary_chunk_id)
        .await?
        .expect("summary should exist");
    assert_eq!(summary.chunk_type, ChunkType::from("semantic"));
    assert_eq!(summary.slot("source_count"), Some(&SlotValue::Number(2.0)));
    let source = repo
        .get_chunk(&agent_id, &consolidation.summaries[0].source_chunk_ids[0])
        .await?
        .expect("source should remain active");
    assert_eq!(source.slot("consolidated"), Some(&SlotValue::Bool(true)));

    let old_id = ChunkId::from(format!("old-{test_id}"));
    let protected_id = ChunkId::from(format!("protected-{test_id}"));
    let linked_id = ChunkId::from(format!("linked-{test_id}"));
    let linker_id = ChunkId::from(format!("linker-{test_id}"));
    for (chunk_id, protected) in [
        (old_id.clone(), false),
        (protected_id.clone(), true),
        (linked_id.clone(), false),
        (linker_id.clone(), false),
    ] {
        let mut chunk = Chunk::new(
            agent_id.clone(),
            chunk_id.clone(),
            ChunkType::from("stale"),
            100,
        )
        .with_slot("topic", SlotValue::Symbol("old".to_string()));
        if protected {
            chunk.upsert_slot("protected", SlotValue::Bool(true));
        }
        repo.create_chunk(CreateChunk {
            chunk,
            initial_practice_event_id: format!("encode-{}", chunk_id.as_str()),
        })
        .await?;
    }
    repo.upsert_association(AssociationWrite {
        agent_id: agent_id.clone(),
        src_chunk_id: linker_id,
        dst_chunk_id: linked_id.clone(),
        source: "test".to_string(),
        strength: 1.0,
        fan: 1,
        updated_at_ms: 150,
    })
    .await?;

    let forget = repo
        .forget(ForgetRequest {
            agent_id: agent_id.clone(),
            chunk_type: Some(ChunkType::from("stale")),
            now_ms: 1_000_000,
            recency_cutoff_ms: 500,
            base_level_cutoff: 0.0,
            allow_linked_forget: false,
        })
        .await?;

    assert!(forget.forgotten_chunk_ids.contains(&old_id));
    assert!(forget.protected_chunk_ids.contains(&protected_id));
    assert!(forget.archived_chunk_ids.contains(&linked_id));
    assert!(repo.get_chunk(&agent_id, &old_id).await?.is_none());
    assert!(repo.get_chunk(&agent_id, &protected_id).await?.is_some());
    let linked = repo
        .get_chunk(&agent_id, &linked_id)
        .await?
        .expect("linked chunk should be archived but active");
    assert_eq!(linked.slot("archived"), Some(&SlotValue::Bool(true)));

    cleanup_agent(&config, agent_id.as_str()).await?;
    Ok(())
}

fn live_memgraph_enabled() -> bool {
    std::env::var("NESTOR_STORE_MEMGRAPH_TESTS").as_deref() == Ok("1")
}

fn live_config() -> MemgraphRepositoryConfig {
    MemgraphRepositoryConfig {
        uri: std::env::var("NESTOR_MEMGRAPH_URI")
            .unwrap_or_else(|_| "bolt://127.0.0.1:7687".to_string()),
        user: std::env::var("NESTOR_MEMGRAPH_USER").unwrap_or_else(|_| "memgraph".to_string()),
        password: std::env::var("NESTOR_MEMGRAPH_PASSWORD").unwrap_or_default(),
        database: "memgraph".to_string(),
        max_connections: std::env::var("NESTOR_MEMGRAPH_MAX_CONNECTIONS")
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(4),
        tls_ca_file: std::env::var("NESTOR_MEMGRAPH_TLS_CA_FILE").ok(),
        schema_info_enabled: true,
    }
}

async fn cleanup_agent(config: &MemgraphRepositoryConfig, agent_id: &str) -> TestResult<()> {
    let graph = Graph::connect(
        ConfigBuilder::default()
            .uri(config.uri.clone())
            .user(config.user.clone())
            .password(config.password.clone())
            .db(config.database.clone())
            .max_connections(config.max_connections)
            .build()?,
    )
    .await?;
    let mut result = graph
        .execute(
            query(
                "
                MATCH (n)
                WHERE n.agent_id = $agent_id OR n.tenant_id = $agent_id
                DETACH DELETE n
                ",
            )
            .param("agent_id", agent_id),
        )
        .await?;
    while result.next().await?.is_some() {}
    Ok(())
}

fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_millis())
}
