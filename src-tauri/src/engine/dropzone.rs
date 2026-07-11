use std::collections::{HashMap, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};

use crate::engine::freshness::tracked_file_from_metadata;
use crate::engine::paths::PathScope;
use crate::models::{
    AppConfig, AppError, AutomationRule, DropzoneFile, DropzonePreview, DropzoneRejectedFile,
    DropzoneRuleGroup, RuleAction, RuleMatchExplanation, RuleMode, TrackedFile,
};
use crate::rules::{decide_file_against_rules, RuleDecisionScope, RuleVerdict};
use crate::storage::{self, Database};

pub const SHAKE_INTERVAL_MS: u64 = 1_000;
pub const SHAKE_MINIMUM_DISTANCE: f64 = 900.0;
pub const SHAKE_FACTOR: f64 = 4.0;

#[derive(Debug, Clone, Copy)]
struct PointerMove {
    dx: i32,
    dy: i32,
    time_ms: u64,
}

#[derive(Debug, Default)]
pub struct ShakeDetector {
    last_position: Option<(i32, i32)>,
    moves: VecDeque<PointerMove>,
}

impl ShakeDetector {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn update(&mut self, left_button_down: bool, x: i32, y: i32, time_ms: u64) -> bool {
        if !left_button_down {
            self.reset();
            return false;
        }

        let Some((previous_x, previous_y)) = self.last_position.replace((x, y)) else {
            return false;
        };

        let dx = x - previous_x;
        let dy = y - previous_y;
        if dx == 0 && dy == 0 {
            self.prune(time_ms);
            return false;
        }

        let changed_direction = self.push_move(PointerMove { dx, dy, time_ms });
        self.prune(time_ms);

        if changed_direction && self.is_shaking() {
            self.reset();
            return true;
        }

        false
    }

    pub fn reset(&mut self) {
        self.last_position = None;
        self.moves.clear();
    }

    fn push_move(&mut self, movement: PointerMove) -> bool {
        if let Some(last) = self.moves.back_mut() {
            if direction(last.dx) == direction(movement.dx)
                && direction(last.dy) == direction(movement.dy)
            {
                last.dx += movement.dx;
                last.dy += movement.dy;
                return false;
            }
        }

        self.moves.push_back(movement);
        true
    }

    fn prune(&mut self, time_ms: u64) {
        let earliest = time_ms.saturating_sub(SHAKE_INTERVAL_MS);
        while self
            .moves
            .front()
            .is_some_and(|movement| movement.time_ms < earliest)
        {
            self.moves.pop_front();
        }
    }

    fn is_shaking(&self) -> bool {
        if self.moves.len() < 2 {
            return false;
        }

        let total_distance = self
            .moves
            .iter()
            .map(|movement| distance(movement.dx, movement.dy))
            .sum::<f64>();
        if total_distance < SHAKE_MINIMUM_DISTANCE {
            return false;
        }

        let (mut x, mut y) = (0, 0);
        let (mut min_x, mut max_x, mut min_y, mut max_y) = (0, 0, 0, 0);
        for movement in &self.moves {
            x += movement.dx;
            y += movement.dy;
            min_x = min_x.min(x);
            max_x = max_x.max(x);
            min_y = min_y.min(y);
            max_y = max_y.max(y);
        }
        let diagonal = distance(max_x - min_x, max_y - min_y).max(1.0);

        total_distance >= diagonal * SHAKE_FACTOR
    }
}

fn direction(value: i32) -> i8 {
    value.signum() as i8
}

fn distance(dx: i32, dy: i32) -> f64 {
    let dx = f64::from(dx);
    let dy = f64::from(dy);
    (dx * dx + dy * dy).sqrt()
}

pub fn preview_dropzone_files(
    db: &Database,
    paths: &[String],
) -> Result<DropzonePreview, AppError> {
    let config = storage::get_config(db)?;
    let rules = storage::rules::list_rules(db)?;
    let mut files = Vec::new();
    let mut tracked_files = Vec::new();
    let mut rejected_files = Vec::new();

    for path in paths {
        match build_dropzone_file(path, &config) {
            Ok((file, tracked)) => {
                files.push(file);
                tracked_files.push(tracked);
            }
            Err(error) => rejected_files.push(DropzoneRejectedFile {
                path: path.clone(),
                reason: error.message,
            }),
        }
    }

    let (rule_groups, preview_only, unmatched_files) =
        plan_rule_groups(&config, &rules, &tracked_files)?;

    Ok(DropzonePreview {
        files,
        rejected_files,
        watch_targets: config
            .watch_targets
            .into_iter()
            .filter(|target| target.enabled)
            .collect(),
        rule_groups,
        preview_only,
        unmatched_files,
    })
}

