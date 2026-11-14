//! What Didia switches off. Flag names come from Dia's own feature-flag list
//! (strings in the Dia program, see MECHANICS.md); each is set to "false" through
//! Dia's "transientFeatureFlagOverrides" setting.

/// Dia's AI: Chat, Ask on page, skills, agents, Morning Brief, memory, AI upsells.
pub const AI_FLAGS: &[&str] = &[
    "chat-enabled",
    "chat-history-panel-enabled",
    "chat-response-modes-enabled",
    "chat-save-as-pdf-enabled",
    "chats-group-enabled",
    "ask-in-page-enabled",
    "ask-on-page-chat-bar-single-app-upsell-enabled",
    "app-skills-enabled",
    "hero-use-case-skills-enabled",
    "skills-v3-enabled",
    "reasoning-skill-enabled",
    "skills-formatter-prompt-enabled",
    "clia-agents-chat-enabled",
    "clia-subagents-enabled",
    "clia-auto-activate-enabled",
    "clia-thinking-enabled",
    "clia-in-chat-upsell-enabled",
    "clia-in-app-upsell-enabled",
    "clia-command-bar-upsell-enabled",
    "clia-chatgpt-ga-enabled",
    "clia-claude-ga-enabled",
    "clia-enable-chatgpt",
    "clia-enable-chatgpt-tools",
    "clia-enable-chatgpt-upsell",
    "clia-artifact-in-chat-presentation-enabled",
    "clia-morning-brief-prototype-enabled",
    "clia-morning-brief-settings-enabled",
    "clia-morning-brief-connect-apps-enabled",
    "episode-recording-enabled",
    "episode-recording-home-enabled",
    "episode-recording-morning-brief-enabled",
    "context-builder-enabled",
    "context-bundles-enabled",
    "context-bundles-aop-upsell-enabled",
    "browse-for-me-enabled",
    "browse-for-me-pip-enabled",
    "browser-use-query-submission-enabled",
    "task-browser-use-enabled",
    "task-experience-enabled",
    "task-notepad-enabled",
    "workboard-enabled",
    "workboard-apps-enabled",
    "workboard-today-enabled",
    "memory-store-suggestions-enabled",
    "per-chat-personalization-toggle-enabled",
    "artifact-sharing-enabled",
    "artifact-generation-better-answers-enabled",
    "sync-chat-companion-enabled",
    "enable-remote-agents",
    "codex-developer-integration-enabled",
    "tab-group-auto-rename-enabled",
    "clean-tabs-enabled",
    "clean-tabs-upsell-enabled",
    "slack-proactive-enabled",
    "meet-pip-chat-button-enabled",
    "dia-usage-pricing-enabled",
    "trial-guide-enabled",
    "trial-guide-stamp-card-on-new-tab-page-enabled",
    "usage-receipt-on-trial-end-enabled",
    "new-tab-page-connect-apps-upsell-enabled",
    "command-bar-apps-upsell-enabled",
    "tool-tour-upsell-on-aop-enabled",
    "google-chat-upsell-enabled",
    "salesforce-upsell-enabled",
    "onboarding-welcome-postcard-enabled",
    "onboarding-dia-tour-aop-cta-enabled",
];

/// Atlassian: Atlassian sign-in and sign-up, Jira/Confluence app connections, Atlassian tools.
pub const ATLASSIAN_FLAGS: &[&str] = &[
    "atlassian-first-party-experience-enabled",
    "atlassian-identity-signup-enabled",
    "atlassian-mcp-v2-enabled",
    "atlassian-session-cookie-exchange-enabled",
    "atlassian-session-cookie-expiry-refresh-enabled",
    "atlassian-sites-accessible-products-enabled",
    "atlassian-custom-templates-enabled",
    "atlassian-tool-call-animated-icon-enabled",
    "onboarding-atlassian-step-enabled",
];

