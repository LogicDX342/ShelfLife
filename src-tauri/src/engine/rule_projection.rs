use crate::engine::freshness::classify_decay_state;
use crate::models::{
    AppConfig, AppError, AutomationRule, Expiry, FileDecayState, RuleAction, RuleMatchExplanation,
    RuleMode, TrackedFile,
};
use crate::rules::{CompiledRuleSet, RuleDecision, RuleDecisionScope, RuleVerdict};

pub struct TrackedRuleProjection {
    pub tracked: TrackedFile,
    pub decision: RuleDecision,
}

#[derive(Debug, Clone)]
pub struct AutomaticRuleCandidate {
    pub expires_at: u64,
    pub rule: AutomationRule,
    pub explanation: RuleMatchExplanation,
}

pub fn project_watched_file(
    mut tracked: TrackedFile,
    config: &AppConfig,
    rule_set: &CompiledRuleSet,
    now: u64,
) -> Result<TrackedRuleProjection, AppError> {
    let decision = rule_set.decide_file(&tracked, RuleDecisionScope::WatchedFile);
    tracked.matched_rule_ids = decision.matched_rule_ids.clone();
    apply_rule_decision(&mut tracked, &decision, config, now);

    Ok(TrackedRuleProjection { tracked, decision })
}

pub fn automatic_rule_candidate(
    file: &TrackedFile,
    config: &AppConfig,
    rule_set: &CompiledRuleSet,
) -> Result<Option<AutomaticRuleCandidate>, AppError> {
    let Expiry::At(expires_at) = file.expiry else {
        return Ok(None);
    };
    if matches!(file.state, FileDecayState::Ignored) {
        return Ok(None);
    }

    let projection = project_watched_file(
        file.clone(),
        config,
        rule_set,
        crate::engine::freshness::now_seconds(),
    )?;
    match projection.decision.verdict {
        RuleVerdict::Matched {
            effective_rule,
            effective_explanation,
            ..
        } if matches!(effective_rule.mode, RuleMode::Automatic)
            && !matches!(effective_rule.action, RuleAction::Ignore) =>
        {
            Ok(Some(AutomaticRuleCandidate {
                expires_at,
                rule: *effective_rule,
                explanation: *effective_explanation,
            }))
        }
        RuleVerdict::Matched { .. } | RuleVerdict::Unmatched => Ok(None),
    }
}

