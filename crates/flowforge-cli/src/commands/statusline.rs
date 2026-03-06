use colored::Colorize;
use flowforge_core::{AgentSessionStatus, FlowForgeConfig, Result};
use flowforge_memory::MemoryDb;

const SEP: &str = "\u{2502}"; // │

#[allow(clippy::print_literal, clippy::format_in_format_args)]
pub fn run() -> Result<()> {
    // Read stdin (Claude Code pipes JSON context)
    let stdin_data: serde_json::Value = {
        let mut buf = String::new();
        match std::io::Read::read_to_string(&mut std::io::stdin(), &mut buf) {
            Ok(_) if !buf.trim().is_empty() => {
                serde_json::from_str(&buf).unwrap_or(serde_json::json!({}))
            }
            _ => serde_json::json!({}),
        }
    };

    let model = stdin_data
        .get("model")
        .and_then(|v| {
            // Handle both string and object formats
            v.as_str().or_else(|| {
                v.get("display_name")
                    .and_then(|d| d.as_str())
                    .or_else(|| v.get("id").and_then(|id| id.as_str()))
            })
        })
        .unwrap_or("");

    let project_name = std::env::current_dir()
        .ok()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
        .unwrap_or_else(|| "unknown".to_string());

    let ctx_remaining: Option<u32> = stdin_data
        .get("context_window")
        .and_then(|cw| cw.get("remaining_percentage"))
        .and_then(|v| v.as_f64())
        .map(|f| f as u32);

    let session_cost: Option<f64> = stdin_data
        .get("cost")
        .and_then(|c| c.get("total_cost_usd"))
        .and_then(|v| v.as_f64());

    let session_name: Option<&str> = stdin_data
        .get("session_name")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty());

    let config = FlowForgeConfig::load(&FlowForgeConfig::config_path()).ok();
    let db = config
        .as_ref()
        .and_then(|c| MemoryDb::open(&c.db_path()).ok());

    // Get git info
    let git_branch = std::process::Command::new("git")
        .args(["branch", "--show-current", "--no-color"])
        .output()
        .ok()
        .and_then(|o| {
            if o.status.success() {
                Some(String::from_utf8_lossy(&o.stdout).trim().to_string())
            } else {
                None
            }
        })
        .filter(|s| !s.is_empty());

    let git_status = std::process::Command::new("git")
        .args(["status", "--porcelain", "--no-renames"])
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
        .unwrap_or_default();
    let (staged, modified, untracked) = parse_git_porcelain(&git_status);

    let mut lines = Vec::new();

    // ══════════════════════════════════════════════════════════════
    // LINE 1: Header — project + git + model + context + duration
    // ══════════════════════════════════════════════════════════════
    let display_name = session_name.unwrap_or(&project_name);
    let icon = "⚡".bright_yellow().to_string();
    let mut header_parts: Vec<String> =
        vec![format!("{}{}", icon, display_name.bold().bright_magenta())];

    // Git branch + changes
    if let Some(ref branch) = git_branch {
        let mut git_str = branch.bright_blue().to_string();
        let mut changes = Vec::new();
        if staged > 0 {
            changes.push(format!("+{}", staged).bright_green().to_string());
        }
        if modified > 0 {
            changes.push(format!("~{}", modified).bright_yellow().to_string());
        }
        if untracked > 0 {
            changes.push(format!("?{}", untracked).dimmed().to_string());
        }
        if !changes.is_empty() {
            git_str = format!("{} {}", git_str, changes.join(""));
        }
        header_parts.push(git_str);
    }

    // Model
    if !model.is_empty() {
        header_parts.push(shorten_model(model).bright_magenta().to_string());
    }

    // Context remaining
    if let Some(remaining) = ctx_remaining {
        let ctx_used = 100u32.saturating_sub(remaining);
        let ctx_str = format!("ctx {}%", ctx_used);
        let colored = if ctx_used < 50 {
            ctx_str.bright_green()
        } else if ctx_used < 70 {
            ctx_str.bright_cyan()
        } else if ctx_used < 85 {
            ctx_str.bright_yellow()
        } else {
            ctx_str.bright_red()
        };
        header_parts.push(colored.to_string());
    }

    // Session duration (always shown, -- when no session)
    let current_session = db.as_ref().and_then(|d| d.get_current_session().ok().flatten());

    // When the current session is brand new (0 commands), fall back to previous session
    // for metrics like trust, hooks, errors so the statusline doesn't show all dashes.
    let prev_session = if current_session.as_ref().map(|s| s.commands).unwrap_or(0) == 0 {
        db.as_ref().and_then(|d| {
            d.list_sessions(2).ok().and_then(|sessions| {
                sessions.into_iter().find(|s| {
                    Some(&s.id) != current_session.as_ref().map(|c| &c.id)
                })
            })
        })
    } else {
        None
    };
    // The session to use for metrics: current if it has data, else previous
    let metrics_session = if current_session.as_ref().map(|s| s.commands).unwrap_or(0) > 0 {
        current_session.as_ref()
    } else {
        prev_session.as_ref().or(current_session.as_ref())
    };
    if let Some(ref session) = current_session {
        let secs = chrono::Utc::now()
            .signed_duration_since(session.started_at)
            .num_seconds();
        header_parts.push(format_duration(secs).cyan().to_string());
    } else {
        header_parts.push("--".dimmed().to_string());
    }

    // Session cost
    if let Some(cost) = session_cost {
        let cost_str = format!("${:.2}", cost);
        let colored = if cost < 1.0 {
            cost_str.green()
        } else if cost < 5.0 {
            cost_str.yellow()
        } else {
            cost_str.bright_red()
        };
        header_parts.push(colored.to_string());
    }

    let sep = format!("  {}  ", SEP.dimmed());
    lines.push(header_parts.join(&sep));

    // ══════════════════════════════════════════════════════════════
    // LINE 2: Intelligence + Session metrics (always shown)
    // ══════════════════════════════════════════════════════════════
    {
        let mut parts: Vec<String> = Vec::new();

        // Patterns: short+long
        let short = db.as_ref().and_then(|d| d.count_patterns_short().ok()).unwrap_or(0);
        let long = db.as_ref().and_then(|d| d.count_patterns_long().ok()).unwrap_or(0);
        let total_patterns = short + long;
        let pat_count = if total_patterns > 50 {
            format!("{}", total_patterns).bright_green().to_string()
        } else if total_patterns > 0 {
            format!("{}", total_patterns).yellow().to_string()
        } else {
            "0".dimmed().to_string()
        };
        let proven_tag = if long > 0 {
            format!(" {}", format!("{}⚡", long).bright_cyan())
        } else {
            String::new()
        };
        parts.push(format!("{}{} {}", pat_count, proven_tag, "pat".dimmed()));

        // Vectors + HNSW cluster coverage
        let vec_count = db.as_ref().and_then(|d| d.count_vectors().ok()).unwrap_or(0);
        let outliers = db.as_ref().and_then(|d| d.count_outlier_vectors().ok()).unwrap_or(0);
        let clustered = vec_count - outliers;
        let cluster_pct = if vec_count > 0 {
            (clustered as f64 / vec_count as f64 * 100.0) as u32
        } else {
            0
        };
        let vec_str = if vec_count > 0 {
            format!("{}", vec_count).bright_cyan().to_string()
        } else {
            "0".dimmed().to_string()
        };
        let hnsw_tag = if vec_count > 0 {
            let pct_str = format!("{}%", cluster_pct);
            let colored_pct = if cluster_pct >= 50 {
                pct_str.green().to_string()
            } else if cluster_pct >= 20 {
                pct_str.yellow().to_string()
            } else {
                pct_str.dimmed().to_string()
            };
            format!(" {}{}", "\u{29bf}".dimmed(), colored_pct)
        } else {
            String::new()
        };
        parts.push(format!("{}{} {}", vec_str, hnsw_tag, "vec".dimmed()));

        // Trajectory success rate
        let traj_rate = db.as_ref().and_then(|d| d.recent_trajectory_success_rate(20).ok()).unwrap_or(0.0);
        let traj_pct = (traj_rate * 100.0) as u32;
        let traj_str = format!("traj {}%", traj_pct);
        parts.push(color_by_ratio(traj_rate, &traj_str));

        // Routing accuracy (always shown, -- when <3 data points)
        let (routing_hits, routing_total) = db.as_ref().and_then(|d| d.routing_accuracy_stats().ok()).unwrap_or((0, 0));
        if routing_total > 2 {
            let route_rate = routing_hits as f64 / routing_total as f64;
            let route_pct = (route_rate * 100.0) as u32;
            let route_str = format!("route {}%", route_pct);
            parts.push(color_by_ratio(route_rate, &route_str));
        } else {
            parts.push(format!("{} {}", "route".dimmed(), "--%".dimmed()));
        }

        // Trust score (always shown, falls back to previous session)
        if let Some(ref session) = metrics_session {
            if let Some(ref d) = db {
                if let Ok(Some(trust)) = d.get_trust_score(&session.id) {
                    let trust_pct = (trust.score * 100.0) as u32;
                    let trust_str = format!("trust {}%", trust_pct);
                    let mut detail = color_by_trust(trust.score, &trust_str);
                    if trust.denials > 0 {
                        detail = format!("{} {}", detail, format!("{}deny", trust.denials).red());
                    }
                    parts.push(detail);
                } else {
                    parts.push(format!("{} {}", "trust".dimmed(), "--%".dimmed()));
                }
            } else {
                parts.push(format!("{} {}", "trust".dimmed(), "--%".dimmed()));
            }
        } else {
            parts.push(format!("{} {}", "trust".dimmed(), "--%".dimmed()));
        }

        // Session error count (always shown, falls back to previous session)
        let errs = match (&db, &metrics_session) {
            (Some(d), Some(s)) => d.count_session_failures(&s.id).unwrap_or(0),
            _ => 0,
        };
        let err_str = format!("{} {}", errs, "errs".dimmed());
        if errs == 0 {
            parts.push(err_str.green().to_string());
        } else {
            parts.push(err_str.red().to_string());
        }

        // Hook health (always shown, falls back to previous session)
        let hook_health = match (&db, &metrics_session) {
            (Some(d), Some(s)) => compute_hook_health(d, &s.id),
            _ => HookHealth { total: count_configured_hooks(), ok: 0 },
        };
        let h_str = format!("{}/{} {}", hook_health.ok, hook_health.total, "hooks".dimmed());
        if hook_health.total == 0 {
            parts.push(h_str.dimmed().to_string());
        } else if hook_health.ok == hook_health.total {
            parts.push(h_str.green().to_string());
        } else if hook_health.ok as f64 / hook_health.total as f64 >= 0.8 {
            parts.push(h_str.yellow().to_string());
        } else {
            parts.push(h_str.red().to_string());
        }

        // Session activity (always shown, falls back to previous session)
        let (edits, cmds) = metrics_session
            .map(|s| (s.edits, s.commands))
            .unwrap_or((0, 0));
        let edits_str = if edits > 0 {
            format!("{} edits", edits)
        } else {
            format!("{} {}", "0".dimmed(), "edits".dimmed())
        };
        let cmds_str = if cmds > 0 {
            format!("{} cmds", cmds)
        } else {
            format!("{} {}", "0".dimmed(), "cmds".dimmed())
        };
        parts.push(format!("{}  {}", edits_str, cmds_str));

        lines.push(parts.join(&sep));
    }

    // ══════════════════════════════════════════════════════════════
    // LINE 3: Work + Agents + Warnings (always shown)
    // ══════════════════════════════════════════════════════════════
    {
        let mut line3_parts: Vec<String> = Vec::new();

        // Work items (always shown: active, pending, done)
        let (wip, pending, done) = if let Some(ref d) = db {
            let wip = d
                .list_work_items(&flowforge_core::WorkFilter {
                    status: Some(flowforge_core::WorkStatus::InProgress),
                    ..Default::default()
                })
                .unwrap_or_default()
                .len();
            let pending = d
                .list_work_items(&flowforge_core::WorkFilter {
                    status: Some(flowforge_core::WorkStatus::Pending),
                    ..Default::default()
                })
                .unwrap_or_default()
                .len();
            let done = d
                .list_work_items(&flowforge_core::WorkFilter {
                    status: Some(flowforge_core::WorkStatus::Completed),
                    ..Default::default()
                })
                .unwrap_or_default()
                .len();
            (wip, pending, done)
        } else {
            (0, 0, 0)
        };

        let active_str = if wip > 0 {
            format!("{} {}", wip, "active".dimmed()).bright_yellow().to_string()
        } else {
            format!("{} {}", "0".dimmed(), "active".dimmed())
        };
        let pending_str = if pending > 0 {
            format!("{} {}", pending, "pending".dimmed()).bright_blue().to_string()
        } else {
            format!("{} {}", "0".dimmed(), "pending".dimmed())
        };
        let done_str = if done > 0 {
            format!("{} {}", done, "done".dimmed()).green().to_string()
        } else {
            format!("{} {}", "0".dimmed(), "done".dimmed())
        };
        line3_parts.push(format!("{}  {}  {}", active_str, pending_str, done_str));

        // Agents (always shown)
        let current_session_id = current_session.as_ref().map(|s| s.id.clone());
        let agents = if let Some(ref d) = db {
            get_agent_summary(d, current_session_id.as_deref())
        } else {
            AgentSummary { active: 0, idle: 0, total_spawned: 0, names: Vec::new() }
        };
        let live = agents.active + agents.idle;
        let agent_str = if live > 0 {
            let count = format!("{}/{} agents", live, agents.total_spawned)
                .bright_green()
                .to_string();
            if !agents.names.is_empty() {
                format!("{} ({})", count, agents.names.join(" "))
            } else {
                count
            }
        } else if agents.total_spawned > 0 {
            format!("{}/{} {}", "0".dimmed(), agents.total_spawned, "agents".dimmed())
        } else {
            format!("{} {}", "0/0".dimmed(), "agents".dimmed())
        };
        line3_parts.push(agent_str);

        // Unread mail (always shown)
        let unread_count = match (&db, &current_session_id) {
            (Some(d), Some(sid)) => d.get_unread_messages(sid).map(|u| u.len()).unwrap_or(0),
            _ => 0,
        };
        let mail_str = if unread_count > 0 {
            format!("{} {}", unread_count, "mail".dimmed()).bright_yellow().to_string()
        } else {
            format!("{} {}", "0".dimmed(), "mail".dimmed())
        };
        line3_parts.push(mail_str);

        // Warnings / clean status
        let mut warn_parts = Vec::new();
        if let Some(ref d) = db {
            if let Ok(stealable) = d.get_stealable_items(5) {
                if !stealable.is_empty() {
                    warn_parts.push(format!("{} stale", stealable.len()).yellow().to_string());
                }
            }
        }
        let log_path = FlowForgeConfig::project_dir().join("hook-errors.log");
        if log_path.exists() {
            if let Ok(content) = std::fs::read_to_string(&log_path) {
                let err_count = content.lines().filter(|l| !l.trim().is_empty()).count();
                if err_count > 0 {
                    warn_parts.push(format!("{} hook-err", err_count).red().to_string());
                }
            }
        }
        if warn_parts.is_empty() {
            line3_parts.push("\u{2713} clean".green().to_string());
        } else {
            line3_parts.push(format!("!! {}", warn_parts.join(" ")));
        }

        lines.push(line3_parts.join(&sep));
    }

    // Print multi-line dashboard
    println!("{}", lines.join("\n"));

    Ok(())
}

