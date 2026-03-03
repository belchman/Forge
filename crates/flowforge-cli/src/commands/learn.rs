use flowforge_core::{FlowForgeConfig, Result};
use flowforge_memory::{MemoryDb, PatternStore};
use colored::Colorize;

pub fn store(content: &str, category: &str) -> Result<()> {
    let config = FlowForgeConfig::load(&FlowForgeConfig::config_path())?;
    let db = MemoryDb::open(&config.db_path())?;
    let pattern_store = PatternStore::new(&db, &config.patterns);

    let id = pattern_store.store_short_term(content, category)?;
    println!("{} Stored pattern {} in category '{}'", "✓".green(), &id[..8], category);
    Ok(())
}

pub fn search(query: &str, limit: usize) -> Result<()> {
    let config = FlowForgeConfig::load(&FlowForgeConfig::config_path())?;
    let db = MemoryDb::open(&config.db_path())?;

    let short = db.search_patterns_short(query, limit)?;
    let long = db.search_patterns_long(query, limit)?;

    if short.is_empty() && long.is_empty() {
        println!("No patterns found for '{query}'");
        return Ok(());
    }

    if !long.is_empty() {
        println!("{}", "Long-term patterns:".bold());
        for p in &long {
            println!("  [{}] {} (conf: {:.0}%, used: {}x)",
                p.category.cyan(), p.content, p.confidence * 100.0, p.usage_count);
        }
    }

    if !short.is_empty() {
        println!("{}", "Short-term patterns:".bold());
        for p in &short {
            println!("  [{}] {} (conf: {:.0}%, used: {}x)",
                p.category.cyan(), p.content, p.confidence * 100.0, p.usage_count);
        }
    }

    Ok(())
}

pub fn stats() -> Result<()> {
    let config = FlowForgeConfig::load(&FlowForgeConfig::config_path())?;
    let db = MemoryDb::open(&config.db_path())?;

    let short_count = db.count_patterns_short()?;
    let long_count = db.count_patterns_long()?;
    let weights_count = db.count_routing_weights()?;

    println!("{}", "Learning Statistics".bold());
    println!("Short-term patterns: {} / {} max", short_count, config.patterns.short_term_max);
    println!("Long-term patterns:  {} / {} max", long_count, config.patterns.long_term_max);
    println!("Routing weights:     {}", weights_count);

    println!("\nConfig:");
    println!("  Promotion threshold: {}x usage, {:.0}% confidence",
        config.patterns.promotion_min_usage, config.patterns.promotion_min_confidence * 100.0);
    println!("  Decay rate: {:.1}%/hour", config.patterns.decay_rate_per_hour * 100.0);
    println!("  Dedup threshold: {:.0}%", config.patterns.dedup_similarity_threshold * 100.0);

    Ok(())
}
