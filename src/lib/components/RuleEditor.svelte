<script lang="ts">
  import { selectDirectory } from '$lib/api/files';
  import { saveRule, testRule } from '$lib/api/rules';
  import { i18n } from '$lib/i18n/i18n.svelte';
  import type {
    AutomationRule,
    RuleAction,
    RuleMatchExplanation,
    RuleMode,
    SizeCondition,
  } from '$lib/types';
  import { formatBytes, getErrorMessage } from '$lib/utils/format';
  import { notifications } from '$lib/stores/notifications.svelte';
  import { Button } from '$lib/components/ui/button';
  import * as Card from '$lib/components/ui/card';
  import { Input } from '$lib/components/ui/input';
  import * as Select from '$lib/components/ui/select';
  import { Switch } from '$lib/components/ui/switch';

  let {
    onSaved,
    rule = null,
    onCancel = null,
  } = $props<{
    onSaved: () => Promise<void>;
    rule?: AutomationRule | null;
    onCancel?: (() => void) | null;
  }>();

  let name = $state('');
  let enabled = $state(true);
  let watchPath = $state('');
  let priority = $state(0);
  let ttlDays = $state(30);
  let mode = $state<RuleMode>('PreviewOnly');
  let actionKind = $state<'Ignore' | 'Trash' | 'Move' | 'Rename'>('Ignore');
  let destinationPath = $state('');
  let renameTemplate = $state('');
  let extensions = $state('');
  let filenameGlobs = $state('');
  let filenameRegexes = $state('');
  let sourceDomains = $state('');
  let sizeKind = $state<'Any' | 'LessThan' | 'GreaterThan' | 'Between'>('Any');
  let sizeMinMb = $state(0);
  let sizeMaxMb = $state(0);
  let saving = $state(false);
  let testing = $state(false);
  let testResults = $state<RuleMatchExplanation[]>([]);
  async function browseWatchPath() {
    try {
      const selected = await selectDirectory('Select Watch Target Path', watchPath);
      if (selected) {
        watchPath = selected;
      }
    } catch (reason) {
      notifications.error(getErrorMessage(reason, i18n.t('rules.errorSelectFolder')));
    }
  }

  async function browseDestinationPath() {
    try {
      const selected = await selectDirectory('Select Destination Path', destinationPath);
      if (selected) {
        destinationPath = selected;
      }
    } catch (reason) {
      notifications.error(getErrorMessage(reason, i18n.t('rules.errorSelectFolder')));
    }
  }

  function csv(value: string) {
    return value
      .split(',')
      .map((item) => item.trim())
      .filter(Boolean);
  }

  function mbToBytes(value: number) {
    return Math.max(0, Math.round(value * 1024 * 1024));
  }

  function sizeCondition(): SizeCondition {
    if (sizeKind === 'LessThan') return { LessThan: mbToBytes(sizeMaxMb) };
    if (sizeKind === 'GreaterThan') return { GreaterThan: mbToBytes(sizeMinMb) };
    if (sizeKind === 'Between')
      return { Between: { min: mbToBytes(sizeMinMb), max: mbToBytes(sizeMaxMb) } };
    return 'Any';
  }

  function ruleAction(): RuleAction {
    if (actionKind === 'Trash') return 'Trash';
    if (actionKind === 'Move') return { Move: { destination_path: destinationPath } };
    if (actionKind === 'Rename') return { Rename: { template: renameTemplate } };
    return 'Ignore';
  }

  function actionKindFromRule(action: RuleAction) {
    if (action === 'Trash') return 'Trash';
    if (action === 'Ignore') return 'Ignore';
    if ('Move' in action) return 'Move';
    return 'Rename';
  }

  function applyRule(next: AutomationRule | null) {
    name = next?.name ?? '';
    enabled = next?.enabled ?? true;
    watchPath = next?.watch_path ?? '';
    priority = next?.priority ?? 0;
    ttlDays = next ? Math.max(1, Math.round(next.ttl_seconds / 86400)) : 30;
    mode = next?.mode ?? 'PreviewOnly';
    actionKind = next ? actionKindFromRule(next.action) : 'Ignore';
    destinationPath =
      next && typeof next.action === 'object' && 'Move' in next.action
        ? next.action.Move.destination_path
        : '';
    renameTemplate =
      next && typeof next.action === 'object' && 'Rename' in next.action
        ? next.action.Rename.template
        : '';
    extensions = next?.conditions.extensions.join(', ') ?? '';
    filenameGlobs = next?.conditions.filename_globs.join(', ') ?? '';
    filenameRegexes = next?.conditions.filename_regexes.join(', ') ?? '';
    sourceDomains = next?.conditions.source_domains.join(', ') ?? '';
    sizeKind = 'Any';
    sizeMinMb = 0;
    sizeMaxMb = 0;
    if (next && typeof next.conditions.size === 'object') {
      if ('LessThan' in next.conditions.size) {
        sizeKind = 'LessThan';
        sizeMaxMb = next.conditions.size.LessThan / 1024 / 1024;
      } else if ('GreaterThan' in next.conditions.size) {
        sizeKind = 'GreaterThan';
        sizeMinMb = next.conditions.size.GreaterThan / 1024 / 1024;
      } else {
        sizeKind = 'Between';
        sizeMinMb = next.conditions.size.Between.min / 1024 / 1024;
        sizeMaxMb = next.conditions.size.Between.max / 1024 / 1024;
      }
    }
    testResults = [];
  }

  $effect(() => {
    applyRule(rule);
  });

  function buildRule(): AutomationRule {
    const now = Math.floor(Date.now() / 1000);
    return {
      id: rule?.id ?? '',
      name,
      enabled,
      priority,
      watch_path: watchPath,
      ttl_seconds: Math.max(1, ttlDays) * 24 * 60 * 60,
      conditions: {
        extensions: csv(extensions),
        filename_globs: csv(filenameGlobs),
        filename_regexes: csv(filenameRegexes),
        source_domains: csv(sourceDomains),
        size: sizeCondition(),
      },
      action: ruleAction(),
      mode,
      created_at: rule?.created_at ?? now,
      updated_at: now,
    };
  }

  function reset() {
    applyRule(null);
  }

  async function submit() {
    saving = true;
    try {
      await saveRule(buildRule());
      if (!rule) reset();
      await onSaved();
    } catch (reason) {
      notifications.error(getErrorMessage(reason, i18n.t('rules.errorSaveRule')));
    } finally {
      saving = false;
    }
  }

  async function preview() {
    testing = true;
    try {
      testResults = await testRule(buildRule());
    } catch (reason) {
      notifications.error(getErrorMessage(reason, i18n.t('rules.errorTest')));
    } finally {
      testing = false;
    }
  }

  function modeLabel(value: RuleMode) {
    if (value === 'AskFirst') return i18n.t('rules.modeAskFirst');
    if (value === 'Automatic') return i18n.t('rules.modeAutomatic');
    return i18n.t('rules.modePreviewOnly');
  }

  function sizeKindLabel(value: typeof sizeKind) {
    if (value === 'LessThan') return i18n.t('rules.lessThan');
    if (value === 'GreaterThan') return i18n.t('rules.greaterThan');
    if (value === 'Between') return i18n.t('rules.between');
    return i18n.t('rules.anySize');
  }

  function actionKindLabel(value: typeof actionKind) {
    if (value === 'Trash') return i18n.t('file.trash');
    if (value === 'Move') return i18n.t('rules.actionMoveLabel');
    if (value === 'Rename') return i18n.t('rules.actionRenameLabel');
    return i18n.t('rules.actionIgnoreLabel');
  }
