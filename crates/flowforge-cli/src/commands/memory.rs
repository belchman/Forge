use colored::Colorize;
use flowforge_core::{FlowForgeConfig, Result};
use flowforge_memory::MemoryDb;

fn open_db() -> Result<MemoryDb> {
    let config = FlowForgeConfig::load(&FlowForgeConfig::config_path())?;
    let db_path = config.db_path();
    if !db_path.exists() {
        return Err(flowforge_core::Error::Config(
            "FlowForge not initialized. Run `flowforge init --project` first.".to_string(),
        ));
    }
    MemoryDb::open(&db_path)
}

pub fn get(key: &str, namespace: &str) -> Result<()> {
    let db = open_db()?;
    match db.kv_get(key, namespace)? {
        Some(value) => println!("{value}"),
        None => {
            eprintln!(
                "{}: key '{}' not found in namespace '{}'",
                "Not found".yellow(),
                key,
                namespace
            );
            std::process::exit(1);
        }
    }
    Ok(())
}

pub fn set(key: &str, value: &str, namespace: &str) -> Result<()> {
    let db = open_db()?;
    db.kv_set(key, value, namespace)?;
    println!("{} Set {}={}", "✓".green(), key, value);
    Ok(())
}

pub fn delete(key: &str, namespace: &str) -> Result<()> {
    let db = open_db()?;
    db.kv_delete(key, namespace)?;
    println!("{} Deleted {}", "✓".green(), key);
    Ok(())
}

pub fn list(namespace: &str) -> Result<()> {
    let db = open_db()?;
    let keys = db.kv_list(namespace)?;
    if keys.is_empty() {
        println!("No entries in namespace '{namespace}'");
    } else {
        for (key, value) in &keys {
            println!("{}: {}", key.cyan(), value);
        }
    }
    Ok(())
}

pub fn search(query: &str, limit: usize) -> Result<()> {
    let db = open_db()?;
    let results = db.kv_search(query, limit)?;
    if results.is_empty() {
        println!("No results for '{query}'");
    } else {
        for (key, value, ns) in &results {
            println!("[{}] {}: {}", ns.dimmed(), key.cyan(), value);
        }
    }
    Ok(())
}
