use flowforge_core::{FlowForgeConfig, Result};
use flowforge_memory::MemoryDb;
use flowforge_agents::AgentRegistry;
use colored::Colorize;

pub fn run() -> Result<()> {
    println!("{}", "FlowForge Status".bold());
    println!("{}", "─".repeat(40));

    let config_path = FlowForgeConfig::config_path();
    let config = FlowForgeConfig::load(&config_path)?;

    // Project status
    let project_dir = FlowForgeConfig::project_dir();
    if project_dir.exists() {
        println!("Project: {} ({})", "initialized".green(), project_dir.display());
    } else {
        println!("Project: {}", "not initialized".red());
        println!("Run `flowforge init --project` to set up.");
        return Ok(());
    }

    // Database status
    let db_path = config.db_path();
    if db_path.exists() {
        let db = MemoryDb::open(&db_path)?;

        if let Ok(Some(session)) = db.get_current_session() {
            println!("Session: {} ({} edits, {} commands)",
                session.id[..8].to_string().cyan(),
                session.edits,
                session.commands,
            );
        } else {
            println!("Session: {}", "none active".yellow());
        }

        if let Ok(count) = db.count_kv() {
            println!("Memory: {} entries", count);
        }

        if let Ok(count) = db.count_patterns() {
            println!("Patterns: {} total", count);
        }
    } else {
        println!("Database: {}", "not found".red());
    }

    // Agent status
    if let Ok(registry) = AgentRegistry::load(&config.agents) {
        println!("Agents: {} loaded", registry.len());
    } else {
        println!("Agents: {}", "failed to load".red());
    }

    // Hooks status
    println!("\nHooks:");
    println!("  Bash validation: {}", if config.hooks.bash_validation { "enabled".green() } else { "disabled".yellow() });
    println!("  Edit tracking:   {}", if config.hooks.edit_tracking { "enabled".green() } else { "disabled".yellow() });
    println!("  Routing:         {}", if config.hooks.routing { "enabled".green() } else { "disabled".yellow() });
    println!("  Learning:        {}", if config.hooks.learning { "enabled".green() } else { "disabled".yellow() });

    Ok(())
}
