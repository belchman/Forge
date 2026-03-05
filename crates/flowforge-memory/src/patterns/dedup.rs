use std::collections::{HashMap, HashSet};

use flowforge_core::Result;

use crate::embedding::cosine_similarity;

use super::PatternStore;

impl<'a> PatternStore<'a> {
    /// Get the dedup threshold, using per-cluster p95 when both vectors are in the same cluster.
    pub(super) fn get_dedup_threshold(
        &self,
        embedding_id_a: Option<i64>,
        embedding_id_b: Option<i64>,
    ) -> f32 {
        if let (Some(eid_a), Some(eid_b)) = (embedding_id_a, embedding_id_b) {
            if let (Ok(cluster_a), Ok(cluster_b)) = (
                self.db.get_vector_cluster_id(eid_a),
                self.db.get_vector_cluster_id(eid_b),
            ) {
                if let (Some(ca), Some(cb)) = (cluster_a, cluster_b) {
                    if ca == cb {
                        // Same cluster — use cluster's p95 as threshold (convert distance to similarity)
                        if let Ok(Some(cluster)) = self.db.get_cluster(ca) {
                            return 1.0 - cluster.p95_distance as f32;
                        }
                    }
                }
            }
        }
        self.config.dedup_similarity_threshold as f32
    }

    /// Deduplicate using stored vectors instead of re-embedding. (A11)
    /// Uses per-cluster p95 thresholds when available.
    pub(super) fn deduplicate(&self) -> Result<()> {
        let patterns = self.db.get_all_patterns_short()?;
        if patterns.len() < 2 {
            return Ok(());
        }

        // Load stored vectors indexed by source_id, also track embedding IDs
        let vectors = self.db.get_vectors_for_source("pattern_short")?;
        let vec_map: HashMap<String, (Vec<f32>, i64)> = vectors
            .into_iter()
            .map(|(db_id, source_id, vec)| (source_id, (vec, db_id)))
            .collect();

        let mut to_remove: HashSet<usize> = HashSet::new();
        let fallback_threshold = self.config.dedup_similarity_threshold as f32;

        for i in 0..patterns.len() {
            if to_remove.contains(&i) {
                continue;
            }
            let (vec_i, eid_i) = match vec_map.get(&patterns[i].id) {
                Some(v) => (&v.0, Some(v.1)),
                None => continue,
            };
            for j in (i + 1)..patterns.len() {
                if to_remove.contains(&j) {
                    continue;
                }
                let (vec_j, eid_j) = match vec_map.get(&patterns[j].id) {
                    Some(v) => (&v.0, Some(v.1)),
                    None => continue,
                };
                let sim = cosine_similarity(vec_i, vec_j);
                let threshold = self.get_dedup_threshold(eid_i, eid_j);
                // Use at least the fallback threshold to prevent over-aggressive dedup
                let threshold = threshold.max(fallback_threshold);
                if sim > threshold {
                    if patterns[j].confidence < patterns[i].confidence
                        || (patterns[j].confidence == patterns[i].confidence
                            && patterns[j].usage_count < patterns[i].usage_count)
                    {
                        to_remove.insert(j);
                    } else {
                        to_remove.insert(i);
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
}
