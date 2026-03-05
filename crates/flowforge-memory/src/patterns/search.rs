use std::collections::HashMap;

use flowforge_core::{PatternMatch, PatternTier, Result};

use crate::hnsw::HnswIndex;

use super::CachedIndex;
use super::PatternStore;

impl<'a> PatternStore<'a> {
    /// Fetch a pattern by ID and tier, returning a PatternMatch if found.
    pub(super) fn fetch_pattern_match(
        &self,
        id: &str,
        tier: PatternTier,
        similarity: f32,
    ) -> Option<PatternMatch> {
        match tier {
            PatternTier::Short => {
                if let Ok(Some(p)) = self.db.get_pattern_short(id) {
                    Some(PatternMatch {
                        id: p.id,
                        content: p.content,
                        category: p.category,
                        confidence: p.confidence,
                        usage_count: p.usage_count,
                        tier: PatternTier::Short,
                        similarity,
                    })
                } else {
                    None
                }
            }
            PatternTier::Long => {
                if let Ok(Some(p)) = self.db.get_pattern_long(id) {
                    Some(PatternMatch {
                        id: p.id,
                        content: p.content,
                        category: p.category,
                        confidence: p.confidence,
                        usage_count: p.usage_count,
                        tier: PatternTier::Long,
                        similarity,
                    })
                } else {
                    None
                }
            }
        }
    }

    /// Ensure the HNSW cache is built from BOTH tiers (or rebuilt if vector count changed).
    pub(super) fn ensure_hnsw_cache(&self) -> Result<()> {
        let short_vecs = self.db.get_vectors_for_source("pattern_short")?;
        let long_vecs = self.db.get_vectors_for_source("pattern_long")?;
        let current_count = short_vecs.len() + long_vecs.len();

        let needs_rebuild = {
            let cache = self.cache().borrow();
            match &*cache {
                Some(c) => c.built_from_count != current_count,
                None => true,
            }
        };

        if needs_rebuild {
            if current_count == 0 {
                *self.cache().borrow_mut() = None;
                return Ok(());
            }

            let mut id_to_source: HashMap<i64, (String, PatternTier)> = HashMap::new();
            let mut points: Vec<(i64, Vec<f32>)> = Vec::new();

            for (db_id, source_id, vector) in &short_vecs {
                id_to_source.insert(*db_id, (source_id.clone(), PatternTier::Short));
                points.push((*db_id, vector.clone()));
            }
            for (db_id, source_id, vector) in &long_vecs {
                id_to_source.insert(*db_id, (source_id.clone(), PatternTier::Long));
                points.push((*db_id, vector.clone()));
            }

            let mut index = HnswIndex::new();
            index.build(&points);

            *self.cache().borrow_mut() = Some(CachedIndex {
                index,
                id_to_source,
                built_from_count: current_count,
            });
        }

        Ok(())
    }

    /// Search using cached HNSW index built from stored vectors (both tiers).
    pub(super) fn search_with_hnsw(
        &self,
        query_vec: &[f32],
        k: usize,
    ) -> Result<Vec<PatternMatch>> {
        self.ensure_hnsw_cache()?;

        let cache = self.cache().borrow();
        let cached = match &*cache {
            Some(c) => c,
            None => return Ok(Vec::new()),
        };

        let results = cached.index.search(query_vec, k);

        let mut scored = Vec::new();
        for (db_id, distance) in results {
            if let Some((pattern_id, tier)) = cached.id_to_source.get(&db_id) {
                let similarity = 1.0 - distance;
                if let Some(m) = self.fetch_pattern_match(pattern_id, *tier, similarity) {
                    scored.push(m);
                }
            }
        }

        Ok(scored)
    }

    /// Brute-force search using stored vectors across both tiers.
    pub(super) fn search_brute_force(
        &self,
        query_vec: &[f32],
        k: usize,
    ) -> Result<Vec<PatternMatch>> {
        let short_vecs = self.db.get_vectors_for_source("pattern_short")?;
        let long_vecs = self.db.get_vectors_for_source("pattern_long")?;

        let mut scored: Vec<(String, PatternTier, f32)> = Vec::new();

        for (_, source_id, vec) in &short_vecs {
            let sim = crate::embedding::cosine_similarity(query_vec, vec);
            scored.push((source_id.clone(), PatternTier::Short, sim));
        }
        for (_, source_id, vec) in &long_vecs {
            let sim = crate::embedding::cosine_similarity(query_vec, vec);
            scored.push((source_id.clone(), PatternTier::Long, sim));
        }

        scored.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(k);

        let mut results = Vec::new();
        for (id, tier, similarity) in scored {
            if let Some(m) = self.fetch_pattern_match(&id, tier, similarity) {
                results.push(m);
            }
        }

        Ok(results)
    }
}
