use super::*;
use std::path::PathBuf;

use flowforge_core::config::PatternsConfig;
use flowforge_core::PatternTier;
use uuid::Uuid;

use crate::db::MemoryDb;

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

    let results = store.search_all_patterns("cargo rust", 2).unwrap();
    assert!(!results.is_empty());
    assert!(results[0].content.contains("cargo"));
    assert_eq!(results[0].tier, PatternTier::Short);
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
    let mut config = PatternsConfig::default();
    // Raise promotion thresholds so instant promotion doesn't fire during this test
    config.promotion_min_usage = 10;
    config.promotion_min_confidence = 0.9;
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
fn test_record_feedback_instant_promotion() {
    let db = setup_db();
    let config = PatternsConfig::default(); // promotion_min_usage=1, promotion_min_confidence=0.5
    let store = PatternStore::new(&db, &config);

    let id = store
        .store_short_term("instant promote pattern", "test")
        .unwrap();
    // Pattern starts at confidence 0.5, usage 0
    assert!(db.get_pattern_short(&id).unwrap().is_some());
    assert!(db.get_pattern_long(&id).unwrap().is_none());

    // Positive feedback: confidence → 0.55, usage → 1 → meets thresholds → instant promotion
    store.record_feedback(&id, true).unwrap();
    assert!(db.get_pattern_short(&id).unwrap().is_none(), "should be removed from short-term");
    let long = db.get_pattern_long(&id).unwrap().expect("should be in long-term");
    assert!(long.confidence >= 0.55);
    assert_eq!(long.usage_count, 1);
}

