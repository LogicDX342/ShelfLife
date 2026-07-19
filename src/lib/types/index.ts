export type FileDecayState =
  'Fresh' | 'Stale' | 'Decaying' | 'Pinned' | 'ManuallyIgnored' | 'RuleIgnored';

export type Expiry = { At: number } | 'Permanent' | { SnoozedUntil: number };

export type TrackedFile = {
  path: string;
  file_name: string;
  watch_target_id: string;
  size_bytes: number;
  last_observed_mtime: number | null;
  freshness_at: number;
  expiry: Expiry;
  state: FileDecayState;
  matched_rule_ids: string[];
  origin_url: string | null;
};

export type RuleMode = 'PreviewOnly' | 'AskFirst' | 'Automatic';
export type RuleTiming = 'OnArrival' | { AfterSeconds: number };
export type RuleAction =
  'Trash' | { Move: { destination_folder: string; rename_template: string | null } } | 'Ignore';

export type SizeCondition =
  | 'Any'
  | { LessThan: number }
  | { GreaterThan: number }
  | { Between: { min: number; max: number } };

export type RuleConditions = {
  extensions: string[];
  filename_globs: string[];
  filename_regexes: string[];
  source_domains: string[];
  size: SizeCondition;
};

export type AutomationRule = {
  id: string;
  name: string;
  enabled: boolean;
  priority: number;
  watch_path: string;
  timing: RuleTiming;
  conditions: RuleConditions;
  action: RuleAction;
  mode: RuleMode;
  created_at: number;
  updated_at: number;
};

export type RuleMatchExplanation = {
  file_path: string;
  size_bytes: number | null;
  rule_id: string | null;
  rule_name: string | null;
  matched_extension: boolean;
  matched_size: boolean;
  matched_origin: string | null;
  matched_filename_pattern: string | null;
  proposed_action: RuleAction | null;
  mode: RuleMode | null;
  message: string;
};

export type UndoStatus =
  'Available' | 'Completed' | { Unavailable: { reason: string } } | { Failed: { reason: string } };

export type AuditActionKind = 'Trash' | 'Move' | 'Pin' | 'Snooze' | 'Ignore';

export type AuditEntry = {
  id: string;
  sequence: number;
  timestamp: number;
  action_kind: AuditActionKind;
  source_path: string;
  destination_path: string | null;
  file_name: string;
  size_bytes: number;
  rule_id: string | null;
  rule_name: string | null;
  explanation: RuleMatchExplanation | null;
  undo_status: UndoStatus;
};

export type DropzoneFile = {
  path: string;
  file_name: string;
  size_bytes: number;
};

export type DropzoneRejectedFile = {
  path: string;
  reason: string;
};

export type DropzoneRuleGroup = {
  rule_id: string;
  rule_name: string;
  mode: RuleMode;
  action: RuleAction;
  file_paths: string[];
  file_count: number;
  total_size_bytes: number;
};

export type DropzonePreview = {
  files: DropzoneFile[];
  rejected_files: DropzoneRejectedFile[];
  watch_targets: WatchTarget[];
  rule_groups: DropzoneRuleGroup[];
  preview_only: RuleMatchExplanation[];
  unmatched_files: string[];
};

export type DropzoneActionFailure = {
  path: string;
  error: AppError;
};

export type DropzoneActionResult = {
  entries: AuditEntry[];
  failures: DropzoneActionFailure[];
};

export type BulkTriageFailure = {
  path: string;
  error: AppError;
};

export type BulkTriageResult = {
  entries: AuditEntry[];
  failures: BulkTriageFailure[];
};

export type UserTriageAction =
  | 'Pin'
  | { Snooze: { seconds: number } }
  | 'Ignore'
  | { Move: { destination_folder: string } }
  | 'TrashNow';

export type WatchTarget = {
  id: string;
  path: string;
  enabled: boolean;
  recursive: boolean;
  ignore_patterns: string[];
};

export type CloseBehavior = 'Ask' | 'HideToTray' | 'Quit';

export type TrayLabels = {
  open: string;
  review: string;
  pause: string;
  resume: string;
  reconcile: string;
  preferences: string;
  quit: string;
  tooltip: string;
  tooltip_paused: string;
};

export type AppConfig = {
  watch_targets: WatchTarget[];
  default_ttl_seconds: number;
  stale_threshold_seconds: number;
  decaying_threshold_seconds: number;
  default_move_destination: string | null;
  notifications_enabled: boolean;
  start_at_login: boolean;
  close_behavior: CloseBehavior;
  dropzone_enabled: boolean;
};

export type AppError = {
  code: string;
  message: string;
  recoverable: boolean;
  details: string | null;
};

export type AppUpdate = {
  version: string;
  current_version: string;
};

export type AppUpdateEvent =
  | { event: 'Progress'; data: { chunkLength: number; contentLength: number | null } }
  | { event: 'Finished' };
