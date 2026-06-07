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
  | { Move: { destination_path: string } }
  | { Rename: { template: string } }
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
  blocked_by_protected_pattern: string | null;
  proposed_action: RuleAction | null;
  mode: RuleMode | null;
  message: string;
};

export type UndoStatus =
  | 'Available'
  | 'Completed'
  | { Unavailable: { reason: string } }
  | { Failed: { reason: string } };

export type AuditActionKind =
  | 'Trash'
  | 'Move'
  | 'Rename'
  | 'Pin'
  | 'Snooze'
  | 'Ignore'
  | 'RulePreview';

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
  | { Move: { destination_path: string } }
  | 'TrashNow'
  | { Rename: { template: string } };

export type WatchTarget = {
  id: string;
  path: string;
  enabled: boolean;
  recursive: boolean;
  default_ttl_seconds: number | null;
  ignore_patterns: string[];
  rule_ids: string[];
};

export type AppConfig = {
  version: number;
  watch_targets: WatchTarget[];
  protected_patterns: string[];
  default_ttl_seconds: number;
  stale_threshold_seconds: number;
  decaying_threshold_seconds: number;
  safe_folder_path: string;
  notifications_enabled: boolean;
  start_at_login: boolean;
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
