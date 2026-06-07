use std::cmp::Reverse;
use std::path::Path;

use regex::Regex;

use crate::models::{AppConfig, AppError, AutomationRule, RuleMatchExplanation, TrackedFile};
use crate::rules::conditions::evaluate_conditions;
use crate::rules::explanation::{protected_explanation, rule_explanation};

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
    for rule in enabled_rules {
        if !path_is_inside(Path::new(&file.path), Path::new(&rule.watch_path)) {
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

use std::cell::RefCell;
use std::collections::HashMap;

thread_local! {
    static REGEX_CACHE: RefCell<HashMap<String, Regex>> = RefCell::new(HashMap::new());
}

pub fn protected_pattern_match(
    file_name: &str,
    patterns: &[String],
) -> Result<Option<String>, AppError> {
    REGEX_CACHE.with(|cache| {
        for pattern in patterns {
            let mut cache_borrow = cache.borrow_mut();
            let regex = if cache_borrow.contains_key(pattern) {
                cache_borrow.get(pattern).unwrap()
            } else {
                let re = Regex::new(pattern).map_err(|error| {
                    AppError::with_details(
                        "RULE_INVALID_REGEX",
                        "Protected pattern could not be parsed.",
                        true,
                        error.to_string(),
                    )
                })?;
                cache_borrow.insert(pattern.clone(), re);
                cache_borrow.get(pattern).unwrap()
            };

            if regex.is_match(file_name) {
                return Ok(Some(pattern.clone()));
            }
        }
        Ok(None)
    })
}

fn path_is_inside(path: &Path, root: &Path) -> bool {
    // Fast path: string prefix check
    let path_str = path.to_string_lossy().to_lowercase().replace('/', "\\");
    let root_str = root.to_string_lossy().to_lowercase().replace('/', "\\");

    // Ensure root ends with a separator to avoid partial folder match (e.g., /foo matching /foobar)
    let root_prefix = if root_str.ends_with('\\') {
        root_str.clone()
    } else {
        format!("{}\\", root_str)
    };

    if !path_str.starts_with(&root_prefix) && path_str != root_str {
        return false;
    }

    let Ok(path) = path.canonicalize() else {
        return false;
    };
    let Ok(root) = root.canonicalize() else {
        return false;
    };
    path.starts_with(root)
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