/// Print the legend explaining all statusline symbols.
#[allow(clippy::print_literal)]
pub fn print_legend() -> Result<()> {
    println!("{}", "FlowForge Dashboard Legend".bold().cyan());
    println!();

    println!("{}", "LINE 1: IDENTITY + ENVIRONMENT".bold());
    println!("  {}flowforge   Icon prefix + project or session name", "⚡".bright_yellow());
    println!("  branch       Git branch with +staged ~modified ?untracked");
    println!("  op4.6        Model name (Opus 4.6, Sonnet 4.6, etc.)");
    println!("  ctx 23%      Context window usage (green<50 cyan<70 yellow<85 red)");
    println!("  5m / --      Session duration (-- when no session)");
    println!("  $1.23        Session cost (green<$1 yellow<$5 red)");
    println!();

    println!("{}", "LINE 2: INTELLIGENCE + SESSION METRICS".bold());
    println!("  N [M{}] pat  Total patterns (short+long), M{}=promoted/proven", "⚡".bright_cyan(), "⚡".bright_cyan());
    println!("  N [P%] vec   HNSW vectors, P%=cluster coverage (higher=better organized)");
    println!("  traj N%      Trajectory success rate (last 20 judged)");
    println!("  route N%     Routing accuracy (--% when <3 data points)");
    println!("  trust N%     Guidance trust score (--% when no session)");
    println!("  N errs       Distinct tool failures this session");
    println!("  N/M hooks    Hooks working / total configured");
    println!("  N edits      File edits this session");
    println!("  N cmds       Commands run this session");
    println!();

    println!("{}", "LINE 3: WORK + AGENTS".bold());
    println!("  N active     In-progress work items");
    println!("  N pending    Pending work items");
    println!("  N done       Completed work items");
    println!("  N/M agents   Live / total spawned agents");
    println!("  N mail       Unread co-agent messages");
    println!("  {} clean     No warnings (or !! stale / hook-err)", "\u{2713}".green());
    println!();

    println!("{}", "All fields always visible. Dimmed = zero/unavailable.".dimmed());

    Ok(())
}

