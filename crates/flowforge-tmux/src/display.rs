use colored::Colorize;
use flowforge_core::{TeamMemberStatus, TmuxState};

pub fn render_display(state: &TmuxState) -> String {
    let width = 50;
    let inner = width - 2;

    let mut lines = Vec::new();

    // Top border
    lines.push(format!("┌{}┐", "─".repeat(inner)));

    // Title
    let title = "FlowForge Team Monitor";
    let team_label = state
        .team_name
        .as_deref()
        .map(|n| format!(" ({})", n))
        .unwrap_or_default();
    let full_title = format!("  {}{}", title, team_label);
    lines.push(format!("│{}│", pad_right(&full_title, inner)));

    // Separator
    lines.push(format!("├{}┤", "─".repeat(inner)));

    // Column headers
    let header = format!(
        " {:<12}│ {:<8}│ {}",
        "Agent", "Status", "Task"
    );
    lines.push(format!("│{}│", pad_right(&header, inner)));

    // Header underline
    let underline = format!(
        " {}│{}│{}",
        "─".repeat(12),
        "─".repeat(9),
        "─".repeat(inner.saturating_sub(23))
    );
    lines.push(format!("│{}│", pad_right(&underline, inner)));

    // Members
    if state.members.is_empty() {
        let empty = "  (no team members)";
        lines.push(format!("│{}│", pad_right(empty, inner)));
    } else {
        for member in &state.members {
            let status_str = match member.status {
                TeamMemberStatus::Active => "active".green().to_string(),
                TeamMemberStatus::Idle => "idle".yellow().to_string(),
                TeamMemberStatus::Completed => "done".blue().to_string(),
                TeamMemberStatus::Error => "error".red().to_string(),
            };

            let task = member.current_task.as_deref().unwrap_or("-");

            let plain_status = match member.status {
                TeamMemberStatus::Active => "active",
                TeamMemberStatus::Idle => "idle",
                TeamMemberStatus::Completed => "done",
                TeamMemberStatus::Error => "error",
            };

            let agent_id = truncate(&member.agent_id, 11);
            let task_display = truncate(task, inner.saturating_sub(24));

            // Build the plain version for padding calculation
            let plain_row = format!(
                " {:<12}│ {:<8}│ {}",
                agent_id, plain_status, task_display
            );

            // Build the colored version for display
            let colored_row = format!(
                " {:<12}│ {:<8}│ {}",
                agent_id, status_str, task_display
            );

            let plain_len = plain_row.chars().count();
            let pad_needed = inner.saturating_sub(plain_len);
            lines.push(format!("│{}{}│", colored_row, " ".repeat(pad_needed)));
        }
    }

    // Separator
    lines.push(format!("├{}┤", "─".repeat(inner)));

    // Recent events (show last 3)
    let events: Vec<&String> = state.recent_events.iter().rev().take(3).collect();
    if events.is_empty() {
        let no_events = " No recent events";
        lines.push(format!("│{}│", pad_right(no_events, inner)));
    } else {
        for event in events {
            let ev = format!(" Recent: {}", truncate(event, inner.saturating_sub(10)));
            lines.push(format!("│{}│", pad_right(&ev, inner)));
        }
    }

    // Stats line
    let stats = format!(
        " Memory: {} entries │ Patterns: {}",
        state.memory_count, state.pattern_count
    );
    lines.push(format!("│{}│", pad_right(&stats, inner)));

    // Bottom border
    lines.push(format!("└{}┘", "─".repeat(inner)));

    lines.join("\n")
}

fn pad_right(s: &str, width: usize) -> String {
    let char_count = s.chars().count();
    if char_count >= width {
        s.chars().take(width).collect()
    } else {
        format!("{}{}", s, " ".repeat(width - char_count))
    }
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.chars().count() <= max_len {
        s.to_string()
    } else if max_len > 2 {
        format!("{}..", s.chars().take(max_len - 2).collect::<String>())
    } else {
        s.chars().take(max_len).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use flowforge_core::TeamMemberState;

    fn make_state() -> TmuxState {
        TmuxState {
            session_name: "flowforge".to_string(),
            team_name: Some("test-team".to_string()),
            members: vec![
                TeamMemberState {
                    agent_id: "team-lead".to_string(),
                    agent_type: "leader".to_string(),
                    status: TeamMemberStatus::Active,
                    current_task: Some("Coordinating".to_string()),
                    updated_at: Utc::now(),
                },
                TeamMemberState {
                    agent_id: "researcher".to_string(),
                    agent_type: "research".to_string(),
                    status: TeamMemberStatus::Idle,
                    current_task: None,
                    updated_at: Utc::now(),
                },
            ],
            recent_events: vec!["researcher completed task-001".to_string()],
            memory_count: 342,
            pattern_count: 89,
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn test_render_contains_title() {
        let output = render_display(&make_state());
        assert!(output.contains("FlowForge Team Monitor"));
    }

    #[test]
    fn test_render_contains_members() {
        let output = render_display(&make_state());
        assert!(output.contains("team-lead"));
        assert!(output.contains("researcher"));
    }

    #[test]
    fn test_render_contains_stats() {
        let output = render_display(&make_state());
        assert!(output.contains("342"));
        assert!(output.contains("89"));
    }

    #[test]
    fn test_render_empty_state() {
        let state = TmuxState {
            session_name: "flowforge".to_string(),
            team_name: None,
            members: Vec::new(),
            recent_events: Vec::new(),
            memory_count: 0,
            pattern_count: 0,
            updated_at: Utc::now(),
        };
        let output = render_display(&state);
        assert!(output.contains("no team members"));
    }
}
