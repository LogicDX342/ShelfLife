use crate::models::{AutomationRule, RuleMatchExplanation};
use crate::rules::conditions::ConditionMatch;

pub fn protected_explanation(
    file_path: &str,
    size_bytes: u64,
    protected_pattern: String,
) -> RuleMatchExplanation {
    RuleMatchExplanation {
        file_path: file_path.to_string(),
        size_bytes: Some(size_bytes),
        rule_id: None,
        rule_name: None,
        matched_extension: false,
        matched_size: false,
        matched_origin: None,
        matched_filename_pattern: None,
        blocked_by_protected_pattern: Some(protected_pattern.clone()),
        proposed_action: None,
        mode: None,
        message: format!(
            "Protected by pattern {protected_pattern}. No action will run automatically."
        ),
    }
}

pub fn rule_explanation(
    file_path: &str,
    size_bytes: u64,
    rule: &AutomationRule,
    condition_match: ConditionMatch,
) -> RuleMatchExplanation {
    let message = if condition_match.matched {
        format!(
            "{} matched and proposes {:?} in {:?} mode.",
            rule.name, rule.action, rule.mode
        )
    } else {
        format!("{} did not match all conditions.", rule.name)
    };

    RuleMatchExplanation {
        file_path: file_path.to_string(),
        size_bytes: Some(size_bytes),
        rule_id: Some(rule.id.clone()),
        rule_name: Some(rule.name.clone()),
        matched_extension: condition_match.matched_extension,
        matched_size: condition_match.matched_size,
        matched_origin: condition_match.matched_origin,
        matched_filename_pattern: condition_match.matched_filename_pattern,
        blocked_by_protected_pattern: None,
        proposed_action: condition_match.matched.then(|| rule.action.clone()),
        mode: condition_match.matched.then(|| rule.mode.clone()),
        message,
    }
}
