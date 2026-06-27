use std::fs::Metadata;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::models::{AppConfig, Expiry, FileDecayState, OriginEvidence, TrackedFile};

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

pub fn calculate_freshness_at(
    first_seen_at: u64,
    last_observed_mtime: Option<u64>,
    last_observed_atime: Option<u64>,
    last_user_action_at: Option<u64>,
) -> u64 {
    [
        Some(first_seen_at),
        last_observed_mtime,
        last_observed_atime,
        last_user_action_at,
    ]
    .into_iter()
    .flatten()
    .max()
    .unwrap_or(first_seen_at)
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
    let first_seen_at = existing.map(|file| file.first_seen_at).unwrap_or(now);
    let last_observed_mtime = metadata.modified().ok().and_then(system_time_seconds);
    let last_observed_atime = metadata.accessed().ok().and_then(system_time_seconds);
    let last_user_action_at = existing.and_then(|file| file.last_user_action_at);
    let freshness_at = calculate_freshness_at(
        first_seen_at,
        last_observed_mtime,
        last_observed_atime,
        last_user_action_at,
    );
    let expiry = existing
        .map(|file| file.expiry.clone())
        .unwrap_or_else(|| Expiry::At(freshness_at + config.default_ttl_seconds));
    let state = existing
        .filter(|file| matches!(file.state, FileDecayState::Ignored))
        .map(|file| file.state.clone())
        .unwrap_or_else(|| classify_decay_state(freshness_at, &expiry, now, config));

    TrackedFile {
        path: path.to_string_lossy().to_string(),
        file_name,
        watch_target_id: existing
            .map(|f| f.watch_target_id.clone())
            .unwrap_or_else(|| watch_target_id.to_string()),
        size_bytes: metadata.len(),
        first_seen_at,
        last_observed_mtime,
        last_observed_atime,
        last_user_action_at,
        freshness_at,
        expiry,
        state,
        matched_rule_ids: Vec::new(),
        // Reuse existing origin evidence when already resolved — avoids an ADS
        // syscall per file on every scan. A replaced file will have a changed mtime
        // which causes a full re-evaluation including a fresh origin read.
        origin: match existing {
            Some(file) if !matches!(file.origin, crate::models::OriginEvidence::Unknown) => {
                file.origin.clone()
            }
            _ => read_origin_evidence(path),
        },
    }
}

#[cfg(target_os = "windows")]
pub fn read_origin_evidence(path: &Path) -> OriginEvidence {
    let ads_path = format!("{}:Zone.Identifier:$DATA", path.to_string_lossy());
    let Ok(content) = std::fs::read_to_string(ads_path) else {
        return OriginEvidence::Unknown;
    };

    let mut zone_id = None;
    let mut host_url = None;
    let mut referrer_url = None;

    for line in content.lines() {
        if let Some(value) = line.strip_prefix("ZoneId=") {
            zone_id = value.parse::<u32>().ok();
        } else if let Some(value) = line.strip_prefix("HostUrl=") {
            host_url = Some(value.to_string());
        } else if let Some(value) = line.strip_prefix("ReferrerUrl=") {
            referrer_url = Some(value.to_string());
        }
    }

    OriginEvidence::WindowsZoneIdentifier {
        zone_id,
        host_url,
        referrer_url,
    }
}

#[cfg(not(target_os = "windows"))]
pub fn read_origin_evidence(_path: &Path) -> OriginEvidence {
    OriginEvidence::Unknown
}