fn apply_rule_decision(
    tracked: &mut TrackedFile,
    decision: &RuleDecision,
    config: &AppConfig,
    now: u64,
) {
    let is_pinned_or_snoozed = match &tracked.expiry {
        Expiry::Permanent => true,
        Expiry::SnoozedUntil(until) if *until > now => true,
        _ => false,
    };

    let (matched_rule_ttl, matched_rule_is_ignore) = match &decision.verdict {
        RuleVerdict::Matched {
            effective_rule,
            rule_ttl_seconds,
            ..
        } => (
            *rule_ttl_seconds,
            !matches!(effective_rule.mode, RuleMode::PreviewOnly)
                && matches!(effective_rule.action, RuleAction::Ignore),
        ),
        RuleVerdict::Unmatched => (None, false),
    };

    if !is_pinned_or_snoozed {
        if let Some(ttl) = matched_rule_ttl {
            tracked.expiry = Expiry::At(tracked.freshness_at + ttl);
        } else {
            tracked.expiry = Expiry::At(tracked.freshness_at + config.default_ttl_seconds);
        }
    }

    if matches!(tracked.expiry, Expiry::Permanent) {
        tracked.state = FileDecayState::Pinned;
    } else if matched_rule_is_ignore {
        tracked.state = FileDecayState::Ignored;
    } else if matches!(tracked.state, FileDecayState::Ignored)
        && tracked.last_user_action_at.is_some()
    {
    } else {
        tracked.state = classify_decay_state(tracked.freshness_at, &tracked.expiry, now, config);
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use crate::engine::freshness::{now_seconds, tracked_file_from_metadata};
    use crate::models::{AppConfig, AutomationRule, Expiry, FileDecayState, RuleAction, RuleMode};
    use crate::rules::CompiledRuleSet;
    use crate::storage::test_util::Fixture;

    use super::{automatic_rule_candidate, project_watched_file};

    #[test]
    fn automatic_rule_applies_matched_ids_and_rule_ttl() {
        let fixture = Fixture::new("shelflife-rule-projection");
        let config = fixture.config();
        let tracked = tracked_fixture_file(&fixture, &config, "download.zip");
        let mut rule = fixture.rule();
        rule.mode = RuleMode::Automatic;

        let rule_set =
            CompiledRuleSet::compile(vec![rule], &config).expect("rule set should compile");
        let projection = project_watched_file(tracked, &config, &rule_set, now_seconds())
            .expect("projection should build");

        assert_eq!(
            projection.tracked.matched_rule_ids,
            vec![String::from("zip-rule")]
        );
        assert_eq!(
            projection.tracked.expiry,
            Expiry::At(projection.tracked.freshness_at + 86_400)
        );
    }

    #[test]
    fn preview_rule_does_not_block_lower_automatic_rule_ttl_and_candidate() {
        let fixture = Fixture::new("shelflife-rule-projection");
        let config = fixture.config();
        let tracked = tracked_fixture_file(&fixture, &config, "download.zip");
        let mut preview_rule = fixture.rule();
        preview_rule.id = String::from("preview-zip-rule");
        preview_rule.priority = 20;
        preview_rule.mode = RuleMode::PreviewOnly;
        preview_rule.ttl_seconds = 1;
        let mut automatic_rule = fixture.rule();
        automatic_rule.id = String::from("auto-zip-rule");
        automatic_rule.priority = 10;
        automatic_rule.mode = RuleMode::Automatic;
        automatic_rule.ttl_seconds = 1;
        let rule_set =
            CompiledRuleSet::compile(vec![automatic_rule.clone(), preview_rule], &config)
                .expect("rule set should compile");

        let projection = project_watched_file(tracked.clone(), &config, &rule_set, now_seconds())
            .expect("projection should build");
        let candidate = automatic_rule_candidate(&projection.tracked, &config, &rule_set)
            .expect("candidate should evaluate");

        assert_eq!(
            projection.tracked.matched_rule_ids,
            vec![
                String::from("preview-zip-rule"),
                String::from("auto-zip-rule")
            ]
        );
        assert_eq!(
            projection.tracked.expiry,
            Expiry::At(projection.tracked.freshness_at + automatic_rule.ttl_seconds)
        );
        let candidate = candidate.expect("lower automatic rule should remain actionable");
        assert_eq!(candidate.rule.id, automatic_rule.id);
    }

    #[test]
    fn automatic_ignore_sets_ignored_without_rule_ttl() {
        let fixture = Fixture::new("shelflife-rule-projection");
        let config = fixture.config();
        let tracked = tracked_fixture_file(&fixture, &config, "download.zip");
        let mut rule = fixture.rule();
        rule.mode = RuleMode::Automatic;
        rule.action = RuleAction::Ignore;
        rule.ttl_seconds = 1;

        let rule_set =
            CompiledRuleSet::compile(vec![rule], &config).expect("rule set should compile");
        let projection = project_watched_file(tracked, &config, &rule_set, now_seconds())
            .expect("projection should build");

        assert_eq!(projection.tracked.state, FileDecayState::Ignored);
        assert_eq!(
            projection.tracked.expiry,
            Expiry::At(projection.tracked.freshness_at + AppConfig::default().default_ttl_seconds)
        );
    }

    #[test]
    fn manually_ignored_file_stays_ignored_without_rule_match() {
        let fixture = Fixture::new("shelflife-rule-projection");
        let config = fixture.config();
        let mut tracked = tracked_fixture_file(&fixture, &config, "download.txt");
        tracked.state = FileDecayState::Ignored;
        tracked.last_user_action_at = Some(now_seconds());

        let rule_set = CompiledRuleSet::compile(Vec::<AutomationRule>::new(), &config)
            .expect("rule set should compile");
        let projection = project_watched_file(tracked, &config, &rule_set, now_seconds())
            .expect("projection should build");

        assert_eq!(projection.tracked.state, FileDecayState::Ignored);
        assert!(projection.tracked.matched_rule_ids.is_empty());
    }

    #[test]
    fn automatic_candidate_returns_actionable_rule_data_and_excludes_non_actionable_files() {
        let fixture = Fixture::new("shelflife-rule-projection");
        let config = fixture.config();
        let mut tracked = tracked_fixture_file(&fixture, &config, "download.zip");
        let expires_at = now_seconds() + 120;
        tracked.expiry = Expiry::At(expires_at);
        let mut automatic_rule = fixture.rule();
        automatic_rule.mode = RuleMode::Automatic;

        let rule_set = CompiledRuleSet::compile(vec![automatic_rule.clone()], &config)
            .expect("rule set should compile");
        let candidate = automatic_rule_candidate(&tracked, &config, &rule_set)
            .expect("candidate should evaluate")
            .expect("automatic rule should be actionable");

        assert_eq!(candidate.expires_at, expires_at);
        assert_eq!(candidate.rule.id, automatic_rule.id);
        assert_eq!(
            candidate.explanation.rule_id,
            Some(automatic_rule.id.clone())
        );

        let mut ask_first = automatic_rule.clone();
        ask_first.mode = RuleMode::AskFirst;
        let ask_first_rule_set =
            CompiledRuleSet::compile(vec![ask_first], &config).expect("rule set should compile");
        assert!(
            automatic_rule_candidate(&tracked, &config, &ask_first_rule_set)
                .expect("candidate should evaluate")
                .is_none()
        );

        let mut preview = automatic_rule.clone();
        preview.mode = RuleMode::PreviewOnly;
        let preview_rule_set =
            CompiledRuleSet::compile(vec![preview], &config).expect("rule set should compile");
        assert!(
            automatic_rule_candidate(&tracked, &config, &preview_rule_set)
                .expect("candidate should evaluate")
                .is_none()
        );

        let mut ignore = automatic_rule.clone();
        ignore.action = RuleAction::Ignore;
        let ignore_rule_set =
            CompiledRuleSet::compile(vec![ignore], &config).expect("rule set should compile");
        assert!(
            automatic_rule_candidate(&tracked, &config, &ignore_rule_set)
                .expect("candidate should evaluate")
                .is_none()
        );

        let mut ignored = tracked;
        ignored.state = FileDecayState::Ignored;
        let automatic_rule_set = CompiledRuleSet::compile(vec![automatic_rule], &config)
            .expect("rule set should compile");
        assert!(
            automatic_rule_candidate(&ignored, &config, &automatic_rule_set)
                .expect("candidate should evaluate")
                .is_none()
        );
    }

    fn tracked_fixture_file(
        fixture: &Fixture,
        config: &AppConfig,
        name: &str,
    ) -> crate::models::TrackedFile {
        let path = fixture.write_watch_file(name, "body");
        let metadata = fs::metadata(&path).expect("metadata should exist");
        let mut tracked = tracked_file_from_metadata(&path, &metadata, None, config, "watch");
        tracked.origin_url = None;
        tracked
    }
}
