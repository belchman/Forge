use chrono::Utc;
use rusqlite::{params, OptionalExtension};

use flowforge_core::{Result, RoutingWeight};

use super::{parse_datetime, MemoryDb, SqliteExt};

impl MemoryDb {
    pub fn get_routing_weight(
        &self,
        task_pattern: &str,
        agent_name: &str,
    ) -> Result<Option<RoutingWeight>> {
        self.conn
            .query_row(
                "SELECT task_pattern, agent_name, weight, successes, failures, updated_at
                 FROM routing_weights WHERE task_pattern = ?1 AND agent_name = ?2",
                params![task_pattern, agent_name],
                |row| {
                    Ok(RoutingWeight {
                        task_pattern: row.get(0)?,
                        agent_name: row.get(1)?,
                        weight: row.get(2)?,
                        successes: row.get(3)?,
                        failures: row.get(4)?,
                        updated_at: parse_datetime(row.get::<_, String>(5)?),
                    })
                },
            )
            .optional()
            .sq()
    }

    pub fn get_all_routing_weights(&self) -> Result<Vec<RoutingWeight>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT task_pattern, agent_name, weight, successes, failures, updated_at
                 FROM routing_weights ORDER BY weight DESC",
            )
            .sq()?;
        let rows = stmt
            .query_map([], |row| {
                Ok(RoutingWeight {
                    task_pattern: row.get(0)?,
                    agent_name: row.get(1)?,
                    weight: row.get(2)?,
                    successes: row.get(3)?,
                    failures: row.get(4)?,
                    updated_at: parse_datetime(row.get::<_, String>(5)?),
                })
            })
            .sq()?;
        rows.collect::<std::result::Result<Vec<_>, _>>().sq()
    }

    pub fn record_routing_success(&self, task_pattern: &str, agent_name: &str) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        self.conn
            .execute(
                "INSERT INTO routing_weights (task_pattern, agent_name, weight, successes, failures, updated_at)
                 VALUES (?1, ?2, 0.6, 1, 0, ?3)
                 ON CONFLICT(task_pattern, agent_name) DO UPDATE SET
                   successes = successes + 1,
                   weight = MIN(1.0, weight + 0.05),
                   updated_at = ?3",
                params![task_pattern, agent_name, now],
            )
            .sq()?;
        Ok(())
    }

    pub fn record_routing_failure(&self, task_pattern: &str, agent_name: &str) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        self.conn
            .execute(
                "INSERT INTO routing_weights (task_pattern, agent_name, weight, successes, failures, updated_at)
                 VALUES (?1, ?2, 0.4, 0, 1, ?3)
                 ON CONFLICT(task_pattern, agent_name) DO UPDATE SET
                   failures = failures + 1,
                   weight = MAX(0.0, weight - 0.05),
                   updated_at = ?3",
                params![task_pattern, agent_name, now],
            )
            .sq()?;
        Ok(())
    }

    pub fn count_routing_weights(&self) -> Result<u64> {
        self.conn
            .query_row("SELECT COUNT(*) FROM routing_weights", [], |row| row.get(0))
            .sq()
    }
}
