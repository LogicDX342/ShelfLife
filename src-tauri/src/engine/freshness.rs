use std::fs::Metadata;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use url::Url;

use crate::models::{AppConfig, Expiry, FileDecayState, TrackedFile};

pub fn now_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

pub fn system_time_seconds(time: SystemTime) -> Option<u64> {
    time.duration_since(UNIX_EPOCH)
        .ok()
        .map(|value| value.as_secs())
}

pub fn classify_decay_state(
    freshness_at: u64,
    expiry: &Expiry,
    now: u64,
    config: &AppConfig,
) -> FileDecayState {
    match expiry {
        Expiry::Permanent => FileDecayState::Pinned,
        Expiry::SnoozedUntil(until) if *until > now => FileDecayState::Fresh,
        Expiry::SnoozedUntil(_) | Expiry::At(_) => {
            let expires_at = match expiry {
                Expiry::At(value) | Expiry::SnoozedUntil(value) => *value,
                Expiry::Permanent => unreachable!(),
            };
            let (stale_threshold_seconds, decaying_threshold_seconds) =
                effective_decay_thresholds(freshness_at, expires_at, config);

            if expires_at <= now.saturating_add(decaying_threshold_seconds) {
                FileDecayState::Decaying
            } else if freshness_at.saturating_add(stale_threshold_seconds) <= now {
                FileDecayState::Stale
            } else {
                FileDecayState::Fresh
            }
        }
    }
}

fn effective_decay_thresholds(
    freshness_at: u64,
    expires_at: u64,
    config: &AppConfig,
) -> (u64, u64) {
    let effective_ttl_seconds = expires_at.saturating_sub(freshness_at);

    if config.default_ttl_seconds == 0 || effective_ttl_seconds == config.default_ttl_seconds {
        return (
            config.stale_threshold_seconds,
            config.decaying_threshold_seconds,
        );
    }

    (
        scale_threshold_to_ttl(
            config.stale_threshold_seconds,
            effective_ttl_seconds,
            config.default_ttl_seconds,
        ),
        scale_threshold_to_ttl(
            config.decaying_threshold_seconds,
            effective_ttl_seconds,
            config.default_ttl_seconds,
        ),
    )
}

fn scale_threshold_to_ttl(global_threshold: u64, effective_ttl: u64, default_ttl: u64) -> u64 {
    if global_threshold == 0 || effective_ttl == 0 || default_ttl == 0 {
        return 0;
    }

    let scaled =
        (global_threshold as u128).saturating_mul(effective_ttl as u128) / default_ttl as u128;
    let capped = scaled.min(effective_ttl as u128) as u64;

    capped.max(1)
}

pub fn tracked_file_from_metadata(
    path: &Path,
    metadata: &Metadata,
    existing: Option<&TrackedFile>,
    config: &AppConfig,
    watch_target_id: &str,
) -> TrackedFile {
    let now = now_seconds();
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_string();
    let last_observed_mtime = metadata.modified().ok().and_then(system_time_seconds);
    let baseline_freshness = existing.map(|file| file.freshness_at).unwrap_or(now);
    let freshness_at =
        last_observed_mtime.map_or(baseline_freshness, |mtime| baseline_freshness.max(mtime));
    let expiry = existing
        .map(|file| file.expiry.clone())
        .unwrap_or_else(|| Expiry::At(freshness_at + config.default_ttl_seconds));
    let state = existing
        .filter(|file| matches!(file.state, FileDecayState::ManuallyIgnored))
        .map(|file| file.state)
        .unwrap_or_else(|| classify_decay_state(freshness_at, &expiry, now, config));

    TrackedFile {
        path: path.to_string_lossy().to_string(),
        file_name,
        watch_target_id: existing
            .map(|f| f.watch_target_id.clone())
            .unwrap_or_else(|| watch_target_id.to_string()),
        size_bytes: metadata.len(),
        last_observed_mtime,
        freshness_at,
        expiry,
        state,
        matched_rule_ids: Vec::new(),
        // Reuse the previous origin lookup for an unchanged file, including None:
        // on Windows, None normally means the Zone.Identifier ADS had no usable URL.
        // A replaced file will have changed size or mtime and refresh the evidence.
        origin_url: match existing {
            Some(file)
                if file.size_bytes == metadata.len()
                    && file.last_observed_mtime == last_observed_mtime =>
            {
                file.origin_url.clone()
            }
            _ => read_origin_url(path),
        },
    }
}

