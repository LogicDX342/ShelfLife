use std::path::Path;

use crate::engine::paths::PathScope;
use crate::models::{AppError, AutomationRule, RuleAction, RuleMode, RuleTiming};

pub(crate) fn validate_rule(rule: &AutomationRule, scope: &PathScope<'_>) -> Result<(), AppError> {
    scope.validate_rule_watch_path(Path::new(&rule.watch_path))?;

    if matches!(rule.timing, RuleTiming::OnArrival)
        && !matches!(rule.action, RuleAction::Move { .. })
    {
        return Err(AppError::new(
            "RULE_INVALID_TIMING",
            "On-arrival timing is only available for move rules.",
            true,
        ));
    }

    if matches!(rule.timing, RuleTiming::OnArrival) && !matches!(rule.mode, RuleMode::Automatic) {
        return Err(AppError::new(
            "RULE_INVALID_ARRIVAL_MODE",
            "On-arrival rules must run automatically because they only handle future detection events.",
            true,
        ));
    }

    if matches!(rule.action, RuleAction::Ignore) && matches!(rule.mode, RuleMode::AskFirst) {
        return Err(AppError::new(
            "RULE_INVALID_MODE",
            "Ask-first mode is not available for Ignore rules because Ignore takes effect immediately.",
            true,
        ));
    }

    if let RuleAction::Move {
        destination_folder,
        rename_template,
    } = &rule.action
    {
        scope.validate_move_destination(Path::new(destination_folder))?;
        if let Some(template) = rename_template {
            validate_rename_template(template)?;
        }
    }

    Ok(())
}

pub(crate) fn validate_rename_template(template: &str) -> Result<(), AppError> {
    if template.trim().is_empty() {
        return Ok(());
    }

    let invalid_character = template.chars().find(|character| {
        matches!(
            character,
            '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|'
        )
    });
    if let Some(character) = invalid_character {
        return Err(AppError::with_details(
            "RULE_INVALID_RENAME_TEMPLATE",
            "Rename template contains a character that is not valid in Windows file names.",
            true,
            character.to_string(),
        ));
    }

    let mut remaining = template;
    while let Some(open_index) = remaining.find('{') {
        let after_open = &remaining[open_index + 1..];
        let Some(close_index) = after_open.find('}') else {
            return Err(AppError::new(
                "RULE_INVALID_RENAME_TEMPLATE",
                "Rename template has an unclosed placeholder.",
                true,
            ));
        };
        let placeholder = &after_open[..close_index];
        if !matches!(placeholder, "name" | "ext" | "file" | "date") {
            return Err(AppError::with_details(
                "RULE_INVALID_RENAME_TEMPLATE",
                "Rename template contains an unknown placeholder.",
                true,
                format!("{{{placeholder}}}"),
            ));
        }
        remaining = &after_open[close_index + 1..];
    }

    if remaining.contains('}') {
        return Err(AppError::new(
            "RULE_INVALID_RENAME_TEMPLATE",
            "Rename template has a closing placeholder brace without an opening brace.",
            true,
        ));
    }

    if !template.contains('{') {
        validate_reserved_name(template.trim_matches(' ').trim_end_matches('.'))?;
    }

    Ok(())
}

pub(crate) fn validate_reserved_name(file_name: &str) -> Result<(), AppError> {
    #[cfg(target_os = "windows")]
    {
        let stem = Path::new(file_name)
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or(file_name)
            .trim_end_matches('.');
        let upper = stem.to_ascii_uppercase();
        let reserved = matches!(
            upper.as_str(),
            "CON"
                | "PRN"
                | "AUX"
                | "NUL"
                | "COM1"
                | "COM2"
                | "COM3"
                | "COM4"
                | "COM5"
                | "COM6"
                | "COM7"
                | "COM8"
                | "COM9"
                | "LPT1"
                | "LPT2"
                | "LPT3"
                | "LPT4"
                | "LPT5"
                | "LPT6"
                | "LPT7"
                | "LPT8"
                | "LPT9"
        );
        if reserved {
            return Err(AppError::with_details(
                "RULE_INVALID_RENAME_TEMPLATE",
                "Rename template resolves to a reserved Windows file name.",
                true,
                file_name.to_string(),
            ));
        }
    }

    let _ = file_name;
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::engine::paths::PathScope;
    use crate::models::{RuleAction, RuleMode, RuleTiming};
    use crate::storage::test_util::Fixture;

    use super::validate_rule;

    #[test]
    fn ask_first_ignore_rule_is_rejected() {
        let fixture = Fixture::new("shelflife-rule-validation");
        let config = fixture.config();
        let scope = PathScope::new(&config);
        let mut rule = fixture.rule();
        rule.action = RuleAction::Ignore;
        rule.mode = RuleMode::AskFirst;

        let error = validate_rule(&rule, &scope).expect_err("Ask-first Ignore should be invalid");

        assert_eq!(error.code, "RULE_INVALID_MODE");
    }

    #[test]
    fn non_automatic_on_arrival_rule_is_rejected() {
        let fixture = Fixture::new("shelflife-arrival-rule-validation");
        let config = fixture.config();
        let scope = PathScope::new(&config);
        let mut rule = fixture.rule();
        rule.action = RuleAction::Move {
            destination_folder: fixture.safe.to_string_lossy().into_owned(),
            rename_template: None,
        };
        rule.timing = RuleTiming::OnArrival;

        for mode in [RuleMode::PreviewOnly, RuleMode::AskFirst] {
            rule.mode = mode;
            let error = validate_rule(&rule, &scope)
                .expect_err("non-automatic OnArrival should be invalid");

            assert_eq!(error.code, "RULE_INVALID_ARRIVAL_MODE");
        }

        rule.mode = RuleMode::Automatic;
        validate_rule(&rule, &scope).expect("automatic OnArrival should remain valid");
    }
}
