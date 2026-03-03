mod commands;
mod hooks;

use clap::{Parser, Subcommand};
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(name = "flowforge", about = "Agent orchestration for Claude Code", version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize FlowForge in the current project
    Init {
        /// Initialize for the current project
        #[arg(long)]
        project: bool,
        /// Also set up global config
        #[arg(long)]
        global: bool,
    },
    /// Handle Claude Code hooks
    Hook {
        #[command(subcommand)]
        event: HookEvent,
    },
    /// Show FlowForge status
    Status,
    /// Memory operations
    Memory {
        #[command(subcommand)]
        action: MemoryAction,
    },
    /// Session management
    Session {
        #[command(subcommand)]
        action: SessionAction,
    },
    /// Pattern learning operations
    Learn {
        #[command(subcommand)]
        action: LearnAction,
    },
    /// Agent management
    Agent {
        #[command(subcommand)]
        action: AgentAction,
    },
    /// Route a task to the best agent
    Route {
        /// Task description to route
        task: String,
    },
    /// tmux monitor management
    Tmux {
        #[command(subcommand)]
        action: TmuxAction,
    },
    /// Start the MCP server
    Mcp {
        #[command(subcommand)]
        action: McpAction,
    },
}

#[derive(Subcommand)]
enum HookEvent {
    PreToolUse,
    PostToolUse,
    UserPromptSubmit,
    SessionStart,
    SessionEnd,
    Stop,
    PreCompact,
    SubagentStart,
    SubagentStop,
    TeammateIdle,
    TaskCompleted,
}

#[derive(Subcommand)]
enum MemoryAction {
    /// Get a value by key
    Get { key: String, #[arg(long, default_value = "default")] namespace: String },
    /// Set a key-value pair
    Set { key: String, value: String, #[arg(long, default_value = "default")] namespace: String },
    /// Delete a key
    Delete { key: String, #[arg(long, default_value = "default")] namespace: String },
    /// List keys in a namespace
    List { #[arg(long, default_value = "default")] namespace: String },
    /// Search memory by query
    Search { query: String, #[arg(long, default_value_t = 5)] limit: usize },
}

#[derive(Subcommand)]
enum SessionAction {
    /// Show current session info
    Current,
    /// List recent sessions
    List { #[arg(long, default_value_t = 10)] limit: usize },
    /// Show session metrics
    Metrics,
}

#[derive(Subcommand)]
enum LearnAction {
    /// Store a new pattern
    Store { content: String, #[arg(long, default_value = "general")] category: String },
    /// Search patterns
    Search { query: String, #[arg(long, default_value_t = 5)] limit: usize },
    /// Show learning statistics
    Stats,
}

#[derive(Subcommand)]
enum AgentAction {
    /// List all loaded agents
    List,
    /// Show info about a specific agent
    Info { name: String },
    /// Search for agents
    Search { query: String },
}

#[derive(Subcommand)]
enum TmuxAction {
    /// Start the tmux monitor
    Start,
    /// Update the tmux display
    Update,
    /// Stop the tmux monitor
    Stop,
    /// Show current tmux state
    Status,
}

#[derive(Subcommand)]
enum McpAction {
    /// Start the MCP server (JSON-RPC over stdio)
    Serve,
}

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("warn")),
        )
        .with_writer(std::io::stderr)
        .init();

    let cli = Cli::parse();

    let result = match cli.command {
        Commands::Init { project, global } => commands::init::run(project, global),
        Commands::Hook { event } => match event {
            HookEvent::PreToolUse => hooks::pre_tool_use::run(),
            HookEvent::PostToolUse => hooks::post_tool_use::run(),
            HookEvent::UserPromptSubmit => hooks::user_prompt_submit::run(),
            HookEvent::SessionStart => hooks::session_start::run(),
            HookEvent::SessionEnd => hooks::session_end::run(),
            HookEvent::Stop => hooks::stop::run(),
            HookEvent::PreCompact => hooks::pre_compact::run(),
            HookEvent::SubagentStart => hooks::subagent_start::run(),
            HookEvent::SubagentStop => hooks::subagent_stop::run(),
            HookEvent::TeammateIdle => hooks::teammate_idle::run(),
            HookEvent::TaskCompleted => hooks::task_completed::run(),
        },
        Commands::Status => commands::status::run(),
        Commands::Memory { action } => match action {
            MemoryAction::Get { key, namespace } => commands::memory::get(&key, &namespace),
            MemoryAction::Set { key, value, namespace } => commands::memory::set(&key, &value, &namespace),
            MemoryAction::Delete { key, namespace } => commands::memory::delete(&key, &namespace),
            MemoryAction::List { namespace } => commands::memory::list(&namespace),
            MemoryAction::Search { query, limit } => commands::memory::search(&query, limit),
        },
        Commands::Session { action } => match action {
            SessionAction::Current => commands::session::current(),
            SessionAction::List { limit } => commands::session::list(limit),
            SessionAction::Metrics => commands::session::metrics(),
        },
        Commands::Learn { action } => match action {
            LearnAction::Store { content, category } => commands::learn::store(&content, &category),
            LearnAction::Search { query, limit } => commands::learn::search(&query, limit),
            LearnAction::Stats => commands::learn::stats(),
        },
        Commands::Agent { action } => match action {
            AgentAction::List => commands::agent::list(),
            AgentAction::Info { name } => commands::agent::info(&name),
            AgentAction::Search { query } => commands::agent::search(&query),
        },
        Commands::Route { task } => commands::route::run(&task),
        Commands::Tmux { action } => match action {
            TmuxAction::Start => commands::tmux::start(),
            TmuxAction::Update => commands::tmux::update(),
            TmuxAction::Stop => commands::tmux::stop(),
            TmuxAction::Status => commands::tmux::status(),
        },
        Commands::Mcp { action } => match action {
            McpAction::Serve => commands::mcp::serve(),
        },
    };

    if let Err(e) = result {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}
