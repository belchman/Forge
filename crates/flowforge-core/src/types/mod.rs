mod agents;
mod collaboration;
mod guidance;
mod patterns;
mod sessions;
mod work;

pub use agents::*;
pub use collaboration::*;
pub use guidance::*;
pub use patterns::*;
pub use sessions::*;
pub use work::*;

#[cfg(test)]
mod tests {
    #[test]
    fn test_all_types_accessible_from_crate_root() {
        // Verify key types are accessible from the crate root
        let _: Option<crate::AgentDef> = None;
        let _: Option<crate::WorkItem> = None;
        let _: Option<crate::GuidanceRule> = None;
        let _: Option<crate::SessionInfo> = None;
        let _: Option<crate::ShortTermPattern> = None;
        let _: Option<crate::ConversationMessage> = None;
        let _: Option<crate::TmuxState> = None;
        let _: Option<crate::RoutingResult> = None;
        let _: Option<crate::PatternCluster> = None;
        let _: Option<crate::WorkFilter> = None;
        let _: Option<crate::MailboxMessage> = None;
        let _: Option<crate::TrustScore> = None;
    }
}
