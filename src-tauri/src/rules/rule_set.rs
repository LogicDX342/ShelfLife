use std::cmp::Reverse;
use std::path::Path;

use crate::engine::paths::{root_contains, PathScope};
use crate::models::{
    AppConfig, AppError, AutomationRule, RuleAction, RuleMatchExplanation, RuleMode, TrackedFile,
};

use super::conditions::CompiledConditions;
use super::explanation::rule_explanation;
use super::validation::validate_rule;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuleDecisionScope {
    WatchedFile,
    Dropzone,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuleVerdict {
    Matched {
        effective_rule: Box<AutomationRule>,
        effective_explanation: Box<RuleMatchExplanation>,
        rule_ttl_seconds: Option<u64>,
    },
    Unmatched,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleDecision {
    pub verdict: RuleVerdict,
    pub explanations: Vec<RuleMatchExplanation>,
    pub matched_rule_ids: Vec<String>,
}

struct CompiledRule {
    rule: AutomationRule,
    conditions: CompiledConditions,
}

pub struct CompiledRuleSet {
    rules: Vec<CompiledRule>,
}

impl CompiledRuleSet {
    pub fn compile(
        rules: impl IntoIterator<Item = AutomationRule>,
        config: &AppConfig,
    ) -> Result<Self, AppError> {
        let scope = PathScope::new(config);
        let mut compiled_rules = Vec::new();

        for rule in rules {
            let conditions = CompiledConditions::compile(&rule.conditions)?;
            validate_rule(&rule, &scope)?;
            compiled_rules.push(CompiledRule { rule, conditions });
        }

        compiled_rules.sort_by_key(|rule| Reverse(rule.rule.priority));

        Ok(Self {
            rules: compiled_rules,
        })
    }

    pub fn decide_file(&self, file: &TrackedFile, scope: RuleDecisionScope) -> RuleDecision {
        let mut explanations = Vec::new();
        let mut matched_rule_ids = Vec::new();
        let mut effective_match = None;

        for compiled_rule in self.rules.iter().filter(|rule| rule.rule.enabled) {
            if !should_evaluate_rule(scope, compiled_rule, &file.path) {
                continue;
            }

            let condition_match =
                compiled_rule
                    .conditions
                    .evaluate(&file.file_name, file.size_bytes, &file.origin);
            let explanation = rule_explanation(
                &file.path,
                file.size_bytes,
                &compiled_rule.rule,
                condition_match,
            );

            if explanation.proposed_action.is_some() {
                matched_rule_ids.push(compiled_rule.rule.id.clone());
                if effective_match.is_none() {
                    effective_match = Some((
                        compiled_rule.rule.clone(),
                        explanation.clone(),
                        rule_ttl_seconds_for_match(&compiled_rule.rule),
                    ));
                }
            }

            explanations.push(explanation);
        }

        if explanations.is_empty() {
            explanations.push(unmatched_explanation(file));
        }

        let verdict = match effective_match {
            Some((effective_rule, effective_explanation, rule_ttl_seconds)) => {
                RuleVerdict::Matched {
                    effective_rule: Box::new(effective_rule),
                    effective_explanation: Box::new(effective_explanation),
                    rule_ttl_seconds,
                }
            }
            None => RuleVerdict::Unmatched,
        };

        RuleDecision {
            verdict,
            explanations,
            matched_rule_ids,
        }
    }

    pub fn explain_file(&self, file: &TrackedFile) -> Vec<RuleMatchExplanation> {
        self.decide_file(file, RuleDecisionScope::WatchedFile)
            .explanations
    }
}

fn should_evaluate_rule(scope: RuleDecisionScope, rule: &CompiledRule, file_path: &str) -> bool {
    match scope {
        RuleDecisionScope::WatchedFile => {
            root_contains(&rule.rule.watch_path, Path::new(file_path))
        }
        RuleDecisionScope::Dropzone => true,
    }
}

fn rule_ttl_seconds_for_match(rule: &AutomationRule) -> Option<u64> {
    if matches!(rule.mode, RuleMode::PreviewOnly) || matches!(rule.action, RuleAction::Ignore) {
        None
    } else {
        Some(rule.ttl_seconds)
    }
}

fn unmatched_explanation(file: &TrackedFile) -> RuleMatchExplanation {
    RuleMatchExplanation {
        file_path: file.path.clone(),
        size_bytes: Some(file.size_bytes),
        rule_id: None,
        rule_name: None,
        matched_extension: false,
        matched_size: true,
        matched_origin: None,
        matched_filename_pattern: None,
        proposed_action: None,
        mode: None,
        message: String::from("No enabled rule matched this file."),
    }
}

#[cfg(test)]
mod tests {
    use crate::models::{OriginEvidence, RuleAction, RuleConditions, RuleMode, SizeCondition};
    use crate::storage::test_util::Fixture;

    use super::{CompiledRuleSet, RuleDecisionScope, RuleVerdict};

    #[test]
    fn source_domain_unknown_is_not_matched_by_default() {
        let conditions = RuleConditions {
            source_domains: vec![String::from("example.com")],
            size: SizeCondition::Any,
            ..RuleConditions::default()
        };
        let compiled = super::super::conditions::CompiledConditions::compile(&conditions)
            .expect("conditions should compile");
        let result = compiled.evaluate("download.zip", 10, &OriginEvidence::Unknown);
        assert!(!result.matched);
    }

    #[test]
    fn higher_priority_preview_rule_is_effective_and_has_no_rule_ttl() {
        let fixture = Fixture::new("shelflife-rule-decision");
        let file = fixture.write_watch_file("download.zip", "body");
        fixture.save_config();
        fixture.track_file(&file);
        let tracked =
            crate::storage::tracked::get_tracked_file(&fixture.db, &file.to_string_lossy())
                .expect("tracked lookup should work")
                .expect("tracked file should exist");

        let mut preview = fixture.rule();
        preview.priority = 50;
        preview.mode = RuleMode::PreviewOnly;
        let mut automatic = fixture.rule();
        automatic.id = String::from("auto-zip-rule");
        automatic.priority = 10;
        automatic.mode = RuleMode::Automatic;

        let rule_set =
            CompiledRuleSet::compile(vec![automatic.clone(), preview.clone()], &fixture.config())
                .expect("rule set should compile");
        let decision = rule_set.decide_file(&tracked, RuleDecisionScope::WatchedFile);

        assert_eq!(
            decision.matched_rule_ids,
            vec![String::from("zip-rule"), String::from("auto-zip-rule")]
        );
        match decision.verdict {
            RuleVerdict::Matched {
                effective_rule,
                rule_ttl_seconds,
                ..
            } => {
                assert_eq!(effective_rule.id, preview.id);
                assert_eq!(rule_ttl_seconds, None);
            }
            RuleVerdict::Unmatched => panic!("preview rule should be effective"),
        }
    }

    #[test]
    fn ignore_match_has_no_rule_ttl() {
        let fixture = Fixture::new("shelflife-rule-decision");
        let file = fixture.write_watch_file("download.zip", "body");
        fixture.save_config();
        fixture.track_file(&file);
        let tracked =
            crate::storage::tracked::get_tracked_file(&fixture.db, &file.to_string_lossy())
                .expect("tracked lookup should work")
                .expect("tracked file should exist");

        let mut rule = fixture.rule();
        rule.mode = RuleMode::Automatic;
        rule.action = RuleAction::Ignore;

        let rule_set = CompiledRuleSet::compile(vec![rule], &fixture.config())
            .expect("rule set should compile");
        let decision = rule_set.decide_file(&tracked, RuleDecisionScope::WatchedFile);

        match decision.verdict {
            RuleVerdict::Matched {
                rule_ttl_seconds, ..
            } => assert_eq!(rule_ttl_seconds, None),
            RuleVerdict::Unmatched => panic!("ignore rule should be effective"),
        }
    }
}
