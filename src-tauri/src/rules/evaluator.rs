use std::cmp::Reverse;
use std::path::Path;

use crate::engine::paths::PathScope;
use crate::models::{AppConfig, AppError, AutomationRule, RuleMatchExplanation, TrackedFile};
use crate::rules::conditions::evaluate_conditions;
use crate::rules::explanation::{protected_explanation, rule_explanation};
use crate::rules::regex_cache::cached_regex_is_match;

pub fn explain_file_against_rules(
    file: &TrackedFile,
    config: &AppConfig,
    rules: &[AutomationRule],
) -> Result<Vec<RuleMatchExplanation>, AppError> {
    if let Some(pattern) = protected_pattern_match(&file.file_name, &config.protected_patterns)? {
        return Ok(vec![protected_explanation(
            &file.path,
            file.size_bytes,
            pattern,
        )]);
    }

    let mut enabled_rules: Vec<AutomationRule> =
        rules.iter().filter(|rule| rule.enabled).cloned().collect();
    enabled_rules.sort_by_key(|rule| Reverse(rule.priority));

    let mut explanations = Vec::new();
    let scope = PathScope::new(config);
    for rule in enabled_rules {
        if !scope.rule_watch_path_contains(&rule.watch_path, Path::new(&file.path)) {
            continue;
        }

        let condition_match = evaluate_conditions(
            &file.file_name,
            file.size_bytes,
            &file.origin,
            &rule.conditions,
        )?;
        explanations.push(rule_explanation(
            &file.path,
            file.size_bytes,
            &rule,
            condition_match,
        ));
    }

    if explanations.is_empty() {
        explanations.push(RuleMatchExplanation {
            file_path: file.path.clone(),
            size_bytes: Some(file.size_bytes),
            rule_id: None,
            rule_name: None,
            matched_extension: false,
            matched_size: true,
            matched_origin: None,
            matched_filename_pattern: None,
            blocked_by_protected_pattern: None,
            proposed_action: None,
            mode: None,
            message: String::from("No enabled rule matched this file."),
        });
    }

    Ok(explanations)
}

pub fn matching_rule_ids(
    file: &TrackedFile,
    config: &AppConfig,
    rules: &[AutomationRule],
) -> Result<Vec<String>, AppError> {
    Ok(explain_file_against_rules(file, config, rules)?
        .into_iter()
        .filter(|explanation| {
            explanation.proposed_action.is_some()
                && explanation.blocked_by_protected_pattern.is_none()
        })
        .filter_map(|explanation| explanation.rule_id)
        .collect())
}

pub fn protected_pattern_match(
    file_name: &str,
    patterns: &[String],
) -> Result<Option<String>, AppError> {
    for pattern in patterns {
        if cached_regex_is_match(pattern, file_name, "Protected pattern could not be parsed.")? {
            return Ok(Some(pattern.clone()));
        }
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use crate::models::{AppConfig, OriginEvidence, RuleConditions, SizeCondition};

    use super::protected_pattern_match;

    #[test]
    fn protected_patterns_match_before_rules() {
        let pattern = protected_pattern_match(
            "2025_tax_return.pdf",
            &AppConfig::default().protected_patterns,
        )
        .expect("pattern should parse");
        assert!(pattern.is_some());
    }

    #[test]
    fn source_domain_unknown_is_not_matched_by_default() {
        let conditions = RuleConditions {
            source_domains: vec![String::from("example.com")],
            size: SizeCondition::Any,
            ..RuleConditions::default()
        };
        let result = crate::rules::conditions::evaluate_conditions(
            "download.zip",
            10,
            &OriginEvidence::Unknown,
            &conditions,
        )
        .expect("conditions should evaluate");
        assert!(!result.matched);
    }
}