pub fn build_dropzone_file(
    path: &str,
    config: &AppConfig,
) -> Result<(DropzoneFile, TrackedFile), AppError> {
    let source = PathBuf::from(path);
    if !source.exists() {
        return Err(AppError::path_not_found(path));
    }
    if !source.is_file() {
        return Err(AppError::with_details(
            "PATH_OUT_OF_SCOPE",
            "Only files can be dropped into the dropzone. No file was changed.",
            true,
            path,
        ));
    }

    let metadata = fs::metadata(&source)?;
    let file_name = source
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or(path)
        .to_string();
    let tracked = tracked_file_from_metadata(&source, &metadata, None, config, "");

    Ok((
        DropzoneFile {
            path: path.to_string(),
            file_name,
            size_bytes: metadata.len(),
        },
        tracked,
    ))
}

#[allow(clippy::type_complexity)]
pub fn plan_rule_groups(
    config: &AppConfig,
    rules: &[AutomationRule],
    files: &[TrackedFile],
) -> Result<
    (
        Vec<DropzoneRuleGroup>,
        Vec<RuleMatchExplanation>,
        Vec<String>,
    ),
    AppError,
> {
    let mut groups: HashMap<String, DropzoneRuleGroup> = HashMap::new();
    let mut preview_only = Vec::new();
    let mut unmatched_files = Vec::new();
    let scope = PathScope::new(config);

    for file in files {
        let decision = decide_file_against_rules(file, config, rules, RuleDecisionScope::Dropzone)?;
        let RuleVerdict::Matched {
            effective_rule: rule,
            effective_explanation,
            ..
        } = decision.verdict
        else {
            unmatched_files.push(file.path.clone());
            continue;
        };

        if matches!(rule.mode, RuleMode::PreviewOnly) {
            preview_only.push(*effective_explanation);
            continue;
        }

        let mut effective_explanation = *effective_explanation;
        if matches!(rule.action, RuleAction::Ignore)
            && !scope.is_in_enabled_watch_target(Path::new(&file.path))
        {
            effective_explanation.proposed_action = None;
            effective_explanation.message =
                String::from("Dropzone skipped Ignore because this file is outside watch targets.");
            preview_only.push(effective_explanation);
            unmatched_files.push(file.path.clone());
            continue;
        }

        let group = groups
            .entry(rule.id.clone())
            .or_insert_with(|| DropzoneRuleGroup {
                rule_id: rule.id.clone(),
                rule_name: rule.name.clone(),
                mode: rule.mode.clone(),
                action: rule.action.clone(),
                file_paths: Vec::new(),
                file_count: 0,
                total_size_bytes: 0,
            });
        group.file_paths.push(file.path.clone());
        group.file_count += 1;
        group.total_size_bytes += file.size_bytes;
    }

    let mut rule_groups: Vec<DropzoneRuleGroup> = groups.into_values().collect();
    rule_groups.sort_by(|left, right| left.rule_name.cmp(&right.rule_name));

    Ok((rule_groups, preview_only, unmatched_files))
}

#[cfg(test)]
mod tests {
    use crate::models::{RuleAction, RuleMode};
    use crate::storage;
    use crate::storage::test_util::{path_string, Fixture};

    use super::{preview_dropzone_files, ShakeDetector};

    #[test]
    fn shake_detector_triggers_after_reversing_travel_threshold() {
        let mut detector = ShakeDetector::new();
        let mut triggered = false;
        let samples = [
            (0, 0, 0),
            (300, 0, 100),
            (0, 0, 200),
            (300, 0, 300),
            (0, 0, 400),
            (300, 0, 500),
            (0, 0, 600),
        ];

        for (x, y, time) in samples {
            triggered = detector.update(true, x, y, time);
            if triggered {
                break;
            }
        }

        assert!(triggered);
    }

    #[test]
    fn shake_detector_coalesces_same_direction_movement() {
        let mut detector = ShakeDetector::new();
        detector.update(true, 0, 0, 0);
        detector.update(true, 10, 0, 10);
        detector.update(true, 20, 0, 20);

        assert_eq!(detector.moves.len(), 1);
    }

