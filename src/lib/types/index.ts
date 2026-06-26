export type FileDecayState = 'Fresh' | 'Stale' | 'Decaying' | 'Pinned' | 'Ignored' | 'Missing';

export type Expiry = { At: number } | 'Permanent' | { SnoozedUntil: number };

export type OriginEvidence =
  | { MacWhereFroms: { values: string[] } }
  | {
      WindowsZoneIdentifier: {
        zone_id: number | null;
        host_url: string | null;
        referrer_url: string | null;
      };
    }
  | { LinuxXattr: { key: string; value_utf8: string | null } }
  | 'Unknown';

export type TrackedFile = {
  path: string;
  file_name: string;
  watch_target_id: string;
  size_bytes: number;
  first_seen_at: number;
  last_observed_mtime: number | null;
  last_observed_atime: number | null;
  last_user_action_at: number | null;
  freshness_at: number;
  expiry: Expiry;
  state: FileDecayState;
  matched_rule_ids: string[];
  origin: OriginEvidence;
};

export type RuleMode = 'PreviewOnly' | 'AskFirst' | 'Automatic';
export type RuleAction =
  | 'Trash'
  | { Move: { destination_folder: string; rename_template: string | null } }
  | 'Ignore';

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
  ttl_seconds: number;
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
  | 'Available'
  | 'Completed'
  | { Unavailable: { reason: string } }
  | { Failed: { reason: string } };

export type AuditActionKind = 'Trash' | 'Move' | 'Pin' | 'Snooze' | 'Ignore' | 'RulePreview';

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
  | 'MoveToSafeFolder'
  | { Move: { destination_folder: string } }
  | 'TrashNow';

export type WatchTarget = {
  id: string;
  path: string;
  enabled: boolean;
  recursive: boolean;
  ignore_patterns: string[];
  include_hidden_patterns: string[];
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
  safe_folder_path: string;
  notifications_enabled: boolean;
  start_at_login: boolean;
  close_behavior: CloseBehavior;
  dropzone_enabled: boolean;
};

export type FilePreviewContent =
  | { Text: { snippet: string; truncated: boolean } }
  | { Image: { width: number; height: number; format: string; thumbnail_path: string | null } }
  | { Pdf: { page_count: number | null; title: string | null } }
  | 'Unknown';

export type FilePreview = {
  path: string;
  file_name: string;
  size_bytes: number;
  mime_type: string | null;
  content: FilePreviewContent;
};

export type AppError = {
  code: string;
  message: string;
  recoverable: boolean;
  details: string | null;
};
