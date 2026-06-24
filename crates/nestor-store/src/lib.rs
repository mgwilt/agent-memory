#![forbid(unsafe_code)]

pub mod cypher;
pub mod memgraph;
pub mod memgraph_repository;
pub mod migrations;
pub mod repository;
pub mod retrieval;

pub use memgraph::{
    MemgraphDriverPlan, MemgraphRepositoryConfig, MigrationApplyReport, SchemaExecutor,
    apply_embedded_migrations, apply_migrations, recommended_driver_plan,
};
pub use memgraph_repository::MemgraphRepository;
pub use migrations::{
    MigrationStatement, SchemaMigration, embedded_migration_statements, embedded_migrations,
    is_already_applied_schema_error, migration_statements, validate_migrations,
};
pub use repository::{
    AssociationWrite, BufferSetCurrent, CandidateQuery, ChunkWithHistory, ConsolidateRequest,
    ConsolidationGroupReport, ConsolidationReport, CreateChunk, DEFAULT_CANDIDATE_LIMIT,
    ForgetReport, ForgetRequest, MAX_CANDIDATE_LIMIT, MemoryRepository, PracticeEventWrite,
    ProductionRuleRecord, RetrievalPracticeWrite, StoredSlot, UpdateChunk, bounded_candidate_limit,
    chunk_slot_hash, slot_value_hash, stored_slots_from_chunk, stored_slots_from_slots,
};
pub use retrieval::{
    MismatchPolicy, RankedRetrievalCandidate, RetrievalDiagnostics, RetrievalHit, RetrievalMiss,
    RetrievalMissReason, RetrievalOutcome, RetrievalRequest, RetrievalScoreBreakdown,
    RetrievalStatus, retrieve_chunk, retrieve_chunk_outcome,
};
