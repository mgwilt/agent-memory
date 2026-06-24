use criterion::{BatchSize, BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use nestor_core::{
    ActivationInput, ActivationParams, AgentId, Chunk, ChunkId, ChunkType, MemoryError,
    MemoryResult, PartialMatchingParams, PracticeEvent, Slot, SlotValue, score_activation,
};
use nestor_rules::RuleId;
use nestor_session::SessionState;
use nestor_store::{
    AssociationWrite, BufferSetCurrent, CandidateQuery, ChunkWithHistory, ConsolidateRequest,
    ConsolidationReport, CreateChunk, ForgetReport, ForgetRequest, MemoryRepository,
    MismatchPolicy, PracticeEventWrite, ProductionRuleRecord, RetrievalPracticeWrite,
    RetrievalRequest, UpdateChunk, retrieve_chunk,
};

const NOW_MS: u64 = 120_000;

#[derive(Debug, Clone)]
struct BenchRepository {
    candidates: Vec<ChunkWithHistory>,
}

impl BenchRepository {
    fn new(candidate_count: usize) -> Self {
        let candidates = (0..candidate_count)
            .map(|index| ChunkWithHistory {
                chunk: candidate_chunk(index),
                practice_events: bounded_practice_history(index, 8),
                spread_score: (index % 7) as f64 * 0.05,
                base_level_cache_stale: false,
            })
            .collect();

        Self { candidates }
    }
}

#[async_trait::async_trait]
impl MemoryRepository for BenchRepository {
    async fn create_chunk(&self, _req: CreateChunk) -> MemoryResult<Chunk> {
        Err(unsupported_write("create_chunk"))
    }

    async fn upsert_chunk(&self, _req: CreateChunk) -> MemoryResult<Chunk> {
        Err(unsupported_write("upsert_chunk"))
    }

    async fn get_chunk(
        &self,
        _agent_id: &AgentId,
        _chunk_id: &ChunkId,
    ) -> MemoryResult<Option<Chunk>> {
        Ok(None)
    }

    async fn update_chunk(&self, _req: UpdateChunk) -> MemoryResult<Chunk> {
        Err(unsupported_write("update_chunk"))
    }

    async fn soft_delete_chunk(
        &self,
        _agent_id: &AgentId,
        _chunk_id: &ChunkId,
    ) -> MemoryResult<()> {
        Err(unsupported_write("soft_delete_chunk"))
    }

    async fn append_practice_event(&self, _req: PracticeEventWrite) -> MemoryResult<()> {
        Err(unsupported_write("append_practice_event"))
    }

    async fn record_successful_retrieval(&self, _req: RetrievalPracticeWrite) -> MemoryResult<()> {
        Ok(())
    }

    async fn fetch_candidates(&self, req: CandidateQuery) -> MemoryResult<Vec<ChunkWithHistory>> {
        req.validate()?;
        Ok(self
            .candidates
            .iter()
            .filter(|candidate| candidate.chunk.agent_id == req.agent_id)
            .filter(|candidate| {
                req.chunk_type
                    .as_ref()
                    .is_none_or(|chunk_type| candidate.chunk.chunk_type.as_str() == chunk_type)
            })
            .take(req.candidate_limit)
            .cloned()
            .collect())
    }

    async fn upsert_association(&self, _req: AssociationWrite) -> MemoryResult<()> {
        Err(unsupported_write("upsert_association"))
    }

    async fn set_buffer_current(&self, _req: BufferSetCurrent) -> MemoryResult<()> {
        Ok(())
    }

    async fn upsert_production_rule(
        &self,
        _req: ProductionRuleRecord,
    ) -> MemoryResult<ProductionRuleRecord> {
        Err(unsupported_write("upsert_production_rule"))
    }

    async fn get_production_rule(
        &self,
        _agent_id: &AgentId,
        _rule_id: &RuleId,
    ) -> MemoryResult<Option<ProductionRuleRecord>> {
        Ok(None)
    }

    async fn consolidate(&self, req: ConsolidateRequest) -> MemoryResult<ConsolidationReport> {
        req.validate()?;
        Ok(ConsolidationReport {
            agent_id: req.agent_id,
            groups_considered: 0,
            groups_consolidated: 0,
            summaries: Vec::new(),
        })
    }

    async fn forget(&self, req: ForgetRequest) -> MemoryResult<ForgetReport> {
        req.validate()?;
        Ok(ForgetReport {
            agent_id: req.agent_id,
            examined: 0,
            forgotten_chunk_ids: Vec::new(),
            archived_chunk_ids: Vec::new(),
            protected_chunk_ids: Vec::new(),
        })
    }
}

fn unsupported_write(operation: &str) -> MemoryError {
    MemoryError::Validation(format!(
        "{operation} is not supported by benchmark repository"
    ))
}

fn bounded_practice_history(candidate_index: usize, event_count: usize) -> Vec<PracticeEvent> {
    (0..event_count)
        .map(|event_index| PracticeEvent {
            occurred_at_ms: 1_000 + (candidate_index as u64 * 17) + (event_index as u64 * 503),
            weight: 1.0 + (event_index % 3) as f64 * 0.1,
        })
        .collect()
}

fn candidate_chunk(index: usize) -> Chunk {
    Chunk::new(
        AgentId::from("agent-1"),
        ChunkId::from(format!("ck-{index:03}")),
        ChunkType::from("fact"),
        1_000,
    )
    .with_slot("topic", SlotValue::Symbol("act-r".to_string()))
    .with_slot("ordinal", SlotValue::Number(index as f64))
}

fn activation_input(history_size: usize) -> ActivationInput {
    ActivationInput {
        now_ms: NOW_MS,
        practice_events: bounded_practice_history(0, history_size),
        spread_score: 0.35,
        partial_match_score: -0.1,
        noise: 0.0,
        params: ActivationParams::deterministic(),
    }
}

fn retrieval_request(candidate_limit: usize) -> RetrievalRequest {
    let mut request = RetrievalRequest::new(AgentId::from("agent-1"), NOW_MS);
    request.chunk_type = Some("fact".to_string());
    request.candidate_limit = candidate_limit;
    request.activation_params = ActivationParams {
        retrieval_threshold: -50.0,
        noise_s: 0.2,
        ..ActivationParams::deterministic()
    };
    request.cue_slots = vec![
        Slot::new("topic", SlotValue::Symbol(" ACT-R ".to_string())),
        Slot::new("ordinal", SlotValue::Number(25.0)),
    ];
    request.mismatch_policy = MismatchPolicy::Partial {
        params: PartialMatchingParams {
            mismatch_penalty: 0.5,
            missing_slot_penalty: 0.5,
        },
        similarities: Vec::new(),
    };
    request.deterministic_seed = Some(7);
    request.commit_on_hit = false;
    request
}

fn bench_activation(c: &mut Criterion) {
    let mut group = c.benchmark_group("activation");
    for history_size in [8_usize, 32, 128] {
        let input = activation_input(history_size);
        group.bench_with_input(
            BenchmarkId::new("score_activation", history_size),
            &input,
            |b, input| {
                b.iter(|| black_box(score_activation(black_box(input))));
            },
        );
    }
    group.finish();
}

fn bench_retrieval(c: &mut Criterion) {
    let mut group = c.benchmark_group("retrieval");
    group.sample_size(20);
    let runtime = tokio::runtime::Runtime::new().expect("tokio runtime should start");
    for candidate_count in [50_usize, 100, 200] {
        let repository = BenchRepository::new(candidate_count);
        let request = retrieval_request(candidate_count);
        group.bench_with_input(
            BenchmarkId::new("rank_candidates", candidate_count),
            &candidate_count,
            |b, _| {
                b.iter_batched(
                    || SessionState::new(AgentId::from("agent-1")),
                    |mut session| {
                        let outcome = runtime.block_on(retrieve_chunk(
                            &repository,
                            &mut session,
                            request.clone(),
                        ));
                        if let Err(error) = &outcome {
                            panic!("benchmark retrieval failed: {error}");
                        }
                        black_box(outcome)
                    },
                    BatchSize::SmallInput,
                );
            },
        );
    }
    group.finish();
}

criterion_group!(benches, bench_activation, bench_retrieval);
criterion_main!(benches);
