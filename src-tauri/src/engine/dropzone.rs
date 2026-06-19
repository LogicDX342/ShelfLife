use std::cmp::Reverse;
use std::collections::{HashMap, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};

use redb::Database;

use crate::engine::freshness::tracked_file_from_metadata;
use crate::engine::paths::PathScope;
use crate::models::{
    AppConfig, AppError, AutomationRule, DropzoneFile, DropzonePreview, DropzoneRejectedFile,
    DropzoneRuleGroup, OriginEvidence, RuleAction, RuleMatchExplanation, RuleMode, TrackedFile,
};
use crate::rules::conditions::evaluate_conditions;
use crate::rules::explanation::{protected_explanation, rule_explanation};
use crate::rules::protected_pattern_match;
use crate::storage;

pub const SHAKE_INTERVAL_MS: u64 = 1_000;
pub const SHAKE_MINIMUM_DISTANCE: f64 = 1_000.0;
pub const SHAKE_FACTOR: f64 = 4.0;

#[derive(Debug, Clone, Copy)]
struct PointerPoint {
    x: i32,
    y: i32,
    time_ms: u64,
}

#[derive(Debug, Clone, Copy)]
struct PointerMove {
    dx: i32,
    dy: i32,
    time_ms: u64,
}

#[derive(Debug, Default)]
pub struct ShakeDetector {
    last_point: Option<PointerPoint>,
    points: VecDeque<PointerPoint>,
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

        let point = PointerPoint { x, y, time_ms };
        let Some(previous) = self.last_point.replace(point) else {
            self.points.push_back(point);
            return false;
        };

        let dx = x - previous.x;
        let dy = y - previous.y;
        if dx == 0 && dy == 0 {
            self.prune(time_ms);
            return false;
        }

        self.points.push_back(point);
        self.push_move(PointerMove { dx, dy, time_ms });
        self.prune(time_ms);

        if self.is_shaking() {
            self.reset();
            return true;
        }

        false
    }

    pub fn reset(&mut self) {
        self.last_point = None;
        self.points.clear();
        self.moves.clear();
    }

    #[cfg(test)]
    fn movement_count(&self) -> usize {
        self.moves.len()
    }

    fn push_move(&mut self, movement: PointerMove) {
        if let Some(last) = self.moves.back_mut() {
            if same_direction(*last, movement) {
                last.dx += movement.dx;
                last.dy += movement.dy;
                last.time_ms = movement.time_ms;
                return;
            }
        }

        self.moves.push_back(movement);
    }

    fn prune(&mut self, time_ms: u64) {
        let earliest = time_ms.saturating_sub(SHAKE_INTERVAL_MS);
        while self
            .points
            .front()
            .is_some_and(|point| point.time_ms < earliest)
        {
            self.points.pop_front();
        }
        while self
            .moves
            .front()
            .is_some_and(|movement| movement.time_ms < earliest)
        {
            self.moves.pop_front();
        }
    }

    fn is_shaking(&self) -> bool {
        if self.moves.is_empty() || self.points.len() < 2 {
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

        let min_x = self.points.iter().map(|point| point.x).min().unwrap_or(0);
        let max_x = self.points.iter().map(|point| point.x).max().unwrap_or(0);
        let min_y = self.points.iter().map(|point| point.y).min().unwrap_or(0);
        let max_y = self.points.iter().map(|point| point.y).max().unwrap_or(0);
        let diagonal = distance(max_x - min_x, max_y - min_y).max(1.0);

        total_distance >= diagonal * SHAKE_FACTOR
    }
}

fn same_direction(left: PointerMove, right: PointerMove) -> bool {
    let dot_product = left.dx * right.dx + left.dy * right.dy;
    dot_product > 0
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
    let mut tracked = tracked_file_from_metadata(
        &source,
        &metadata,
        None,
        config,
        config.default_ttl_seconds,
        "",
    );
    tracked.origin = OriginEvidence::Unknown;

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
    let mut enabled_rules: Vec<&AutomationRule> =
        rules.iter().filter(|rule| rule.enabled).collect();
    enabled_rules.sort_by_key(|rule| Reverse(rule.priority));

    let mut groups: HashMap<String, DropzoneRuleGroup> = HashMap::new();
    let mut preview_only = Vec::new();
    let mut unmatched_files = Vec::new();
    let scope = PathScope::new(config);

    for file in files {
        if let Some(pattern) = protected_pattern_match(&file.file_name, &config.protected_patterns)?
        {
            preview_only.push(protected_explanation(&file.path, file.size_bytes, pattern));
            unmatched_files.push(file.path.clone());
            continue;
        }

        let mut selected_rule = None;
        let mut saw_preview_only = false;
        for rule in &enabled_rules {
            let condition_match = evaluate_conditions(
                &file.file_name,
                file.size_bytes,
                &file.origin,
                &rule.conditions,
            )?;
            if !condition_match.matched {
                continue;
            }

            let mut explanation =
                rule_explanation(&file.path, file.size_bytes, rule, condition_match);
            if matches!(rule.mode, RuleMode::PreviewOnly) {
                saw_preview_only = true;
                preview_only.push(explanation);
                continue;
            }

            if matches!(rule.action, RuleAction::Ignore)
                && !scope.is_in_enabled_watch_target(Path::new(&file.path))
            {
                explanation.proposed_action = None;
                explanation.message = String::from(
                    "Dropzone skipped Ignore because this file is outside watch targets.",
                );
                preview_only.push(explanation);
                continue;
            }

            selected_rule = Some(rule);
            break;
        }

        if let Some(rule) = selected_rule {
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
        } else if !saw_preview_only {
            unmatched_files.push(file.path.clone());
        }
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

        assert_eq!(detector.movement_count(), 1);
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
    fn preview_groups_highest_priority_executable_rule_globally() {
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

        assert_eq!(result.rule_groups.len(), 1);
        assert_eq!(result.rule_groups[0].rule_id, "ask");
        assert_eq!(result.preview_only.len(), 1);
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

    #[test]
    fn preview_rejects_folders() {
        let fixture = Fixture::new("shelflife-dropzone-folder");
        fixture.save_config();

        let result = preview_dropzone_files(&fixture.db, &[path_string(&fixture.outside)])
            .expect("preview should work");

        assert_eq!(result.rejected_files.len(), 1);
    }
}