// ── Helpers ──

fn format_duration(secs: i64) -> String {
    if secs < 60 {
        format!("{}s", secs)
    } else if secs < 3600 {
        format!("{}m", secs / 60)
    } else {
        format!("{}h{}m", secs / 3600, (secs % 3600) / 60)
    }
}

fn color_by_ratio(ratio: f64, text: &str) -> String {
    if ratio >= 0.9 {
        text.green().to_string()
    } else if ratio >= 0.7 {
        text.yellow().to_string()
    } else {
        text.red().to_string()
    }
}

fn color_by_trust(score: f64, text: &str) -> String {
    if score >= 0.8 {
        text.green().to_string()
    } else if score >= 0.5 {
        text.yellow().to_string()
    } else {
        text.red().to_string()
    }
}

/// Parse git status --porcelain output into (staged, modified, untracked) counts
fn parse_git_porcelain(output: &str) -> (u32, u32, u32) {
    let (mut staged, mut modified, mut untracked) = (0, 0, 0);
    for line in output.lines() {
        if line.len() < 2 {
            continue;
        }
        let bytes = line.as_bytes();
        let x = bytes[0] as char;
        let y = bytes[1] as char;
        if x == '?' && y == '?' {
            untracked += 1;
        } else {
            if x != ' ' && x != '?' {
                staged += 1;
            }
            if y != ' ' && y != '?' {
                modified += 1;
            }
        }
    }
    (staged, modified, untracked)
}

