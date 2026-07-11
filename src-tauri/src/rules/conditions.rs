use globset::{Glob, GlobSetBuilder};
use url::Url;

use crate::models::{AppError, OriginEvidence, RuleConditions, SizeCondition};
use crate::rules::regex_cache::cached_regex_is_match;

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
                    "RULE_INVALID_GLOB",
                    "Filename glob could not be parsed.",
                    true,
                    error.to_string(),
                )
            })?);
        }
        let glob_set = builder.build().map_err(|error| {
            AppError::with_details(
                "RULE_INVALID_GLOB",
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
        if cached_regex_is_match(pattern, file_name, "Filename regex could not be parsed.")? {
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
        let Some(host) = Url::parse(candidate)
            .ok()
            .and_then(|url| url.host_str().map(str::to_owned))
        else {
            continue;
        };

        for pattern in domains {
            if matches_pattern(pattern, &host) {
                return Some(pattern.trim().to_lowercase());
            }
        }
    }

    None
}

fn matches_pattern(pattern: &str, host: &str) -> bool {
    let pattern = pattern.trim();
    if pattern == "*" {
        return true;
    }
    let pattern = pattern.strip_suffix('.').unwrap_or(pattern);
    let host = host.trim_end_matches('.').to_lowercase();
    let (domain, include_apex) = match pattern.strip_prefix("*.") {
        Some(domain) => (domain.to_lowercase(), false),
        None => (pattern.to_lowercase(), true),
    };
    (include_apex && host == domain)
        || (host.ends_with(&domain)
            && host.len() > domain.len()
            && host.as_bytes()[host.len() - domain.len() - 1] == b'.')
}

pub fn validate_source_domain_pattern(pattern: &str) -> Result<(), AppError> {
    let pattern = pattern.trim();
    if pattern == "*" {
        return Ok(());
    }
    let pattern = pattern.strip_suffix('.').unwrap_or(pattern);
    let domain = pattern.strip_prefix("*.").unwrap_or(pattern);
    if domain.is_empty() || domain.contains('*') || !domain.contains('.') {
        return Err(AppError::with_details(
            "RULE_INVALID_SOURCE_DOMAIN",
            "Source domain must be a domain, *.domain wildcard, or *.",
            true,
            pattern.to_owned(),
        ));
    }
    for part in domain.split('.') {
        if part.is_empty()
            || part.starts_with('-')
            || part.ends_with('-')
            || part.chars().any(|c| !c.is_ascii_alphanumeric() && c != '-')
        {
            return Err(AppError::with_details(
                "RULE_INVALID_SOURCE_DOMAIN",
                "Source domain must be a domain, *.domain wildcard, or *.",
                true,
                pattern.to_owned(),
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_matches_pattern() {
        assert!(matches_pattern("*", "example.com"));
        assert!(matches_pattern("example.com", "example.com"));
        assert!(matches_pattern("example.com", "sub.example.com"));
        assert!(matches_pattern("example.com", "sub.sub.example.com"));
        assert!(matches_pattern("example.com.", "example.com"));
        assert!(!matches_pattern("example.com", "otherexample.com"));
        assert!(!matches_pattern("example.com", "example.com.org"));

        assert!(!matches_pattern("*.example.com", "example.com"));
        assert!(matches_pattern("*.example.com", "sub.example.com"));
        assert!(matches_pattern("*.example.com", "sub.sub.example.com"));
        assert!(!matches_pattern("*.example.com", "otherexample.com"));
    }

    #[test]
    fn test_validate_source_domain_pattern() {
        assert!(validate_source_domain_pattern("*").is_ok());
        assert!(validate_source_domain_pattern("example.com").is_ok());
        assert!(validate_source_domain_pattern("*.example.com").is_ok());
        assert!(validate_source_domain_pattern("example.com.").is_ok());

        assert!(validate_source_domain_pattern("").is_err());
        assert!(validate_source_domain_pattern("example").is_err());
        assert!(validate_source_domain_pattern("*.example").is_err());
        assert!(validate_source_domain_pattern("example*").is_err());
        assert!(validate_source_domain_pattern("example..com").is_err());
        assert!(validate_source_domain_pattern("example.com..").is_err());
        assert!(validate_source_domain_pattern("ex_ample.com").is_err());
        assert!(validate_source_domain_pattern("-example.com").is_err());
    }
}
