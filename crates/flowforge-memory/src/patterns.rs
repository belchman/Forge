use chrono::Utc;
use uuid::Uuid;

use flowforge_core::config::PatternsConfig;
use flowforge_core::{LongTermPattern, Result, ShortTermPattern};

use crate::db::MemoryDb;
use crate::embedding::Embedding;

/// Manages pattern learning lifecycle: store, promote, consolidate, search.
pub struct PatternStore<'a> {
    db: &'a MemoryDb,
    config: &'a PatternsConfig,
    embedding: Embedding,
}

impl<'a> PatternStore<'a> {
    pub fn new(db: &'a MemoryDb, config: &'a PatternsConfig) -> Self {
        Self {
            db,
            config,
            embedding: Embedding::default(),
        }
    }

    /// Store a new short-term pattern. Returns the pattern ID.
    pub fn store_short_term(&self, content: &str, category: &str) -> Result<String> {
        let now = Utc::now();
        let id = Uuid::new_v4().to_string();

        // Generate and store embedding
        let vector = self.embedding.embed(content);
        let embedding_id = self.db.store_vector("pattern_short", &id, &vector)?;

        let pattern = ShortTermPattern {
            id: id.clone(),
            content: content.to_string(),
            category: category.to_string(),
            confidence: 0.5,
            usage_count: 0,
            created_at: now,
            last_used: now,
            embedding_id: Some(embedding_id),
        };

        self.db.store_pattern_short(&pattern)?;

        // Enforce max count: remove oldest if over limit
        let count = self.db.count_patterns_short()? as usize;
        if count > self.config.short_term_max {
            self.prune_oldest_short(count - self.config.short_term_max)?;
        }

        Ok(id)
    }

    /// Promote eligible short-term patterns to long-term.
    /// Criteria: usage >= min_usage AND confidence >= min_confidence
    pub fn promote_eligible(&self) -> Result<u32> {
        let patterns = self.db.get_all_patterns_short()?;
        let mut promoted = 0;

        for p in &patterns {
            if p.usage_count >= self.config.promotion_min_usage
                && p.confidence >= self.config.promotion_min_confidence
            {
                let now = Utc::now();
                let long_pattern = LongTermPattern {
                    id: p.id.clone(),
                    content: p.content.clone(),
                    category: p.category.clone(),
                    confidence: p.confidence,
                    usage_count: p.usage_count,
                    success_count: 0,
                    failure_count: 0,
                    created_at: p.created_at,
                    promoted_at: now,
                    last_used: p.last_used,
                    embedding_id: p.embedding_id,
                };

                self.db.store_pattern_long(&long_pattern)?;
                self.db.delete_pattern_short(&p.id)?;
                promoted += 1;
            }
        }

        Ok(promoted)
    }

    /// Run consolidation: deduplication, decay, and pruning.
    pub fn consolidate(&self) -> Result<()> {
        // 1. Promote eligible patterns
        self.promote_eligible()?;

        // 2. Expire old short-term patterns (TTL-based)
        self.expire_short_term()?;

        // 3. Deduplicate similar patterns (using embedding similarity)
        self.deduplicate()?;

        Ok(())
    }

    /// Search patterns using text similarity.
    pub fn search_patterns(&self, query: &str, k: usize) -> Result<Vec<(ShortTermPattern, f32)>> {
        let query_vec = self.embedding.embed(query);
        let all_patterns = self.db.get_all_patterns_short()?;

        let mut scored: Vec<(ShortTermPattern, f32)> = all_patterns
            .into_iter()
            .map(|p| {
                let p_vec = self.embedding.embed(&p.content);
                let sim = Embedding::cosine_similarity(&query_vec, &p_vec);
                (p, sim)
            })
            .collect();

        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(k);

        Ok(scored)
    }

    /// Record usage of a pattern (increments count and confidence).
    pub fn record_usage(&self, pattern_id: &str) -> Result<()> {
        self.db.update_pattern_short_usage(pattern_id)
    }