#[cfg(target_os = "windows")]
pub fn read_origin_url(path: &Path) -> Option<String> {
    let ads_path = format!("{}:Zone.Identifier:$DATA", path.to_string_lossy());
    let Ok(content) = std::fs::read_to_string(ads_path) else {
        return None;
    };

    let mut host_url = None;
    let mut referrer_url = None;

    for line in content.lines() {
        if let Some(value) = line.strip_prefix("HostUrl=") {
            host_url = Some(value.to_string());
        } else if let Some(value) = line.strip_prefix("ReferrerUrl=") {
            referrer_url = Some(value.to_string());
        }
    }

    canonical_origin_url(
        host_url
            .iter()
            .chain(referrer_url.iter())
            .map(String::as_str),
    )
}

#[cfg(not(target_os = "windows"))]
pub fn read_origin_url(_path: &Path) -> Option<String> {
    None
}

pub(crate) fn canonical_origin_url<'a>(
    candidates: impl IntoIterator<Item = &'a str>,
) -> Option<String> {
    candidates.into_iter().find_map(|candidate| {
        let mut url = Url::parse(candidate.trim()).ok()?;
        if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
            return None;
        }

        url.set_username("").ok()?;
        url.set_password(None).ok()?;
        url.set_path("/");
        url.set_query(None);
        url.set_fragment(None);
        Some(url.to_string())
    })
}

#[cfg(test)]
mod tests {
    use std::fs;

    use crate::storage::test_util::Fixture;

    use super::tracked_file_from_metadata;

    fn known_origin_url() -> Option<String> {
        Some(String::from("https://example.com/"))
    }

    #[test]
    fn unchanged_file_reuses_known_origin() {
        let fixture = Fixture::new("shelflife-origin-reuse");
        let path = fixture.write_watch_file("download.txt", "body");
        let metadata = fs::metadata(&path).expect("metadata should exist");
        let mut existing =
            tracked_file_from_metadata(&path, &metadata, None, &fixture.config(), "watch");
        existing.origin_url = known_origin_url();

        let refreshed = tracked_file_from_metadata(
            &path,
            &metadata,
            Some(&existing),
            &fixture.config(),
            "watch",
        );

        assert_eq!(refreshed.origin_url, known_origin_url());
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn unchanged_file_reuses_absent_origin() {
        let fixture = Fixture::new("shelflife-unknown-origin-reuse");
        let path = fixture.write_watch_file("local.txt", "body");
        let ads_path = format!("{}:Zone.Identifier:$DATA", path.to_string_lossy());
        if fs::write(&ads_path, "[ZoneTransfer]\nZoneId=3\n").is_err() {
            return;
        }
        let metadata = fs::metadata(&path).expect("metadata should exist");
        let mut existing =
            tracked_file_from_metadata(&path, &metadata, None, &fixture.config(), "watch");
        existing.origin_url = None;

        let refreshed = tracked_file_from_metadata(
            &path,
            &metadata,
            Some(&existing),
            &fixture.config(),
            "watch",
        );

        assert_eq!(refreshed.origin_url, None);
    }

    #[test]
    fn changed_file_does_not_reuse_stale_origin() {
        let fixture = Fixture::new("shelflife-origin-refresh");
        let path = fixture.write_watch_file("download.txt", "body");
        let metadata = fs::metadata(&path).expect("metadata should exist");
        let mut existing =
            tracked_file_from_metadata(&path, &metadata, None, &fixture.config(), "watch");
        existing.origin_url = known_origin_url();

        fs::write(&path, "different-sized body").expect("file should be replaced");
        let changed_metadata = fs::metadata(&path).expect("metadata should exist");
        let refreshed = tracked_file_from_metadata(
            &path,
            &changed_metadata,
            Some(&existing),
            &fixture.config(),
            "watch",
        );

        assert_ne!(refreshed.origin_url, known_origin_url());
    }
}
