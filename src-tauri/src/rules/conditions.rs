use globset::{Glob, GlobSet, GlobSetBuilder};
use regex::Regex;
use url::Url;

use crate::models::{AppError, RuleConditions, SizeCondition};

#[derive(Debug)]
pub(crate) struct CompiledConditions {
    extensions: Vec<String>,
    filename_globs: Vec<String>,
    filename_glob_set: Option<GlobSet>,
    filename_regexes: Vec<(String, Regex)>,
    source_domains: Vec<String>,
    size: SizeCondition,
}

#[derive(Debug)]
pub(crate) struct ConditionMatch {
    pub matched_extension: bool,
    pub matched_size: bool,
    pub matched_origin: Option<String>,
    pub matched_filename_pattern: Option<String>,
    pub matched: bool,
}

impl CompiledConditions {
    pub(crate) fn compile(conditions: &RuleConditions) -> Result<Self, AppError> {
        if let SizeCondition::Between { min, max } = conditions.size {
            if min > max {
                return Err(AppError::new(
                    "RULE_INVALID_SIZE_RANGE",
                    "Size range minimum cannot exceed maximum. Rule was not saved.",
                    true,
                ));
            }
        }

        let mut filename_glob_set = None;
        if !conditions.filename_globs.is_empty() {
            let mut builder = GlobSetBuilder::new();
            for pattern in &conditions.filename_globs {
                builder.add(Glob::new(pattern).map_err(|error| {
                    AppError::with_details(
                        "RULE_INVALID_GLOB",
                        "Filename glob could not be parsed.",
                        true,
                        error.to_string(),
                    )
                })?);
            }
            filename_glob_set = Some(builder.build().map_err(|error| {
                AppError::with_details(
                    "RULE_INVALID_GLOB",
                    "Filename glob set could not be built.",
                    true,
                    error.to_string(),
                )
            })?);
        }

        let filename_regexes = conditions
            .filename_regexes
            .iter()
            .map(|pattern| {
                Regex::new(pattern)
                    .map(|regex| (pattern.clone(), regex))
                    .map_err(|error| {
                        AppError::with_details(
                            "RULE_INVALID_REGEX",
                            "Filename regex could not be parsed.",
                            true,
                            error.to_string(),
                        )
                    })
            })
            .collect::<Result<Vec<_>, _>>()?;

        for pattern in &conditions.source_domains {
            validate_source_domain_pattern(pattern)?;
        }

        Ok(Self {
            extensions: conditions
                .extensions
                .iter()
                .map(|value| value.trim_start_matches('.').to_lowercase())
                .collect(),
            filename_globs: conditions.filename_globs.clone(),
            filename_glob_set,
            filename_regexes,
            source_domains: conditions.source_domains.clone(),
            size: conditions.size.clone(),
        })
    }

    pub(crate) fn evaluate(
        &self,
        file_name: &str,
        size_bytes: u64,
        origin_url: Option<&str>,
    ) -> ConditionMatch {
        let matched_extension = self.matches_extension(file_name);
        let matched_size = matches_size(size_bytes, &self.size);
        let matched_filename_pattern = self.matches_filename_pattern(file_name);
        let matched_origin = matches_origin(origin_url, &self.source_domains);

        let extension_ok = self.extensions.is_empty() || matched_extension;
        let filename_ok = self.filename_globs.is_empty() && self.filename_regexes.is_empty()
            || matched_filename_pattern.is_some();
        let origin_ok = self.source_domains.is_empty() || matched_origin.is_some();

        ConditionMatch {
            matched_extension,
            matched_size,
            matched_origin,
            matched_filename_pattern,
            matched: extension_ok && filename_ok && origin_ok && matched_size,
        }
    }

    fn matches_extension(&self, file_name: &str) -> bool {
        if self.extensions.is_empty() {
            return false;
        }

        let ext = std::path::Path::new(file_name)
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_lowercase();

        self.extensions.iter().any(|candidate| candidate == &ext)
    }

    fn matches_filename_pattern(&self, file_name: &str) -> Option<String> {
        if let Some(glob_set) = &self.filename_glob_set {
            if let Some(index) = glob_set.matches(file_name).first() {
                if let Some(pattern) = self.filename_globs.get(*index) {
                    return Some(pattern.clone());
                }
            }
        }

        self.filename_regexes
            .iter()
            .find(|(_, regex)| regex.is_match(file_name))
            .map(|(pattern, _)| pattern.clone())
    }
}

pub(crate) fn matches_size(size_bytes: u64, condition: &SizeCondition) -> bool {
    match condition {
        SizeCondition::Any => true,
        SizeCondition::LessThan(max) => size_bytes < *max,
        SizeCondition::GreaterThan(min) => size_bytes > *min,
        SizeCondition::Between { min, max } => size_bytes >= *min && size_bytes <= *max,
    }
}

fn matches_origin(origin_url: Option<&str>, domains: &[String]) -> Option<String> {
    if domains.is_empty() {
        return None;
    }

    let host = Url::parse(origin_url?)
        .ok()?
        .host_str()
        .map(str::to_owned)?;
    for pattern in domains {
        if matches_pattern(pattern, &host) {
            return Some(pattern.trim().to_lowercase());
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

pub(crate) fn validate_source_domain_pattern(pattern: &str) -> Result<(), AppError> {
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

    #[test]
    fn source_domain_matches_canonical_origin_url() {
        assert_eq!(
            matches_origin(
                Some("https://downloads.example.com/"),
                &[String::from("example.com")]
            ),
            Some(String::from("example.com"))
        );
        assert_eq!(matches_origin(None, &[String::from("example.com")]), None);
    }
}