    #[test]
    fn shake_detector_prunes_stale_movement() {
        let mut detector = ShakeDetector::new();
        detector.update(true, 0, 0, 0);
        detector.update(true, 600, 0, 10);
        detector.update(true, 0, 0, 1_200);

        assert!(!detector.update(true, 10, 0, 1_220));
    }

    #[test]
    fn shake_detector_ignores_short_motion() {
        let mut detector = ShakeDetector::new();
        detector.update(true, 0, 0, 0);
        detector.update(true, 100, 0, 100);
        detector.update(true, 0, 0, 200);

        assert!(!detector.update(true, 100, 0, 300));
    }

    #[test]
    fn preview_blocks_lower_priority_executable_rule_globally() {
        let fixture = Fixture::new("shelflife-dropzone-preview");
        let file = fixture.write_outside_file("download.zip", "body");
        fixture.save_config();
        let mut preview = fixture.rule();
        preview.mode = RuleMode::PreviewOnly;
        preview.priority = 50;
        let mut ask = fixture.rule();
        ask.id = String::from("ask");
        ask.name = String::from("Ask zip");
        ask.mode = RuleMode::AskFirst;
        ask.priority = 20;
        storage::rules::save_rule(&fixture.db, &preview).expect("preview rule should save");
        storage::rules::save_rule(&fixture.db, &ask).expect("ask rule should save");

        let result = preview_dropzone_files(&fixture.db, &[path_string(&file)])
            .expect("preview should work");

        assert!(result.rule_groups.is_empty());
        assert_eq!(result.preview_only.len(), 1);
        assert!(result.unmatched_files.is_empty());
    }

    #[test]
    fn preview_skips_external_ignore_rule_as_non_actionable() {
        let fixture = Fixture::new("shelflife-dropzone-ignore");
        let file = fixture.write_outside_file("download.zip", "body");
        fixture.save_config();
        let mut rule = fixture.rule();
        rule.mode = RuleMode::Automatic;
        rule.action = RuleAction::Ignore;
        storage::rules::save_rule(&fixture.db, &rule).expect("rule should save");

        let result = preview_dropzone_files(&fixture.db, &[path_string(&file)])
            .expect("preview should work");

        assert!(result.rule_groups.is_empty());
        assert_eq!(result.preview_only.len(), 1);
        assert_eq!(result.unmatched_files, vec![path_string(&file)]);
    }

    #[test]
    fn preview_excludes_disabled_rules() {
        let fixture = Fixture::new("shelflife-dropzone-disabled");
        let file = fixture.write_outside_file("download.zip", "body");
        fixture.save_config();
        let mut rule = fixture.rule();
        rule.enabled = false;
        rule.mode = RuleMode::AskFirst;
        storage::rules::save_rule(&fixture.db, &rule).expect("rule should save");

        let result = preview_dropzone_files(&fixture.db, &[path_string(&file)])
            .expect("preview should work");

        assert!(result.rule_groups.is_empty());
        assert!(result.preview_only.is_empty());
        assert_eq!(result.unmatched_files, vec![path_string(&file)]);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn preview_uses_zone_identifier_for_source_domain_rules() {
        let fixture = Fixture::new("shelflife-dropzone-origin");
        let file = fixture.write_outside_file("download.zip", "body");
        let ads_path = format!("{}:Zone.Identifier:$DATA", file.to_string_lossy());
        std::fs::write(
            ads_path,
            "[ZoneTransfer]\nZoneId=3\nHostUrl=https://downloads.example.com/download.zip\n",
        )
        .expect("Zone.Identifier should be written");
        fixture.save_config();

        let mut rule = fixture.rule();
        rule.mode = RuleMode::AskFirst;
        rule.conditions.source_domains = vec![String::from("example.com")];
        storage::rules::save_rule(&fixture.db, &rule).expect("rule should save");

        let result = preview_dropzone_files(&fixture.db, &[path_string(&file)])
            .expect("preview should work");

        assert_eq!(result.rule_groups.len(), 1);
        assert_eq!(result.rule_groups[0].rule_id, rule.id);
        assert!(result.preview_only.is_empty());
        assert!(result.unmatched_files.is_empty());
    }

    #[test]
    fn preview_rejects_folders() {
        let fixture = Fixture::new("shelflife-dropzone-folder");
        fixture.save_config();

        let result = preview_dropzone_files(&fixture.db, &[path_string(&fixture.outside)])
            .expect("preview should work");

        assert_eq!(result.rejected_files.len(), 1);
    }
}
