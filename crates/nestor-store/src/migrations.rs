use nestor_core::{MemoryError, MemoryResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SchemaMigration {
    pub name: &'static str,
    pub cypher: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MigrationStatement {
    pub migration_name: &'static str,
    pub ordinal: usize,
    pub cypher: String,
}

pub fn embedded_migrations() -> Vec<SchemaMigration> {
    vec![SchemaMigration {
        name: "001_nestor_memory_schema.cypher",
        cypher: include_str!("../migrations/001_nestor_memory_schema.cypher"),
    }]
}

pub fn validate_migrations(migrations: &[SchemaMigration]) -> MemoryResult<()> {
    if migrations.is_empty() {
        return Err(MemoryError::Validation(
            "at least one schema migration is required".to_string(),
        ));
    }

    let mut previous_version = 0_u64;
    for migration in migrations {
        let version = migration_version(migration.name)?;
        if version <= previous_version {
            return Err(MemoryError::Validation(format!(
                "schema migration {} is out of order",
                migration.name
            )));
        }
        if migration_statements(migration)?.is_empty() {
            return Err(MemoryError::Validation(format!(
                "schema migration {} has no statements",
                migration.name
            )));
        }
        previous_version = version;
    }

    Ok(())
}

pub fn migration_statements(migration: &SchemaMigration) -> MemoryResult<Vec<MigrationStatement>> {
    let mut statements = Vec::new();
    let mut current = String::new();

    for line in migration.cypher.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with("--") {
            continue;
        }

        if trimmed.ends_with(';') {
            let statement_line = trimmed.trim_end_matches(';').trim_end();
            if !statement_line.is_empty() {
                current.push_str(statement_line);
            }
            let cypher = current.trim().to_string();
            if !cypher.is_empty() {
                statements.push(MigrationStatement {
                    migration_name: migration.name,
                    ordinal: statements.len() + 1,
                    cypher,
                });
            }
            current.clear();
        } else {
            current.push_str(trimmed);
            current.push('\n');
        }
    }

    if !current.trim().is_empty() {
        return Err(MemoryError::Validation(format!(
            "schema migration {} has an unterminated statement",
            migration.name
        )));
    }

    Ok(statements)
}

pub fn embedded_migration_statements() -> MemoryResult<Vec<MigrationStatement>> {
    let migrations = embedded_migrations();
    validate_migrations(&migrations)?;
    migrations
        .iter()
        .map(migration_statements)
        .try_fold(Vec::new(), |mut all, statements| {
            all.extend(statements?);
            Ok(all)
        })
}

pub fn is_already_applied_schema_error(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    lower.contains("already exists")
        || lower.contains("already defined")
        || lower.contains("constraint violation")
}

fn migration_version(name: &str) -> MemoryResult<u64> {
    let prefix = name
        .split_once('_')
        .map(|(prefix, _)| prefix)
        .unwrap_or(name);
    prefix.parse::<u64>().map_err(|_| {
        MemoryError::Validation(format!(
            "schema migration {name} must start with a numeric version"
        ))
    })
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

    #[test]
    fn migrations_are_ordered_and_split_into_statements() -> MemoryResult<()> {
        let migrations = embedded_migrations();
        validate_migrations(&migrations)?;
        let statements = embedded_migration_statements()?;

        assert!(statements.len() > 10);
        assert_eq!(
            statements[0].migration_name,
            "001_nestor_memory_schema.cypher"
        );
        assert_eq!(statements[0].ordinal, 1);
        assert!(
            statements
                .iter()
                .all(|statement| !statement.cypher.ends_with(';'))
        );
        Ok(())
    }

    #[test]
    fn migration_validation_rejects_out_of_order_files() {
        let migrations = [
            SchemaMigration {
                name: "002_second.cypher",
                cypher: "RETURN 2;",
            },
            SchemaMigration {
                name: "001_first.cypher",
                cypher: "RETURN 1;",
            },
        ];

        assert!(matches!(
            validate_migrations(&migrations),
            Err(MemoryError::Validation(_))
        ));
    }

    #[test]
    fn migration_parser_rejects_unterminated_statements() {
        let migration = SchemaMigration {
            name: "001_bad.cypher",
            cypher: "CREATE INDEX ON :Chunk(chunk_id)",
        };

        assert!(matches!(
            migration_statements(&migration),
            Err(MemoryError::Validation(_))
        ));
    }

    #[test]
    fn already_applied_schema_errors_are_identified() {
        assert!(is_already_applied_schema_error(
            "Constraint already exists: ASSERT c.chunk_id IS UNIQUE"
        ));
        assert!(is_already_applied_schema_error(
            "Index already defined for label Chunk"
        ));
        assert!(!is_already_applied_schema_error("Syntax error near CREATE"));
    }
}