struct AgentSummary {
    active: usize,
    idle: usize,
    total_spawned: usize,
    names: Vec<String>,
}

/// Get agent summary for the session.
/// When parent_session_id is provided, shows agents from that session and
/// their sub-agents (team lead children) via recursive query.
fn get_agent_summary(db: &MemoryDb, parent_session_id: Option<&str>) -> AgentSummary {
    // Clean up orphaned agent sessions before display
    let _ = db.cleanup_orphaned_agent_sessions();

    let all_agents = if let Some(sid) = parent_session_id {
        db.get_agent_sessions_recursive(sid).unwrap_or_default()
    } else {
        db.get_active_agent_sessions().unwrap_or_default()
    };

    let total_spawned = all_agents.len();
    let live: Vec<_> = all_agents
        .into_iter()
        .filter(|a| a.ended_at.is_none())
        .collect();

    let active: Vec<_> = live
        .iter()
        .filter(|a| a.status == AgentSessionStatus::Active)
        .collect();
    let idle: Vec<_> = live
        .iter()
        .filter(|a| a.status == AgentSessionStatus::Idle)
        .collect();

    let mut names = Vec::new();
    for a in &active {
        names.push(shorten_agent_name(&a.agent_type).bold().green().to_string());
    }
    for a in &idle {
        names.push(shorten_agent_name(&a.agent_type).dimmed().to_string());
    }

    AgentSummary {
        active: active.len(),
        idle: idle.len(),
        total_spawned,
        names,
    }
}

