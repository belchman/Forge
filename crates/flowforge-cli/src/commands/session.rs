use colored::Colorize;
use flowforge_core::{AgentSessionStatus, FlowForgeConfig, Result};
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

    println!(
        "{:<10} {:<20} {:<6} {:<6} Status",
        "ID", "Started", "Edits", "Cmds"
    );
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
        println!(
            "Avg edits/session:    {:.1}",
            total_edits as f64 / total_sessions as f64
        );
        println!(
            "Avg commands/session: {:.1}",
            total_commands as f64 / total_sessions as f64
        );
    }

    Ok(())
}

pub fn agents(session_id: Option<&str>) -> Result<()> {
    let db = open_db()?;

    let parent_id = match session_id {
        Some(id) => id.to_string(),
        None => match db.get_current_session()? {
            Some(s) => s.id,
            None => {
                println!("{}", "No active session".yellow());
                return Ok(());
            }
        },
    };

    let agent_sessions = db.get_agent_sessions(&parent_id)?;
    if agent_sessions.is_empty() {
        println!("No agent sessions for this session");
        return Ok(());
    }

    println!(
        "{:<10} {:<14} {:<12} {:<20} {:<10} {:<6} {:<6}",
        "ID", "Agent Type", "Status", "Started", "Duration", "Edits", "Cmds"
    );
    println!("{}", "─".repeat(80));

    for a in &agent_sessions {
        let status_str = match a.status {
            AgentSessionStatus::Active => "active".green().to_string(),
            AgentSessionStatus::Idle => "idle".yellow().to_string(),
            AgentSessionStatus::Completed => "completed".dimmed().to_string(),
            AgentSessionStatus::Error => "error".red().to_string(),
        };

        let duration = if let Some(end) = a.ended_at {
            let secs = (end - a.started_at).num_seconds();
            format!("{}s", secs)
        } else {
            let secs = (chrono::Utc::now() - a.started_at).num_seconds();
            format!("{}s+", secs)
        };

        let id_short = if a.id.len() >= 8 { &a.id[..8] } else { &a.id };

        println!(
            "{:<10} {:<14} {:<12} {:<20} {:<10} {:<6} {:<6}",
            id_short,
            a.agent_type,
            status_str,
            a.started_at.format("%Y-%m-%d %H:%M"),
            duration,
            a.edits,
            a.commands,
        );
    }
    Ok(())
}