    /// Record feedback on a pattern (success/failure).
    pub fn record_feedback(&self, _pattern_id: &str, _success: bool) -> Result<()> {
        // Long-term pattern feedback would update success_count/failure_count
        // For now this is a placeholder that will be fully connected in the MCP tools
        Ok(())
    }

    fn expire_short_term(&self) -> Result<()> {
        let patterns = self.db.get_all_patterns_short()?;
        let now = Utc::now();
        let ttl = chrono::Duration::hours(self.config.short_term_ttl_hours as i64);

        for p in &patterns {
            if now - p.created_at > ttl && p.confidence < self.config.promotion_min_confidence {
                self.db.delete_pattern_short(&p.id)?;
                self.db.delete_vectors_for_source("pattern_short", &p.id)?;
            }
        }

        Ok(())
    }

    fn deduplicate(&self) -> Result<()> {
        let patterns = self.db.get_all_patterns_short()?;

        let mut to_remove = Vec::new();

        for i in 0..patterns.len() {
            if to_remove.contains(&i) {
                continue;
            }
            let vec_i = self.embedding.embed(&patterns[i].content);
            for j in (i + 1)..patterns.len() {
                if to_remove.contains(&j) {
                    continue;
                }
                let vec_j = self.embedding.embed(&patterns[j].content);
                let sim = Embedding::cosine_similarity(&vec_i, &vec_j);
                if sim > self.config.dedup_similarity_threshold as f32 {
                    // Keep the one with higher confidence/usage
                    if patterns[j].confidence < patterns[i].confidence
                        || (patterns[j].confidence == patterns[i].confidence
                            && patterns[j].usage_count < patterns[i].usage_count)
                    {
                        to_remove.push(j);
                    } else {
                        to_remove.push(i);
                        break;
                    }
                }
            }
        }

        for idx in to_remove {
            let p = &patterns[idx];
            self.db.delete_pattern_short(&p.id)?;
            self.db.delete_vectors_for_source("pattern_short", &p.id)?;
        }

        Ok(())
    }

    fn prune_oldest_short(&self, count: usize) -> Result<()> {
        let patterns = self.db.get_all_patterns_short()?;
        // Patterns are returned sorted by last_used DESC, so take from the end
        let to_prune = patterns.iter().rev().take(count);
        for p in to_prune {
            self.db.delete_pattern_short(&p.id)?;
            self.db.delete_vectors_for_source("pattern_short", &p.id)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn setup_db() -> MemoryDb {
        let path = PathBuf::from(format!("/tmp/flowforge-test-patterns-{}.db", Uuid::new_v4()));
        MemoryDb::open(&path).unwrap()
    }

    #[test]
    fn test_store_and_search() {
        let db = setup_db();
        let config = PatternsConfig::default();
        let store = PatternStore::new(&db, &config);

        store.store_short_term("use cargo build for compilation", "rust").unwrap();
        store.store_short_term("python uses pip for packages", "python").unwrap();

        let results = store.search_patterns("cargo rust", 2).unwrap();
        assert!(!results.is_empty());
        assert!(results[0].0.content.contains("cargo"));
    }

    #[test]
    fn test_promote_eligible() {
        let db = setup_db();
        let config = PatternsConfig {
            promotion_min_usage: 2,
            promotion_min_confidence: 0.5,
            ..PatternsConfig::default()
        };
        let store = PatternStore::new(&db, &config);

        let id = store.store_short_term("test pattern", "test").unwrap();
        // Bump usage to meet promotion threshold
        store.record_usage(&id).unwrap();
        store.record_usage(&id).unwrap();

        let promoted = store.promote_eligible().unwrap();
        assert_eq!(promoted, 1);

        // Should be gone from short-term
        assert_eq!(db.count_patterns_short().unwrap(), 0);
        // Should be in long-term
        assert_eq!(db.count_patterns_long().unwrap(), 1);
    }
}