/// Shorten agent type names for compact display
fn shorten_model(model: &str) -> String {
    // Normalize: lowercase, replace spaces with hyphens for uniform matching
    let m = model.to_lowercase().replace(' ', "-");
    match m.as_str() {
        s if s.contains("opus-4.6") || s.contains("opus-4-6") => "op4.6".to_string(),
        s if s.contains("sonnet-4.6") || s.contains("sonnet-4-6") => "sn4.6".to_string(),
        s if s.contains("haiku-4.5") || s.contains("haiku-4-5") => "hk4.5".to_string(),
        s if s.contains("opus-4.5") || s.contains("opus-4-5") => "op4.5".to_string(),
        s if s.contains("sonnet-4.5") || s.contains("sonnet-4-5") => "sn4.5".to_string(),
        s if s.contains("opus-4") => "op4".to_string(),
        s if s.contains("sonnet-4") => "sn4".to_string(),
        s if s.contains("sonnet-3.5") || s.contains("sonnet-3-5") => "sn3.5".to_string(),
        s if s.contains("haiku-3.5") || s.contains("haiku-3-5") => "hk3.5".to_string(),
        _ => model.to_string(),
    }
}

struct HookHealth {
    total: usize, // configured hook event types
    ok: usize,    // called this session with zero errors
}

