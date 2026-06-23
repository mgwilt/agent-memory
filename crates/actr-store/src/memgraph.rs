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
