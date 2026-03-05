mod dedup;
mod lifecycle;
mod search;
#[cfg(test)]
mod tests;

use std::cell::RefCell;
use std::collections::HashMap;

use chrono::Utc;
use uuid::Uuid;

use flowforge_core::config::PatternsConfig;
use flowforge_core::{PatternMatch, PatternTier, Result, ShortTermPattern};

use crate::db::MemoryDb;
use crate::embedding::{default_embedder, Embedder};
use crate::hnsw::HnswIndex;

/// Cached HNSW index with the vector count it was built from.
/// Opaque to external consumers — use via `HnswCache` type alias.
pub struct CachedIndex {
    pub(crate) index: HnswIndex,
    pub(crate) id_to_source: HashMap<i64, (String, PatternTier)>,
    pub(crate) built_from_count: usize,
}

/// Externalized HNSW cache that survives across `PatternStore` instances.
/// Store this on a long-lived struct (e.g. `ToolRegistry`) and pass it
/// to `PatternStore::with_cache()` to avoid rebuilding on every call.
pub type HnswCache = RefCell<Option<CachedIndex>>;

/// Create a new, empty HNSW cache.
pub fn new_hnsw_cache() -> HnswCache {
    RefCell::new(None)
}

/// Manages pattern learning lifecycle: store, promote, consolidate, search.
pub struct PatternStore<'a> {
    db: &'a MemoryDb,
    config: &'a PatternsConfig,
    embedding: Box<dyn Embedder>,
    /// Lazily-built HNSW index cached for the lifetime of this PatternStore.
    hnsw_cache: RefCell<Option<CachedIndex>>,
    /// Optional external cache that takes priority over the owned one.
    external_cache: Option<&'a HnswCache>,
}

impl<'a> PatternStore<'a> {
    pub fn new(db: &'a MemoryDb, config: &'a PatternsConfig) -> Self {
        Self {
            db,
            config,
            embedding: default_embedder(config),
            hnsw_cache: RefCell::new(None),
            external_cache: None,
        }
    }

    /// Create a PatternStore that uses an externalized HNSW cache.
    /// The cache persists across multiple PatternStore instances,
    /// avoiding costly HNSW index rebuilds on every MCP tool call.
    pub fn with_cache(db: &'a MemoryDb, config: &'a PatternsConfig, cache: &'a HnswCache) -> Self {
        Self {
            db,
            config,
            embedding: default_embedder(config),
            hnsw_cache: RefCell::new(None),
            external_cache: Some(cache),
        }
    }

    /// Returns the active cache — external if available, otherwise owned.
    pub(crate) fn cache(&self) -> &RefCell<Option<CachedIndex>> {
        self.external_cache.unwrap_or(&self.hnsw_cache)
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

    /// Search all patterns (both tiers) using HNSW when >50 total, else brute-force.
    /// Boosts results from the same cluster as the query.
    pub fn search_all_patterns(&self, query: &str, k: usize) -> Result<Vec<PatternMatch>> {
        let query_vec = self.embedding.embed(query);
        let total =
            self.db.count_patterns_short()? as usize + self.db.count_patterns_long()? as usize;

        let mut results = if total > 50 {
            self.search_with_hnsw(&query_vec, k)?
        } else {
            self.search_brute_force(&query_vec, k)?
        };

        // Boost results from the same cluster as the query
        let cluster_mgr = crate::clustering::ClusterManager::new(self.db, self.config);
        if let Ok(Some(query_cluster)) = cluster_mgr.find_cluster(&query_vec) {
            // Look up embedding IDs for each result to check cluster membership
            let all_short = self.db.get_vectors_for_source("pattern_short")?;
            let all_long = self.db.get_vectors_for_source("pattern_long")?;
            let mut source_to_eid: std::collections::HashMap<String, i64> =
                std::collections::HashMap::new();
            for (db_id, source_id, _) in all_short.iter().chain(all_long.iter()) {
                source_to_eid.insert(source_id.clone(), *db_id);
            }

            for result in &mut results {
                if let Some(&eid) = source_to_eid.get(&result.id) {
                    if let Ok(Some(cid)) = self.db.get_vector_cluster_id(eid) {
                        if cid == query_cluster.cluster_id {
                            result.similarity *= 1.1; // 10% boost for same-cluster
                        }
                    }
                }
            }
            results.sort_by(|a, b| {
                b.similarity
                    .partial_cmp(&a.similarity)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
        }

        Ok(results)
    }

    /// Record usage of a pattern (increments count and confidence).
    pub fn record_usage(&self, pattern_id: &str) -> Result<()> {
        self.db.update_pattern_short_usage(pattern_id)
    }

    /// Record feedback on a pattern (success/failure). (A4)
    /// Looks up in both short-term and long-term stores.
    pub fn record_feedback(&self, pattern_id: &str, success: bool) -> Result<()> {
        // Try long-term first (feedback is most meaningful for promoted patterns)
        if self.db.get_pattern_long(pattern_id)?.is_some() {
            self.db.update_pattern_long_feedback(pattern_id, success)?;
            return Ok(());
        }

        // Fall back to short-term: adjust confidence
        if let Some(p) = self.db.get_pattern_short(pattern_id)? {
            let new_confidence = if success {
                (p.confidence + 0.05).min(1.0)
            } else {
                (p.confidence - 0.08).max(0.0)
            };
            self.db
                .update_pattern_short_confidence(pattern_id, new_confidence)?;
            return Ok(());
        }

        Ok(()) // Pattern not found, silently ignore
    }
}
