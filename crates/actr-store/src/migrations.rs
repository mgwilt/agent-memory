#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SchemaMigration {
    pub name: &'static str,
    pub cypher: &'static str,
}

pub fn embedded_migrations() -> Vec<SchemaMigration> {
    vec![SchemaMigration {
        name: "001_actr_memory_schema.cypher",
        cypher: include_str!("../migrations/001_actr_memory_schema.cypher"),
    }]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embeds_initial_schema() {
        let migrations = embedded_migrations();

        assert_eq!(migrations.len(), 1);
        assert!(migrations[0].cypher.contains("CREATE CONSTRAINT"));
        assert!(migrations[0].cypher.contains("CREATE INDEX"));
    }
}