</script>

<form
  class="space-y-6 text-sm"
  onsubmit={(event) => {
    event.preventDefault();
    submit();
  }}
>
  <!-- Section 1: General Settings -->
  <div class="space-y-3">
    <h4
      class="text-xs font-semibold text-fluent-accent uppercase tracking-wider border-b border-fluent-border-light dark:border-fluent-border-dark pb-1"
    >
      {i18n.t('rules.generalSettings')}
    </h4>
    <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
      <label class="flex flex-col gap-1">
        <span class="text-xs font-medium text-fluent-muted-light dark:text-fluent-muted-dark"
          >{i18n.t('rules.ruleName')}</span
        >
        <Input bind:value={name} required placeholder={i18n.t('rules.ruleNamePlaceholder')} />
      </label>

      <label class="flex flex-col gap-1">
        <span class="text-xs font-medium text-fluent-muted-light dark:text-fluent-muted-dark"
          >{i18n.t('rules.watchTargetPath')}</span
        >
        <div class="flex gap-2">
          <Input bind:value={watchPath} required placeholder="C:\Users\Name\Downloads" />
          <Button type="button" variant="outline" onclick={browseWatchPath}>
            {i18n.t('settings.browse')}
          </Button>
        </div>
      </label>

      <div class="grid grid-cols-2 gap-2">
        <label class="flex flex-col gap-1 {actionKind === 'Ignore' ? 'col-span-2' : ''}">
          <span class="text-xs font-medium text-fluent-muted-light dark:text-fluent-muted-dark"
            >{i18n.t('rules.priority')}</span
          >
          <Input type="number" bind:value={priority} />
        </label>
        {#if actionKind !== 'Ignore'}
          <label class="flex flex-col gap-1">
            <span class="text-xs font-medium text-fluent-muted-light dark:text-fluent-muted-dark"
              >{i18n.t('rules.ttlDaysLabel')}</span
            >
            <Input min="1" type="number" bind:value={ttlDays} />
          </label>
        {/if}
      </div>

      <label class="flex flex-col gap-1">
        <span class="text-xs font-medium text-fluent-muted-light dark:text-fluent-muted-dark"
          >{i18n.t('rules.mode')}</span
        >
        <Select.Root type="single" bind:value={mode}>
          <Select.Trigger>
            <span data-slot="select-value">{modeLabel(mode)}</span>
          </Select.Trigger>
          <Select.Content>
            <Select.Item value="PreviewOnly" label={i18n.t('rules.modePreviewOnly')} />
            <Select.Item value="AskFirst" label={i18n.t('rules.modeAskFirst')} />
            <Select.Item value="Automatic" label={i18n.t('rules.modeAutomatic')} />
          </Select.Content>
        </Select.Root>
      </label>

      <div class="flex items-center gap-3 pt-6 select-none">
        <span class="text-xs font-medium text-fluent-muted-light dark:text-fluent-muted-dark"
          >{i18n.t('rules.enabled')}</span
        >
        <Switch bind:checked={enabled} aria-label={i18n.t('rules.enabled')} />
      </div>
    </div>
  </div>

  <!-- Section 2: Match Conditions -->
  <div class="space-y-3">
    <h4
      class="text-xs font-semibold text-fluent-accent uppercase tracking-wider border-b border-fluent-border-light dark:border-fluent-border-dark pb-1"
    >
      {i18n.t('rules.matchConditions')}
    </h4>
    <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
      <label class="flex flex-col gap-1">
        <span class="text-xs font-medium text-fluent-muted-light dark:text-fluent-muted-dark"
          >{i18n.t('rules.extensions')}</span
        >
        <Input bind:value={extensions} placeholder={i18n.t('rules.extensionsPlaceholder')} />
      </label>

      <label class="flex flex-col gap-1">
        <span class="text-xs font-medium text-fluent-muted-light dark:text-fluent-muted-dark"
          >{i18n.t('rules.filenameGlobs')}</span
        >
        <Input bind:value={filenameGlobs} placeholder={i18n.t('rules.filenameGlobsPlaceholder')} />
      </label>

      <label class="flex flex-col gap-1">
        <span class="text-xs font-medium text-fluent-muted-light dark:text-fluent-muted-dark"
          >{i18n.t('rules.filenameRegexes')}</span
        >
        <Input
          bind:value={filenameRegexes}
          placeholder={i18n.t('rules.filenameRegexesPlaceholder')}
        />
      </label>

      <label class="flex flex-col gap-1">
        <span class="text-xs font-medium text-fluent-muted-light dark:text-fluent-muted-dark"
          >{i18n.t('rules.sourceDomains')}</span
        >
        <Input bind:value={sourceDomains} placeholder={i18n.t('rules.sourceDomainsPlaceholder')} />
      </label>
    </div>

    <!-- Size Match Grid -->
    <div class="grid grid-cols-1 md:grid-cols-3 gap-4 pt-2">
      <label class="flex flex-col gap-1">
        <span class="text-xs font-medium text-fluent-muted-light dark:text-fluent-muted-dark"
          >{i18n.t('rules.fileSizeCriteria')}</span
        >
        <Select.Root type="single" bind:value={sizeKind}>
          <Select.Trigger>
            <span data-slot="select-value">{sizeKindLabel(sizeKind)}</span>
          </Select.Trigger>
          <Select.Content>
            <Select.Item value="Any" label={i18n.t('rules.anySize')} />
            <Select.Item value="LessThan" label={i18n.t('rules.lessThan')} />
            <Select.Item value="GreaterThan" label={i18n.t('rules.greaterThan')} />
            <Select.Item value="Between" label={i18n.t('rules.between')} />
          </Select.Content>
        </Select.Root>
      </label>

      {#if sizeKind === 'GreaterThan' || sizeKind === 'Between'}
        <label class="flex flex-col gap-1">
          <span class="text-xs font-medium text-fluent-muted-light dark:text-fluent-muted-dark"
            >{i18n.t('rules.minSizeMb')}</span
          >
          <Input min="0" type="number" bind:value={sizeMinMb} />
        </label>
      {/if}

      {#if sizeKind === 'LessThan' || sizeKind === 'Between'}
        <label class="flex flex-col gap-1">
          <span class="text-xs font-medium text-fluent-muted-light dark:text-fluent-muted-dark"
            >{i18n.t('rules.maxSizeMb')}</span
          >
          <Input min="0" type="number" bind:value={sizeMaxMb} />
        </label>
      {/if}
    </div>
  </div>

  <!-- Section 3: Action Execution -->
  <div class="space-y-3">
    <h4
      class="text-xs font-semibold text-fluent-accent uppercase tracking-wider border-b border-fluent-border-light dark:border-fluent-border-dark pb-1"
    >
      {i18n.t('rules.action')}
    </h4>
    <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
      <label class="flex flex-col gap-1">
        <span class="text-xs font-medium text-fluent-muted-light dark:text-fluent-muted-dark"
          >{i18n.t('rules.action')}</span
        >
        <Select.Root type="single" bind:value={actionKind}>
          <Select.Trigger>
            <span data-slot="select-value">{actionKindLabel(actionKind)}</span>
          </Select.Trigger>
          <Select.Content>
            <Select.Item value="Ignore" label={i18n.t('rules.actionIgnoreLabel')} />
            <Select.Item value="Trash" label={i18n.t('file.trash')} />
            <Select.Item value="Move" label={i18n.t('rules.actionMoveLabel')} />
            <Select.Item value="Rename" label={i18n.t('rules.actionRenameLabel')} />
          </Select.Content>
        </Select.Root>
      </label>

      {#if actionKind === 'Move'}
        <label class="flex flex-col gap-1">
          <span class="text-xs font-medium text-fluent-muted-light dark:text-fluent-muted-dark"
            >{i18n.t('rules.destinationPath')}</span
          >
          <div class="flex gap-2">
            <Input bind:value={destinationPath} placeholder="C:\SafeFolder" required />
            <Button type="button" variant="outline" onclick={browseDestinationPath}>
              {i18n.t('settings.browse')}
            </Button>
          </div>
        </label>
      {/if}

      {#if actionKind === 'Rename'}
        <label class="flex flex-col gap-1">
          <span class="text-xs font-medium text-fluent-muted-light dark:text-fluent-muted-dark"
            >{i18n.t('rules.renameTemplate')}</span
          >
          <Input bind:value={renameTemplate} placeholder="e.g. YYYY-MM-DD_{name}.ext" required />
        </label>
      {/if}
    </div>
  </div>

  <!-- Footer actions -->
  <div
    class="flex items-center justify-end gap-2 border-t border-fluent-border-light dark:border-fluent-border-dark pt-4"
  >
    <Button variant="outline" type="button" onclick={preview} disabled={testing}>
      {#if testing}
        Testing...
      {:else}
        {i18n.t('rules.testRule')}
      {/if}
    </Button>
    {#if onCancel}
      <Button variant="outline" type="button" onclick={onCancel}>
        {i18n.t('dialog.no')}
      </Button>
    {/if}
    <Button type="submit" disabled={saving}>
      {i18n.t('rules.saveRule')}
    </Button>
  </div>

  <!-- Live Test Panel -->
  {#if testResults.length > 0}
    <Card.Root>
      <Card.Content class="space-y-4">
        <h5 class="text-xs font-semibold text-fluent-text-light dark:text-fluent-text-dark">
          {i18n.t('rules.testResultsCount', { count: testResults.length })}
        </h5>
        <div class="flex flex-col gap-2 max-h-48 overflow-y-auto">
          {#each testResults as result (result.file_path)}
            <div
              class="p-2.5 bg-black/5 dark:bg-white/5 rounded text-xs flex justify-between items-center gap-2"
            >
              <span class="truncate font-medium flex-1" title={result.file_path}
                >{result.file_path.split('\\').pop() || result.file_path}</span
              >
              {#if result.size_bytes !== null}
                <span
                  class="text-xs text-fluent-muted-light dark:text-fluent-muted-dark flex-shrink-0"
                >
                  {formatBytes(result.size_bytes)}
                </span>
              {/if}
            </div>
          {/each}
        </div>
      </Card.Content>
    </Card.Root>
  {/if}
</form>
