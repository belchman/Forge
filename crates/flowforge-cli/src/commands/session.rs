use flowforge_core::{FlowForgeConfig, Result};
use flowforge_memory::MemoryDb;
use colored::Colorize;

fn open_db() -> Result<MemoryDb> {
    let config = FlowForgeConfig::load(&FlowForgeConfig::config_path())?;
    MemoryDb::open(&config.db_path())
}

pub fn current() -> Result<()> {
    let db = open_db()?;
    match db.get_current_session()? {
        Some(session) => {
            println!("{}", "Current Session".bold());
            println!("ID:       {}", session.id.cyan());
            println!("Started:  {}", session.started_at);
            println!("CWD:      {}", session.cwd);
            println!("Edits:    {}", session.edits);
            println!("Commands: {}", session.commands);
        }
        None => {
            println!("{}", "No active session".yellow());
        }
    }
    Ok(())
}

pub fn list(limit: usize) -> Result<()> {
    let db = open_db()?;
    let sessions = db.list_sessions(limit)?;
    if sessions.is_empty() {
        println!("No sessions recorded");
        return Ok(());
    }

    println!("{:<10} {:<20} {:<6} {:<6} {}", "ID", "Started", "Edits", "Cmds", "Status");
    println!("{}", "─".repeat(60));

    for session in &sessions {
        let status = if session.ended_at.is_some() {
            "ended".dimmed().to_string()
        } else {
            "active".green().to_string()
        };

        println!(
            "{:<10} {:<20} {:<6} {:<6} {}",
            &session.id[..8],
            session.started_at.format("%Y-%m-%d %H:%M"),
            session.edits,
            session.commands,
            status,
        );
    }
    Ok(())
}

pub fn metrics() -> Result<()> {
    let db = open_db()?;
    let sessions = db.list_sessions(100)?;

    let total_sessions = sessions.len();
    let total_edits: u64 = sessions.iter().map(|s| s.edits).sum();
    let total_commands: u64 = sessions.iter().map(|s| s.commands).sum();
    let active = sessions.iter().filter(|s| s.ended_at.is_none()).count();

    println!("{}", "Session Metrics".bold());
    println!("Total sessions: {}", total_sessions);
    println!("Active:         {}", active);
    println!("Total edits:    {}", total_edits);
    println!("Total commands: {}", total_commands);

    if total_sessions > 0 {
        println!("Avg edits/session:    {:.1}", total_edits as f64 / total_sessions as f64);
        println!("Avg commands/session: {:.1}", total_commands as f64 / total_sessions as f64);
    }

    Ok(())
}