/// Prove: pattern injection follow-through drives instant promotion.
/// Simulates the post_tool_use.rs flow: inject pattern → tool success → record_feedback → promoted.
#[test]
fn test_injection_follow_through_drives_promotion() {
    let db = setup_db();
    let config = PatternsConfig::default(); // min_usage=1, min_confidence=0.5

    // Step 1: Store a pattern (simulates learning from a previous session)
    let store = PatternStore::new(&db, &config);
    let id = store.store_short_term("when fixing SQL, always check indexes first", "error_fix").unwrap();

    // Verify it's in short-term with default confidence 0.5
    let p = db.get_pattern_short(&id).unwrap().unwrap();
    assert!((p.confidence - 0.5).abs() < 0.01);
    assert_eq!(p.usage_count, 0);

    // Step 2: user_prompt_submit injects this pattern into context
    // (simulated by recording the injection)
    use flowforge_core::SessionInfo;
    let session = SessionInfo {
        id: "sess-injection".to_string(),
        started_at: chrono::Utc::now(),
        ended_at: None,
        cwd: "/tmp".to_string(),
        edits: 0, commands: 0, summary: None, transcript_path: None,
    };
    db.create_session(&session).unwrap();
    db.record_context_injection("sess-injection", None, "pattern", Some(&id), Some(0.8), None).unwrap();

    // Step 3: Tool succeeds → post_tool_use calls record_feedback(id, true)
    // This is what the injection follow-through code does in post_tool_use.rs
    store.record_feedback(&id, true).unwrap();

    // Step 4: PROVE the pattern was instantly promoted to long-term
    assert!(db.get_pattern_short(&id).unwrap().is_none(), "should be removed from short-term");
    let long = db.get_pattern_long(&id).unwrap().expect("should be in long-term after feedback");
    assert!(long.confidence >= 0.55, "confidence should have increased");
    assert_eq!(long.usage_count, 1, "usage should have incremented");

    // This proves: a pattern stored in session N, injected in session N+1,
    // and confirmed by tool success gets promoted to long-term WITHIN the same prompt cycle.
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

#[test]
fn test_search_finds_promoted_patterns() {
    let db = setup_db();
    let config = PatternsConfig {
        promotion_min_usage: 1,
        promotion_min_confidence: 0.4,
        ..PatternsConfig::default()
    };
    let store = PatternStore::new(&db, &config);

    let id = store
        .store_short_term("deploy kubernetes service", "devops")
        .unwrap();
    store.record_usage(&id).unwrap();

    // Promote to long-term
    let promoted = store.promote_eligible().unwrap();
    assert_eq!(promoted, 1);
    assert_eq!(db.count_patterns_short().unwrap(), 0);
    assert_eq!(db.count_patterns_long().unwrap(), 1);

    // Search should still find it, now as Long tier
    let results = store.search_all_patterns("deploy kubernetes", 5).unwrap();
    assert!(!results.is_empty());
    assert_eq!(results[0].tier, PatternTier::Long);
    assert!(results[0].content.contains("kubernetes"));
}

#[test]
fn test_search_all_combines_tiers() {
    let db = setup_db();
    let config = PatternsConfig {
        promotion_min_usage: 1,
        promotion_min_confidence: 0.4,
        ..PatternsConfig::default()
    };
    let store = PatternStore::new(&db, &config);

    // Store 3 short-term patterns
    store
        .store_short_term("fix rust compilation error", "rust")
        .unwrap();
    store
        .store_short_term("debug rust test failure", "rust")
        .unwrap();
    let id3 = store
        .store_short_term("optimize rust build time", "rust")
        .unwrap();

    // Promote one to long-term
    store.record_usage(&id3).unwrap();
    store.promote_eligible().unwrap();

    assert_eq!(db.count_patterns_short().unwrap(), 2);
    assert_eq!(db.count_patterns_long().unwrap(), 1);

    // Search should return results from both tiers
    let results = store.search_all_patterns("rust", 5).unwrap();
    assert_eq!(results.len(), 3);

    let has_short = results.iter().any(|m| m.tier == PatternTier::Short);
    let has_long = results.iter().any(|m| m.tier == PatternTier::Long);
    assert!(has_short, "Expected short-term results");
    assert!(has_long, "Expected long-term results");
}

#[test]
fn test_enforce_long_term_max() {
    let db = setup_db();
    let config = PatternsConfig {
        promotion_min_usage: 1,
        promotion_min_confidence: 0.4,
        long_term_max: 3,
        ..PatternsConfig::default()
    };
    let store = PatternStore::new(&db, &config);

    // Store and promote 6 patterns (exceeds max of 3)
    for i in 0..6 {
        let id = store
            .store_short_term(&format!("pattern number {i}"), "test")
            .unwrap();
        store.record_usage(&id).unwrap();
    }
    store.promote_eligible().unwrap();
    assert_eq!(db.count_patterns_long().unwrap(), 6);

    // Enforce max — should prune down to 3
    store.enforce_long_term_max().unwrap();
    assert!(db.count_patterns_long().unwrap() <= 3);
}

#[test]
fn test_migrate_embeddings() {
    let db = setup_db();
    let config = PatternsConfig::default();
    let store = PatternStore::new(&db, &config);

    // Store some patterns
    store
        .store_short_term("test migration pattern", "test")
        .unwrap();
    store
        .store_short_term("another test pattern", "test")
        .unwrap();

    // Set meta to current version — migration should be a no-op
    use crate::embedding::EMBEDDING_VERSION;
    db.set_meta("embedding_version", &EMBEDDING_VERSION.to_string())
        .unwrap();

    // Should succeed without re-embedding (version matches)
    store.migrate_embeddings().unwrap();

    // Force stale version
    db.set_meta("embedding_version", "0").unwrap();

    // Should re-embed all vectors
    store.migrate_embeddings().unwrap();

    // Version should now be updated
    let version = db.get_meta("embedding_version").unwrap();
    assert_eq!(version, Some(EMBEDDING_VERSION.to_string()));

    // Vectors should still exist and search should work
    let results = store.search_all_patterns("test migration", 5).unwrap();
    assert!(!results.is_empty());
}

#[test]
fn test_hnsw_cache_invalidated_after_promotion() {
    let db = setup_db();
    let config = PatternsConfig {
        promotion_min_usage: 1,
        promotion_min_confidence: 0.4,
        ..PatternsConfig::default()
    };
    let store = PatternStore::new(&db, &config);

    // Store enough patterns to trigger HNSW (>50)
    for i in 0..55 {
        store
            .store_short_term(&format!("hnsw test pattern {i}"), "test")
            .unwrap();
    }

    // First search builds the HNSW cache
    let results1 = store.search_all_patterns("hnsw test pattern 0", 5).unwrap();
    assert!(!results1.is_empty());

    // Promote pattern 0 — should invalidate cache
    let patterns = db.get_all_patterns_short().unwrap();
    let p0 = patterns
        .iter()
        .find(|p| p.content == "hnsw test pattern 0")
        .unwrap();
    store.record_usage(&p0.id).unwrap();
    store.promote_eligible().unwrap();

    // Search again — cache should rebuild, promoted pattern should appear as Long
    let results2 = store.search_all_patterns("hnsw test pattern 0", 5).unwrap();
    assert!(!results2.is_empty());
    assert_eq!(
        results2[0].tier,
        PatternTier::Long,
        "Promoted pattern should be Long tier after cache rebuild"
    );
}

#[test]
fn test_dedup_catches_similar_patterns() {
    let db = setup_db();
    let config = PatternsConfig {
        dedup_similarity_threshold: 0.88,
        ..PatternsConfig::default()
    };
    let store = PatternStore::new(&db, &config);

    // Store two very similar patterns (same words, slight reorder)
    store
        .store_short_term("use cargo build for rust compilation", "rust")
        .unwrap();
    store
        .store_short_term("use cargo build for rust compilation tasks", "rust")
        .unwrap();

    assert_eq!(db.count_patterns_short().unwrap(), 2);

    // Run dedup
    store.deduplicate().unwrap();

    // Should have removed one (they share almost all n-grams)
    let remaining = db.count_patterns_short().unwrap();
    assert!(
        remaining <= 1,
        "Expected dedup to remove near-duplicate, got {remaining} remaining"
    );
}

// ── Learning Hardening Tests ──

#[test]
fn test_promotion_blocked_by_failure_correlation() {
    let db = setup_db();
    let config = PatternsConfig {
        promotion_min_usage: 1,
        promotion_min_confidence: 0.4,
        demotion_min_feedback: 3,
        promotion_failure_correlation_max: 0.3,
        ..PatternsConfig::default()
    };
    let store = PatternStore::new(&db, &config);

    let id = store
        .store_short_term("bad pattern that fails", "test")
        .unwrap();
    store.record_usage(&id).unwrap();

    // Record enough failure effectiveness to block promotion
    for i in 0..5 {
        db.record_pattern_effectiveness(&id, &format!("sess-{i}"), "failure", 0.8)
            .unwrap();
    }
    db.recompute_pattern_effectiveness(&id).unwrap();

    let promoted = store.promote_eligible().unwrap();
    assert_eq!(
        promoted, 0,
        "Pattern with high failure correlation should not promote"
    );
    assert_eq!(db.count_patterns_short().unwrap(), 1);
    assert_eq!(db.count_patterns_long().unwrap(), 0);
}

#[test]
fn test_promotion_allowed_when_effectiveness_good() {
    let db = setup_db();
    let config = PatternsConfig {
        promotion_min_usage: 1,
        promotion_min_confidence: 0.4,
        demotion_min_feedback: 3,
        promotion_failure_correlation_max: 0.3,
        ..PatternsConfig::default()
    };
    let store = PatternStore::new(&db, &config);

    let id = store
        .store_short_term("good pattern that succeeds", "test")
        .unwrap();
    store.record_usage(&id).unwrap();

    // Record enough success effectiveness
    for i in 0..5 {
        db.record_pattern_effectiveness(&id, &format!("sess-{i}"), "success", 0.8)
            .unwrap();
    }
    db.recompute_pattern_effectiveness(&id).unwrap();

    let promoted = store.promote_eligible().unwrap();
    assert_eq!(
        promoted, 1,
        "Pattern with good effectiveness should promote"
    );
    assert_eq!(db.count_patterns_long().unwrap(), 1);
}

#[test]
fn test_promotion_allowed_when_insufficient_feedback() {
    let db = setup_db();
    let config = PatternsConfig {
        promotion_min_usage: 1,
        promotion_min_confidence: 0.4,
        demotion_min_feedback: 5,
        promotion_failure_correlation_max: 0.3,
        ..PatternsConfig::default()
    };
    let store = PatternStore::new(&db, &config);

    let id = store
        .store_short_term("new pattern with little data", "test")
        .unwrap();
    store.record_usage(&id).unwrap();

    // Only 2 samples — below demotion_min_feedback threshold of 5
    db.record_pattern_effectiveness(&id, "sess-fake", "failure", 0.8)
        .unwrap();
    db.record_pattern_effectiveness(&id, "sess-fake2", "failure", 0.8)
        .unwrap();

    let promoted = store.promote_eligible().unwrap();
    assert_eq!(
        promoted, 1,
        "Pattern with insufficient feedback should still promote"
    );
}

#[test]
fn test_demote_failing_long_term_patterns() {
    let db = setup_db();
    let config = PatternsConfig {
        promotion_min_usage: 1,
        promotion_min_confidence: 0.4,
        demotion_min_feedback: 3,
        demotion_failure_ratio: 0.6,
        ..PatternsConfig::default()
    };
    let store = PatternStore::new(&db, &config);

    let id = store.store_short_term("pattern to demote", "test").unwrap();
    store.record_usage(&id).unwrap();
    store.promote_eligible().unwrap();
    assert_eq!(db.count_patterns_long().unwrap(), 1);

    // Record heavy failures (4 fail, 1 success = 0.8 failure ratio > 0.6 threshold)
    for _ in 0..4 {
        store.record_feedback(&id, false).unwrap();
    }
    store.record_feedback(&id, true).unwrap();

    let demoted = store.demote_failing().unwrap();
    assert_eq!(
        demoted, 1,
        "Pattern with 80% failure ratio should be demoted"
    );
    assert_eq!(db.count_patterns_long().unwrap(), 0);
}

#[test]
fn test_demote_skips_insufficient_feedback() {
    let db = setup_db();
    let config = PatternsConfig {
        promotion_min_usage: 1,
        promotion_min_confidence: 0.4,
        demotion_min_feedback: 10,
        demotion_failure_ratio: 0.6,
        ..PatternsConfig::default()
    };
    let store = PatternStore::new(&db, &config);

    let id = store
        .store_short_term("pattern with few samples", "test")
        .unwrap();
    store.record_usage(&id).unwrap();
    store.promote_eligible().unwrap();

    // Only 3 failures — below demotion_min_feedback of 10
    for _ in 0..3 {
        store.record_feedback(&id, false).unwrap();
    }

    let demoted = store.demote_failing().unwrap();
    assert_eq!(
        demoted, 0,
        "Pattern with insufficient feedback should not be demoted"
    );
    assert_eq!(db.count_patterns_long().unwrap(), 1);
}

#[test]
fn test_demote_keeps_successful_patterns() {
    let db = setup_db();
    let config = PatternsConfig {
        promotion_min_usage: 1,
        promotion_min_confidence: 0.4,
        demotion_min_feedback: 3,
        demotion_failure_ratio: 0.6,
        ..PatternsConfig::default()
    };
    let store = PatternStore::new(&db, &config);

    let id = store
        .store_short_term("successful pattern", "test")
        .unwrap();
    store.record_usage(&id).unwrap();
    store.promote_eligible().unwrap();

    // Record mostly successes (4 success, 1 failure = 0.2 failure ratio < 0.6)
    for _ in 0..4 {
        store.record_feedback(&id, true).unwrap();
    }
    store.record_feedback(&id, false).unwrap();

    let demoted = store.demote_failing().unwrap();
    assert_eq!(
        demoted, 0,
        "Pattern with low failure ratio should not be demoted"
    );
    assert_eq!(db.count_patterns_long().unwrap(), 1);
}

#[test]
fn test_consolidate_runs_demotion() {
    let db = setup_db();
    let config = PatternsConfig {
        promotion_min_usage: 1,
        promotion_min_confidence: 0.4,
        demotion_min_feedback: 3,
        demotion_failure_ratio: 0.5,
        ..PatternsConfig::default()
    };
    let store = PatternStore::new(&db, &config);

    let id = store
        .store_short_term("pattern to demote via consolidate", "test")
        .unwrap();
    store.record_usage(&id).unwrap();
    store.promote_eligible().unwrap();
    assert_eq!(db.count_patterns_long().unwrap(), 1);

    // Record failures to trigger demotion during consolidation
    for _ in 0..5 {
        store.record_feedback(&id, false).unwrap();
    }

    store.consolidate().unwrap();
    assert_eq!(
        db.count_patterns_long().unwrap(),
        0,
        "Consolidation should have demoted the failing pattern"
    );
}

#[test]
fn test_demotion_config_defaults() {
    let config = PatternsConfig::default();
    assert_eq!(config.demotion_min_feedback, 5);
    assert!((config.demotion_failure_ratio - 0.6).abs() < f64::EPSILON);
    assert!((config.promotion_failure_correlation_max - 0.3).abs() < f64::EPSILON);
}

// ── Module Structure Tests ──

#[test]
fn test_module_structure_re_exports() {
    // Verify PatternStore is accessible from the patterns module
    let db_path = PathBuf::from(format!(
        "/tmp/flowforge-test-patterns-structure-{}.db",
        Uuid::new_v4()
    ));
    let db = MemoryDb::open(&db_path).unwrap();
    let config = PatternsConfig::default();

    // PatternStore::new should work (proves struct + new are re-exported properly)
    let store = PatternStore::new(&db, &config);

    // All public methods from different submodules should be accessible
    let id = store.store_short_term("structure test", "test").unwrap();
    assert!(!id.is_empty());
    store.record_usage(&id).unwrap();
    store.record_feedback(&id, true).unwrap();

    let results = store.search_all_patterns("structure", 5).unwrap();
    assert!(!results.is_empty());
}

#[test]
fn test_module_all_lifecycle_methods_accessible() {
    // Verify all lifecycle methods (promote, demote, consolidate) work through the module
    let db_path = PathBuf::from(format!(
        "/tmp/flowforge-test-patterns-lifecycle-{}.db",
        Uuid::new_v4()
    ));
    let db = MemoryDb::open(&db_path).unwrap();
    let config = PatternsConfig {
        promotion_min_usage: 1,
        promotion_min_confidence: 0.4,
        ..PatternsConfig::default()
    };
    let store = PatternStore::new(&db, &config);

    // Store and promote
    let id = store.store_short_term("lifecycle test", "test").unwrap();
    store.record_usage(&id).unwrap();
    let promoted = store.promote_eligible().unwrap();
    assert_eq!(promoted, 1);

    // Demote (no failures yet, so 0 demoted is fine — just verify it runs)
    let demoted = store.demote_failing().unwrap();
    assert_eq!(demoted, 0);

    // Consolidate (runs all lifecycle phases)
    store
        .store_short_term("consolidation target", "test")
        .unwrap();
    store.consolidate().unwrap();
}

#[test]
fn test_with_cache_shares_hnsw_index() {
    let db_path = std::path::PathBuf::from(format!(
        "/tmp/flowforge-test-patterns-cache-{}.db",
        Uuid::new_v4()
    ));
    let db = MemoryDb::open(&db_path).unwrap();
    let config = PatternsConfig::default();
    let cache = super::new_hnsw_cache();

    // First store: store enough patterns to trigger HNSW (>50)
    let store1 = PatternStore::with_cache(&db, &config, &cache);
    for i in 0..55 {
        store1
            .store_short_term(&format!("pattern {i}"), "test")
            .unwrap();
    }
    // Search triggers HNSW build
    let results = store1.search_all_patterns("pattern 1", 5).unwrap();
    assert!(!results.is_empty());

    // Cache should now be populated
    assert!(cache.borrow().is_some());

    // Second store: reuses the same cache without rebuild
    let store2 = PatternStore::with_cache(&db, &config, &cache);
    let results2 = store2.search_all_patterns("pattern 2", 5).unwrap();
    assert!(!results2.is_empty());
    // Cache still has the same built_from_count (no rebuild)
    assert_eq!(cache.borrow().as_ref().unwrap().built_from_count, 55);
}

#[test]
fn test_new_creates_independent_cache() {
    let db_path = std::path::PathBuf::from(format!(
        "/tmp/flowforge-test-patterns-indep-{}.db",
        Uuid::new_v4()
    ));
    let db = MemoryDb::open(&db_path).unwrap();
    let config = PatternsConfig::default();

    // Using new() creates an owned cache that doesn't survive between stores
    let store = PatternStore::new(&db, &config);
    store.store_short_term("test pattern", "test").unwrap();
    let results = store.search_all_patterns("test", 5).unwrap();
    assert!(!results.is_empty());
}
