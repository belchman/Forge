use std::cell::RefCell;
use std::collections::HashMap;

use chrono::Utc;
use uuid::Uuid;

use flowforge_core::config::PatternsConfig;
use flowforge_core::{LongTermPattern, Result, ShortTermPattern};

use crate::db::MemoryDb;
use crate::embedding::Embedding;
use crate::hnsw::HnswIndex;

/// Cached HNSW index with the vector count it was built from.
struct CachedIndex {
    index: HnswIndex,
    id_to_source: HashMap<i64, String>,
    built_from_count: usize,
}

/// Manages pattern learning lifecycle: store, promote, consolidate, search.
pub struct PatternStore<'a> {
    db: &'a MemoryDb,
    config: &'a PatternsConfig,
    embedding: Embedding,
    /// Lazily-built HNSW index cached for the lifetime of this PatternStore.
    hnsw_cache: RefCell<Option<CachedIndex>>,
}

impl<'a> PatternStore<'a> {
    pub fn new(db: &'a MemoryDb, config: &'a PatternsConfig) -> Self {
        Self {
            db,
            config,
            embedding: Embedding::default(),
            hnsw_cache: RefCell::new(None),
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

    /// Run consolidation: promotion, decay, expiration, and deduplication.
    pub fn consolidate(&self) -> Result<()> {
        // 1. Promote eligible patterns
        self.promote_eligible()?;

        // 2. Apply confidence decay (A6)
        self.apply_decay()?;

        // 3. Expire old short-term patterns (TTL-based)
        self.expire_short_term()?;

        // 4. Deduplicate similar patterns (using stored vectors)
        self.deduplicate()?;

        Ok(())
    }

    /// Search patterns using HNSW index when >50 patterns exist, else brute-force. (A5)
    pub fn search_patterns(&self, query: &str, k: usize) -> Result<Vec<(ShortTermPattern, f32)>> {
        let query_vec = self.embedding.embed(query);
        let short_count = self.db.count_patterns_short()? as usize;

        if short_count > 50 {
            self.search_with_hnsw(&query_vec, k)
        } else {
            self.search_brute_force(&query_vec, k)
        }
    }

    /// Ensure the HNSW cache is built (or rebuilt if vector count changed).
    fn ensure_hnsw_cache(&self) -> Result<()> {
        let vectors = self.db.get_vectors_for_source("pattern_short")?;
        let current_count = vectors.len();

        let needs_rebuild = {
            let cache = self.hnsw_cache.borrow();
            match &*cache {
                Some(c) => c.built_from_count != current_count,
                None => true,
            }
        };

        if needs_rebuild {
            if vectors.is_empty() {
                *self.hnsw_cache.borrow_mut() = None;
                return Ok(());
            }

            let mut id_to_source: HashMap<i64, String> = HashMap::new();
            let mut points: Vec<(i64, Vec<f32>)> = Vec::new();
            for (db_id, source_id, vector) in &vectors {
                id_to_source.insert(*db_id, source_id.clone());
                points.push((*db_id, vector.clone()));
            }

            let mut index = HnswIndex::new();
            index.build(&points);

            *self.hnsw_cache.borrow_mut() = Some(CachedIndex {
                index,
                id_to_source,
                built_from_count: current_count,
            });
        }

        Ok(())
    }

    /// Search using cached HNSW index built from stored vectors.
    fn search_with_hnsw(
        &self,
        query_vec: &[f32],
        k: usize,
    ) -> Result<Vec<(ShortTermPattern, f32)>> {
        self.ensure_hnsw_cache()?;

        let cache = self.hnsw_cache.borrow();
        let cached = match &*cache {
            Some(c) => c,
            None => return Ok(Vec::new()),
        };

        let results = cached.index.search(query_vec, k);

        let mut scored = Vec::new();
        for (db_id, distance) in results {
            if let Some(pattern_id) = cached.id_to_source.get(&db_id) {
                if let Ok(Some(pattern)) = self.db.get_pattern_short(pattern_id) {
                    let similarity = 1.0 - distance; // distance = 1 - cosine_similarity
                    scored.push((pattern, similarity));
                }
            }
        }

        Ok(scored)
    }

    /// Brute-force search for small pattern sets (<= 50).
    fn search_brute_force(
        &self,
        query_vec: &[f32],
        k: usize,
    ) -> Result<Vec<(ShortTermPattern, f32)>> {
        let all_patterns = self.db.get_all_patterns_short()?;

        let mut scored: Vec<(ShortTermPattern, f32)> = all_patterns
            .into_iter()
            .map(|p| {
                let p_vec = self.embedding.embed(&p.content);
                let sim = Embedding::cosine_similarity(query_vec, &p_vec);
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

    /// Apply confidence decay based on time since last use. (A6)
    fn apply_decay(&self) -> Result<()> {
        let now = Utc::now();

        // Short-term patterns: decay at configured rate
        let short_patterns = self.db.get_all_patterns_short()?;
        for p in &short_patterns {
            let hours = (now - p.last_used).num_hours().max(0) as f64;
            if hours < 1.0 {
                continue;
            }
            let decayed = p.confidence - (self.config.decay_rate_per_hour * hours);
            if decayed < 0.1 {
                // Too low confidence — remove
                self.db.delete_pattern_short(&p.id)?;
                self.db.delete_vectors_for_source("pattern_short", &p.id)?;
            } else if (decayed - p.confidence).abs() > 0.001 {
                self.db.update_pattern_short_confidence(&p.id, decayed)?;
            }
        }

        // Long-term patterns: slower decay (0.1%/hr), never delete but mark dormant
        let long_patterns = self.db.get_all_patterns_long()?;
        for p in &long_patterns {
            let hours = (now - p.last_used).num_hours().max(0) as f64;
            if hours < 1.0 {
                continue;
            }
            let decay_rate = 0.001; // 0.1% per hour for long-term
            let decayed = (p.confidence - (decay_rate * hours)).max(0.05); // Floor at 0.05 (dormant)
            if (decayed - p.confidence).abs() > 0.001 {
                self.db.update_pattern_long_confidence(&p.id, decayed)?;
            }
        }

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

    /// Deduplicate using stored vectors instead of re-embedding. (A11)
    fn deduplicate(&self) -> Result<()> {
        let patterns = self.db.get_all_patterns_short()?;
        if patterns.len() < 2 {
            return Ok(());
        }

        // Load stored vectors indexed by source_id
        let vectors = self.db.get_vectors_for_source("pattern_short")?;
        let vec_map: HashMap<String, Vec<f32>> = vectors
            .into_iter()
            .map(|(_, source_id, vec)| (source_id, vec))
            .collect();

        let mut to_remove = Vec::new();
        let threshold = self.config.dedup_similarity_threshold as f32;

        for i in 0..patterns.len() {
            if to_remove.contains(&i) {
                continue;
            }
            let vec_i = match vec_map.get(&patterns[i].id) {
                Some(v) => v,
                None => continue,
            };
            for j in (i + 1)..patterns.len() {
                if to_remove.contains(&j) {
                    continue;
                }
                let vec_j = match vec_map.get(&patterns[j].id) {
                    Some(v) => v,
                    None => continue,
                };
                let sim = Embedding::cosine_similarity(vec_i, vec_j);
                if sim > threshold {
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
        let path = PathBuf::from(format!(
            "/tmp/flowforge-test-patterns-{}.db",
            Uuid::new_v4()
        ));
        MemoryDb::open(&path).unwrap()
    }

    #[test]
    fn test_store_and_search() {
        let db = setup_db();
        let config = PatternsConfig::default();
        let store = PatternStore::new(&db, &config);

        store
            .store_short_term("use cargo build for compilation", "rust")
            .unwrap();
        store
            .store_short_term("python uses pip for packages", "python")
            .unwrap();

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

    #[test]
    fn test_record_feedback_long_term() {
        let db = setup_db();
        let config = PatternsConfig {
            promotion_min_usage: 1,
            promotion_min_confidence: 0.4,
            ..PatternsConfig::default()
        };
        let store = PatternStore::new(&db, &config);

        let id = store.store_short_term("feedback pattern", "test").unwrap();
        store.record_usage(&id).unwrap();
        store.promote_eligible().unwrap();

        // Now give positive feedback on the long-term pattern
        store.record_feedback(&id, true).unwrap();
        let p = db.get_pattern_long(&id).unwrap().unwrap();
        assert_eq!(p.success_count, 1);
        assert!(p.confidence > 0.5);

        // Negative feedback
        store.record_feedback(&id, false).unwrap();
        let p = db.get_pattern_long(&id).unwrap().unwrap();
        assert_eq!(p.failure_count, 1);
    }

    #[test]
    fn test_record_feedback_short_term() {
        let db = setup_db();
        let config = PatternsConfig::default();
        let store = PatternStore::new(&db, &config);

        let id = store
            .store_short_term("short feedback pattern", "test")
            .unwrap();
        let original = db.get_pattern_short(&id).unwrap().unwrap();
        let original_conf = original.confidence;

        store.record_feedback(&id, true).unwrap();
        let updated = db.get_pattern_short(&id).unwrap().unwrap();
        assert!(updated.confidence > original_conf);

        store.record_feedback(&id, false).unwrap();
        let updated2 = db.get_pattern_short(&id).unwrap().unwrap();
        assert!(updated2.confidence < updated.confidence);
    }

    #[test]
    fn test_consolidate_runs_without_error() {
        let db = setup_db();
        let config = PatternsConfig::default();
        let store = PatternStore::new(&db, &config);

        store.store_short_term("pattern one", "test").unwrap();
        store.store_short_term("pattern two", "test").unwrap();

        // Should run all phases without error
        store.consolidate().unwrap();
    }
}
