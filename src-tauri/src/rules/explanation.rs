use crate::models::{AutomationRule, RuleMatchExplanation, SizeCondition};
use crate::rules::conditions::ConditionMatch;

fn format_size_brief(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * 1024;
    const GB: u64 = 1024 * 1024 * 1024;
    if bytes >= GB {
        let val = bytes as f64 / GB as f64;
        if val.fract() == 0.0 {
            format!("{val:.0} GB")
        } else {
            format!("{val:.1} GB")
        }
    } else if bytes >= MB {
        let val = bytes as f64 / MB as f64;
        if val.fract() == 0.0 {
            format!("{val:.0} MB")
        } else {
            format!("{val:.1} MB")
        }
    } else if bytes >= KB {
        let val = bytes as f64 / KB as f64;
        if val.fract() == 0.0 {
            format!("{val:.0} KB")
        } else {
            format!("{val:.1} KB")
        }
    } else {
        format!("{bytes} B")
    }
}

pub fn rule_explanation(
    file_path: &str,
    size_bytes: u64,
    rule: &AutomationRule,
    condition_match: ConditionMatch,
) -> RuleMatchExplanation {
    let message = if condition_match.matched {
        let mut matched_parts = Vec::new();
        if condition_match.matched_extension {
            if let Some(ext) = std::path::Path::new(file_path)
                .extension()
                .and_then(|e| e.to_str())
            {
                matched_parts.push(format!(".{ext}"));
            } else {
                matched_parts.push("Extension".to_string());
            }
        }
        if let Some(ref pattern) = condition_match.matched_filename_pattern {
            matched_parts.push(pattern.clone());
        }
        if let Some(ref origin) = condition_match.matched_origin {
            matched_parts.push(origin.clone());
        }
        if condition_match.matched_size {
            match &rule.conditions.size {
                SizeCondition::Any => {}
                SizeCondition::LessThan(max) => {
                    matched_parts.push(format!("< {}", format_size_brief(*max)));
                }
                SizeCondition::GreaterThan(min) => {
                    matched_parts.push(format!("> {}", format_size_brief(*min)));
                }
                SizeCondition::Between { min, max } => {
                    matched_parts.push(format!(
                        "{} - {}",
                        format_size_brief(*min),
                        format_size_brief(*max)
                    ));
                }
            }
        }

        if matched_parts.is_empty() {
            "Matched".to_string()
        } else {
            matched_parts.join(" • ")
        }
    } else {
        "No match".to_string()
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
        proposed_action: condition_match.matched.then(|| rule.action.clone()),
        mode: condition_match.matched.then(|| rule.mode.clone()),
        message,
    }
}