/// Dia's paid-plan prompts: usage pricing, trials and upsells. Off in Didia (with the AI) and
/// in Clia when it runs on your own Claude subscription.
pub const PLAN_FLAGS: &[&str] = &[
    "dia-usage-pricing-enabled",
    "dia-session-cost-enabled",
    "dia-trial-countdown-from-start-enabled",
    "trial-guide-enabled",
    "trial-guide-stamp-card-on-new-tab-page-enabled",
    "usage-receipt-on-trial-end-enabled",
    "clia-in-chat-upsell-enabled",
    "clia-in-app-upsell-enabled",
    "clia-command-bar-upsell-enabled",
    "clia-enable-chatgpt-upsell",
    "post-subscription-survey-enabled",
    "upsell-dialog-rebrand-enabled",
];

/// Backup: buttons still shown after the flags are off are hidden by their label or kind.
pub struct HideRules {
    pub labels: Vec<&'static str>,
    pub label_prefixes: Vec<&'static str>,
    pub label_containing: Vec<&'static str>,
    pub view_classes: Vec<&'static str>,
    pub menu_items: Vec<&'static str>,
}

pub fn hide_rules(ai: bool, atlassian: bool) -> HideRules {
    let mut r = HideRules {
        labels: vec![],
        label_prefixes: vec![],
        label_containing: vec![],
        view_classes: vec![],
        menu_items: vec![],
    };
    if ai {
        // Labels of Dia's AI buttons (accessibility labels seen in Dia 1.57).
        r.labels.extend(["Chat", "Delete chat"]);
        r.label_prefixes.extend(["Chat with this tab"]);
        r.label_containing.extend(["when you have chat usage", "Personalization on for new chats"]);
        r.view_classes.extend([
            "AssistantCapabilitiesToolbarView",
            "AssistantPersonalityUI.PersonalizeButton",
            "NavigationBar.AssistantBarView",
            "MorningBriefTeaser",
        ]);
        r.menu_items.extend(["Morning Brief", "Skills", "New Chat", "Ask on Page"]);
    }
    if atlassian {
        r.label_containing.extend(["Atlassian"]);
        r.view_classes.extend(["AtlassianSeal", "AtlassianFirstParty"]);
    }
    r
}

pub fn flag_overrides(ai: bool, atlassian: bool, plan: bool) -> String {
    let mut names: Vec<&str> = vec![];
    if ai {
        names.extend(AI_FLAGS);
    }
    if atlassian {
        names.extend(ATLASSIAN_FLAGS);
    }
    if plan {
        names.extend(PLAN_FLAGS);
    }
    let mut seen = std::collections::HashSet::new();
    let mut parts: Vec<String> = names.into_iter().filter(|f| seen.insert(*f)).map(|f| format!("{f}=false")).collect();
    if atlassian {
        parts.push("atlassian-identity-signup-rollout-percentage=0".into());
    }
    parts.join("|")
}

/// DidiaPatch.plist, read by the patch inside the app when it starts.
pub fn patch_config(s: &crate::keeper::Settings) -> plist::Value {
    use plist::{Dictionary, Value};
    let (ai, atlassian, plan) = s.switches();
    let arr = |v: &[&str]| Value::Array(v.iter().map(|s| Value::String(s.to_string())).collect());
    let rules = hide_rules(ai, atlassian);
    let mut defaults = Dictionary::new();
    if ai {
        // Dia reads page text for its AI; off when AI is removed.
        defaults.insert("enablePageContentExtraction".into(), Value::Boolean(false));
    }
    let own_claude = s.edition == crate::keeper::Edition::Clia && s.own_claude;
    let mut d = Dictionary::new();
    d.insert("edition".into(), Value::String(s.edition.name().into()));
    d.insert("flagOverrides".into(), Value::String(flag_overrides(ai, atlassian, plan)));
    d.insert("defaults".into(), Value::Dictionary(defaults));
    d.insert("hideLabels".into(), arr(&rules.labels));
    d.insert("hideLabelPrefixes".into(), arr(&rules.label_prefixes));
    d.insert("hideLabelContaining".into(), arr(&rules.label_containing));
    d.insert("hideViewClasses".into(), arr(&rules.view_classes));
    d.insert("hideMenuItems".into(), arr(&rules.menu_items));
    d.insert("appleScriptJavaScript".into(), Value::Boolean(s.applescript_js));
    d.insert("removeAI".into(), Value::Boolean(ai));
    d.insert("removeAtlassian".into(), Value::Boolean(atlassian));
    d.insert("ownClaude".into(), Value::Boolean(own_claude));
    Value::Dictionary(d)
}
