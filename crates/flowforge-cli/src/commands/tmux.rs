use flowforge_core::{FlowForgeConfig, Result, TmuxState};
use flowforge_tmux::{TmuxManager, TmuxStateManager};
use flowforge_tmux::display::render_display;
use colored::Colorize;

pub fn start() -> Result<()> {
    let config = FlowForgeConfig::load(&FlowForgeConfig::config_path())?;
    let manager = TmuxManager::new(&config.tmux.session_name);

    if !manager.is_available() {
        eprintln!("{}: tmux is not installed or not in PATH", "Error".red());
        std::process::exit(1);
    }

    if manager.session_exists() {
        println!("tmux session '{}' already running", config.tmux.session_name);
        return Ok(());
    }

    let state_mgr = TmuxStateManager::new(FlowForgeConfig::tmux_state_path());
    let state = state_mgr.load().unwrap_or_else(|_| TmuxState {
        session_name: config.tmux.session_name.clone(),
        team_name: None,
        members: Vec::new(),
        recent_events: Vec::new(),
        memory_count: 0,
        pattern_count: 0,
        updated_at: chrono::Utc::now(),
    });

    manager.start(&state)?;
    println!("{} tmux monitor started (session: {})", "✓".green(), config.tmux.session_name);
    println!("Attach with: tmux attach -t {}", config.tmux.session_name);

    Ok(())
}

pub fn update() -> Result<()> {
    let config = FlowForgeConfig::load(&FlowForgeConfig::config_path())?;
    let manager = TmuxManager::new(&config.tmux.session_name);
    let state_mgr = TmuxStateManager::new(FlowForgeConfig::tmux_state_path());

    let state = state_mgr.load()?;
    manager.update(&state)?;

    Ok(())
}

pub fn stop() -> Result<()> {
    let config = FlowForgeConfig::load(&FlowForgeConfig::config_path())?;
    let manager = TmuxManager::new(&config.tmux.session_name);

    if manager.session_exists() {
        manager.stop()?;
        println!("{} tmux monitor stopped", "✓".green());
    } else {
        println!("No active tmux session found");
    }

    Ok(())
}

pub fn status() -> Result<()> {
    let state_mgr = TmuxStateManager::new(FlowForgeConfig::tmux_state_path());
    match state_mgr.load() {
        Ok(state) => {
            println!("{}", render_display(&state));
        }
        Err(_) => {
            println!("No tmux state found. Run `flowforge tmux start` first.");
        }
    }
    Ok(())
}
