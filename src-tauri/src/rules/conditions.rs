use globset::{Glob, GlobSetBuilder};
use regex::Regex;

use crate::models::{AppError, OriginEvidence, RuleConditions, SizeCondition};

pub struct ConditionMatch {
    pub matched_extension: bool,
    pub matched_size: bool,
    pub matched_origin: Option<String>,
    pub matched_filename_pattern: Option<String>,
    pub matched: bool,
}

pub fn evaluate_conditions(
    file_name: &str,
    size_bytes: u64,
    origin: &OriginEvidence,
    conditions: &RuleConditions,
) -> Result<ConditionMatch, AppError> {
    let matched_extension = matches_extension(file_name, &conditions.extensions);
    let matched_size = matches_size(size_bytes, &conditions.size);
    let matched_filename_pattern = matches_filename_pattern(
        file_name,
        &conditions.filename_globs,
        &conditions.filename_regexes,
    )?;
    let matched_origin = matches_origin(origin, &conditions.source_domains);

    let extension_ok = conditions.extensions.is_empty() || matched_extension;
    let filename_ok = conditions.filename_globs.is_empty()
        && conditions.filename_regexes.is_empty()
        || matched_filename_pattern.is_some();
    let origin_ok = conditions.source_domains.is_empty() || matched_origin.is_some();

    Ok(ConditionMatch {
        matched_extension,
        matched_size,
        matched_origin,
        matched_filename_pattern,
        matched: extension_ok && filename_ok && origin_ok && matched_size,
    })
}

pub fn matches_size(size_bytes: u64, condition: &SizeCondition) -> bool {
    match condition {
        SizeCondition::Any => true,
        SizeCondition::LessThan(max) => size_bytes < *max,
        SizeCondition::GreaterThan(min) => size_bytes > *min,
        SizeCondition::Between { min, max } => size_bytes >= *min && size_bytes <= *max,
    }
}

fn matches_extension(file_name: &str, extensions: &[String]) -> bool {
    if extensions.is_empty() {
        return false;
    }

    let ext = std::path::Path::new(file_name)
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .trim_start_matches('.')
        .to_lowercase();

    extensions
        .iter()
        .map(|value| value.trim_start_matches('.').to_lowercase())
        .any(|candidate| candidate == ext)
}

fn matches_filename_pattern(
    file_name: &str,
    globs: &[String],
    regexes: &[String],
) -> Result<Option<String>, AppError> {
    if !globs.is_empty() {
        let mut builder = GlobSetBuilder::new();
        for glob in globs {
            builder.add(Glob::new(glob).map_err(|error| {
                AppError::with_details(
                    "RULE_INVALID_REGEX",
                    "Filename glob could not be parsed.",
                    true,
                    error.to_string(),
                )
            })?);
        }
        let glob_set = builder.build().map_err(|error| {
            AppError::with_details(
                "RULE_INVALID_REGEX",
                "Filename glob set could not be built.",
                true,
                error.to_string(),
            )
        })?;

        if let Some(index) = glob_set.matches(file_name).first() {
            return Ok(globs.get(*index).cloned());
        }
    }

    for pattern in regexes {
        let regex = Regex::new(pattern).map_err(|error| {
            AppError::with_details(
                "RULE_INVALID_REGEX",
                "Filename regex could not be parsed.",
                true,
                error.to_string(),
            )
        })?;
        if regex.is_match(file_name) {
            return Ok(Some(pattern.clone()));
        }
    }

    Ok(None)
}

fn matches_origin(origin: &OriginEvidence, domains: &[String]) -> Option<String> {
    if domains.is_empty() {
        return None;
    }

    let candidates: Vec<&str> = match origin {
        OriginEvidence::WindowsZoneIdentifier {
            host_url,
            referrer_url,
            ..
        } => host_url
            .iter()
            .chain(referrer_url.iter())
            .map(String::as_str)
            .collect(),
        OriginEvidence::MacWhereFroms { values } => values.iter().map(String::as_str).collect(),
        OriginEvidence::LinuxXattr {
            value_utf8: Some(value),
            ..
        } => vec![value.as_str()],
        OriginEvidence::LinuxXattr {
            value_utf8: None, ..
        }
        | OriginEvidence::Unknown => Vec::new(),
    };

    for candidate in candidates {
        let normalized = candidate.to_lowercase();
        for domain in domains {
            let domain = domain.to_lowercase();
            if normalized.contains(&domain) {
                return Some(domain);
            }
        }
    }

    None
}
