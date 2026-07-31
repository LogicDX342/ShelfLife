import type {
  AppConfig,
  AutomationRule,
  RuleAction,
  RuleConditions,
  RuleMode,
  RuleTiming,
} from '$lib/types';

const DAY_SECONDS = 24 * 60 * 60;

type RuleTemplateAction = 'Trash' | 'MoveToDefaultDestination';

export type RuleTemplate = {
  id:
    | 'sort-documents-on-detection'
    | 'installer-leftovers'
    | 'downloaded-archives'
    | 'large-downloads'
    | 'duplicate-downloads'
    | 'screenshots';
  nameKey: string;
  descriptionKey: string;
  summaryKey: string;
  priority: number;
  timing: RuleTiming;
  mode: RuleMode;
  action: RuleTemplateAction;
  conditions: RuleConditions;
};

export const STARTER_RULE_TEMPLATES: readonly RuleTemplate[] = [
  {
    id: 'sort-documents-on-detection',
    nameKey: 'rules.templates.sortDocuments.name',
    descriptionKey: 'rules.templates.sortDocuments.description',
    summaryKey: 'rules.templates.sortDocuments.summary',
    priority: 60,
    timing: 'OnArrival',
    mode: 'Automatic',
    action: 'MoveToDefaultDestination',
    conditions: {
      extensions: ['pdf', 'doc', 'docx', 'xls', 'xlsx', 'ppt', 'pptx'],
      filename_globs: [],
      filename_regexes: [],
      source_domains: [],
      size: 'Any',
    },
  },
  {
    id: 'installer-leftovers',
    nameKey: 'rules.templates.installers.name',
    descriptionKey: 'rules.templates.installers.description',
    summaryKey: 'rules.templates.installers.summary',
    priority: 50,
    timing: { AfterSeconds: 14 * DAY_SECONDS },
    mode: 'AskFirst',
    action: 'Trash',
    conditions: {
      extensions: ['exe', 'msi', 'msix', 'appx', 'appxbundle', 'msixbundle'],
      filename_globs: [],
      filename_regexes: [],
      source_domains: [],
      size: 'Any',
    },
  },
  {
    id: 'downloaded-archives',
    nameKey: 'rules.templates.archives.name',
    descriptionKey: 'rules.templates.archives.description',
    summaryKey: 'rules.templates.archives.summary',
    priority: 40,
    timing: { AfterSeconds: 30 * DAY_SECONDS },
    mode: 'AskFirst',
    action: 'Trash',
    conditions: {
      extensions: ['zip', '7z', 'rar', 'tar', 'gz', 'tgz', 'bz2', 'xz'],
      filename_globs: [],
      filename_regexes: [],
      source_domains: [],
      size: 'Any',
    },
  },
  {
    id: 'large-downloads',
    nameKey: 'rules.templates.largeDownloads.name',
    descriptionKey: 'rules.templates.largeDownloads.description',
    summaryKey: 'rules.templates.largeDownloads.summary',
    priority: 30,
    timing: { AfterSeconds: 7 * DAY_SECONDS },
    mode: 'AskFirst',
    action: 'Trash',
    conditions: {
      extensions: [],
      filename_globs: [],
      filename_regexes: [],
      source_domains: [],
      size: { GreaterThan: 1024 * 1024 * 1024 },
    },
  },
  {
    id: 'duplicate-downloads',
    nameKey: 'rules.templates.duplicateDownloads.name',
    descriptionKey: 'rules.templates.duplicateDownloads.description',
    summaryKey: 'rules.templates.duplicateDownloads.summary',
    priority: 20,
    timing: { AfterSeconds: 14 * DAY_SECONDS },
    mode: 'AskFirst',
    action: 'Trash',
    conditions: {
      extensions: [],
      filename_globs: [],
      filename_regexes: ['(?i)^.+ \\(\\d+\\)(?:\\.[^.]+)?$'],
      source_domains: [],
      size: 'Any',
    },
  },
  {
    id: 'screenshots',
    nameKey: 'rules.templates.screenshots.name',
    descriptionKey: 'rules.templates.screenshots.description',
    summaryKey: 'rules.templates.screenshots.summary',
    priority: 10,
    timing: { AfterSeconds: 30 * DAY_SECONDS },
    mode: 'AskFirst',
    action: 'Trash',
    conditions: {
      extensions: ['png', 'jpg', 'jpeg', 'gif', 'webp'],
      filename_globs: [],
      filename_regexes: ['(?i)^screen(?:shot| shot)(?:[-_ (].*)?\\.(?:png|jpe?g|gif|webp)$'],
      source_domains: [],
      size: 'Any',
    },
  },
];

export function preferredTemplateWatchPath(config: AppConfig | null): string {
  return config?.watch_targets.find((target) => target.enabled)?.path ?? '';
}

function createTemplateAction(template: RuleTemplate, config: AppConfig | null): RuleAction {
  if (template.action === 'Trash') return 'Trash';

  return {
    Move: {
      destination_folder: config?.default_move_destination ?? '',
      rename_template: null,
    },
  };
}

export function createRuleFromTemplate(
  template: RuleTemplate,
  config: AppConfig | null,
  localizedName: string,
): AutomationRule {
  const now = Math.floor(Date.now() / 1000);

  return {
    id: '',
    name: localizedName,
    enabled: true,
    priority: template.priority,
    watch_path: preferredTemplateWatchPath(config),
    timing: structuredClone(template.timing),
    conditions: structuredClone(template.conditions),
    action: createTemplateAction(template, config),
    mode: template.mode,
    created_at: now,
    updated_at: now,
  };
}
