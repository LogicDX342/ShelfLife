use redb::Database;

use crate::models::{AppConfig, AppError, ReconciliationReport, TrackedFile, WatchTarget};
use crate::rules::matching_rule_ids;
use crate::storage;

pub fn refresh_tracked_rule_state(db: &Database) -> Result<ReconciliationReport, AppError> {
    let config = storage::get_config(db)?;
    let rules = storage::rules::list_rules(db)?;
    let files = storage::tracked::list_tracked_files(db)?;
    let now = crate::engine::freshness::now_seconds();

    let mut report = ReconciliationReport::default();
    let mut changed = Vec::new();

    for file in files {
        let mut refreshed = file.clone();
        refreshed.matched_rule_ids = matching_rule_ids(&refreshed, &config, &rules)?;
        let ttl_seconds = effective_ttl_seconds_for_file(&config, &refreshed);
        crate::engine::freshness::apply_rules_to_tracked_file(
            &mut refreshed,
            &rules,
            &config,
            ttl_seconds,
            now,
        );

        if refreshed != file {
            report.updated.push(refreshed.path.clone());
            changed.push(refreshed);
        }
    }

    if !changed.is_empty() {
        storage::tracked::upsert_tracked_files_batch(db, &changed)?;
    }

    Ok(report)
}

fn effective_ttl_seconds_for_file(config: &AppConfig, file: &TrackedFile) -> u64 {
    config
        .watch_targets
        .iter()
        .find(|target| target.id == file.watch_target_id)
        .map(|target| effective_ttl_seconds(config, target))
        .unwrap_or(config.default_ttl_seconds)
}

fn effective_ttl_seconds(config: &AppConfig, target: &WatchTarget) -> u64 {
    target
        .default_ttl_seconds
        .unwrap_or(config.default_ttl_seconds)
}

#[cfg(test)]
mod tests {
    use crate::models::{Expiry, RuleMode};
    use crate::storage;
    use crate::storage::test_util::Fixture;

    use super::refresh_tracked_rule_state;

    #[test]
    fn refresh_updates_tracked_rule_matches_from_database() {
        let fixture = Fixture::new("shelflife-rule-refresh");
        let file = fixture.write_watch_file("download.zip", "body");
        fixture.save_config();
        fixture.track_file(&file);

        let mut rule = fixture.rule();
        rule.mode = RuleMode::Automatic;
        storage::rules::save_rule(&fixture.db, &rule).expect("rule should save");

        let report = refresh_tracked_rule_state(&fixture.db).expect("rule state should refresh");
        let tracked = storage::tracked::get_tracked_file(&fixture.db, &file.to_string_lossy())
            .expect("tracked lookup should work")
            .expect("tracked file should exist");

        assert_eq!(report.updated, vec![file.to_string_lossy().to_string()]);
        assert_eq!(tracked.matched_rule_ids, vec![String::from("zip-rule")]);
        assert_eq!(tracked.expiry, Expiry::At(tracked.freshness_at + 86_400));
    }

    #[test]
    fn refresh_removes_deleted_rule_matches_and_restores_default_ttl() {
        let fixture = Fixture::new("shelflife-rule-refresh");
        let file = fixture.write_watch_file("download.zip", "body");
        fixture.save_config();
        fixture.track_file(&file);

        let mut rule = fixture.rule();
        rule.mode = RuleMode::Automatic;
        storage::rules::save_rule(&fixture.db, &rule).expect("rule should save");
        refresh_tracked_rule_state(&fixture.db).expect("initial refresh should work");

        storage::rules::delete_rule(&fixture.db, &rule.id).expect("rule should delete");
        refresh_tracked_rule_state(&fixture.db).expect("second refresh should work");
        let tracked = storage::tracked::get_tracked_file(&fixture.db, &file.to_string_lossy())
            .expect("tracked lookup should work")
            .expect("tracked file should exist");

        assert!(tracked.matched_rule_ids.is_empty());
        assert_eq!(
            tracked.expiry,
            Expiry::At(
                tracked.freshness_at + crate::models::AppConfig::default().default_ttl_seconds
            )
        );
    }
}