fn compute_hook_health(db: &MemoryDb, session_id: &str) -> HookHealth {
    // Count configured hooks from settings.json
    let total = count_configured_hooks();

    // Get session metrics to find which hooks have been called and which errored
    let metrics = db.get_session_metrics(session_id).unwrap_or_default();

    let mut called: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut errored: std::collections::HashSet<String> = std::collections::HashSet::new();

    for (name, value) in &metrics {
        if let Some(hook) = name.strip_prefix("hook_calls:") {
            if *value > 0.0 {
                called.insert(hook.to_string());
            }
        }
        if let Some(hook) = name.strip_prefix("hook_errors:") {
            if *value > 0.0 {
                errored.insert(hook.to_string());
            }
        }
    }

    let ok = called.iter().filter(|h| !errored.contains(*h)).count();
    HookHealth { total, ok }
}

fn count_configured_hooks() -> usize {
    let settings_path = FlowForgeConfig::project_dir()
        .parent()
        .unwrap_or(".".as_ref())
        .join(".claude/settings.json");
    let content = match std::fs::read_to_string(&settings_path) {
        Ok(c) => c,
        Err(_) => return 0,
    };
    let val: serde_json::Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(_) => return 0,
    };
    val.get("hooks")
        .and_then(|h| h.as_object())
        .map(|obj| obj.len())
        .unwrap_or(0)
}

fn shorten_agent_name(agent_type: &str) -> String {
    match agent_type {
        "general-purpose" | "general" => "gen".to_string(),
        "Explore" | "explore" => "exp".to_string(),
        "Plan" | "plan" => "pln".to_string(),
        "code-simplifier" => "sim".to_string(),
        "claude-code-guide" => "guide".to_string(),
        "statusline-setup" => "sline".to_string(),
        "test-runner" => "test".to_string(),
        t if t.chars().count() > 6 => t.chars().take(6).collect(),
        t => t.to_string(),
    }
}
