use std::cell::RefCell;
use std::collections::HashMap;

use regex::Regex;

use crate::models::AppError;

thread_local! {
    static REGEX_CACHE: RefCell<HashMap<String, Regex>> = RefCell::new(HashMap::new());
}

pub fn cached_regex_is_match(
    pattern: &str,
    value: &str,
    error_message: &'static str,
) -> Result<bool, AppError> {
    REGEX_CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        if !cache.contains_key(pattern) {
            let regex = Regex::new(pattern).map_err(|error| {
                AppError::with_details("RULE_INVALID_REGEX", error_message, true, error.to_string())
            })?;
            cache.insert(pattern.to_string(), regex);
        }

        Ok(cache
            .get(pattern)
            .expect("compiled regex should exist in cache")
            .is_match(value))
    })
}
