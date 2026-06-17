<script lang="ts">
  import { selectDirectory } from '$lib/api/files';
  import { saveRule, testRule } from '$lib/api/rules';
  import { Button } from '$lib/components/ui/button';
  import * as Card from '$lib/components/ui/card';
  import { Input } from '$lib/components/ui/input';
  import { Label } from '$lib/components/ui/label';
  import * as Select from '$lib/components/ui/select';
  import { i18n } from '$lib/i18n/i18n.svelte';
  import { notifications } from '$lib/stores/notifications.svelte';
  import type {
    AutomationRule,
    RuleAction,
    RuleMatchExplanation,
    RuleMode,
    SizeCondition,
  } from '$lib/types';
  import { formatBytes, getErrorMessage } from '$lib/utils/format';

  let {
    onSaved,
    rule = null,
    onCancel,
  } = $props<{
    onSaved: () => Promise<void>;
    rule?: AutomationRule | null;
    onCancel: () => void;
  }>();

  let name = $state('');
  let enabled = $state(true);
  let watchPath = $state('');
  let priority = $state(0);
  let ttlDays = $state(30);
  let mode = $state<RuleMode>('PreviewOnly');
  let actionKind = $state<'Ignore' | 'Trash' | 'Move'>('Ignore');
  let destinationFolder = $state('');
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
      const selected = await selectDirectory('Select Destination Folder', destinationFolder);
      if (selected) {
        destinationFolder = selected;
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
    if (actionKind === 'Move')
      return {
        Move: {
          destination_folder: destinationFolder,
          rename_template: renameTemplate.trim() ? renameTemplate : null,
        },
      };
    return 'Ignore';
  }

  function actionKindFromRule(action: RuleAction) {
    if (action === 'Trash') return 'Trash';
    if (action === 'Ignore') return 'Ignore';
    if ('Move' in action) return 'Move';
    return 'Ignore';
  }

  function applyRule(next: AutomationRule | null) {
    name = next?.name ?? '';
    enabled = next?.enabled ?? true;
    watchPath = next?.watch_path ?? '';
    priority = next?.priority ?? 0;
    ttlDays = next ? Math.max(1, Math.round(next.ttl_seconds / 86400)) : 30;
    mode = next?.mode ?? 'PreviewOnly';
    actionKind = next ? actionKindFromRule(next.action) : 'Ignore';
    destinationFolder =
      next && typeof next.action === 'object' && 'Move' in next.action
        ? next.action.Move.destination_folder
        : '';
    renameTemplate =
      next && typeof next.action === 'object' && 'Move' in next.action
        ? (next.action.Move.rename_template ?? '')
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
  <Card.Root>
    <Card.Content class="space-y-4">
      <div class="flex items-center justify-between border-b pb-2">
        <h3 class="text-sm font-semibold text-primary">
          {i18n.t('rules.generalSettings')}
        </h3>
      </div>
      <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
        <div class="flex flex-col gap-1.5">
          <Label for="rule-name">{i18n.t('rules.ruleName')}</Label>
          <Input
            id="rule-name"
            bind:value={name}
            required
            placeholder={i18n.t('rules.ruleNamePlaceholder')}
          />
        </div>

        <div class="flex flex-col gap-1.5">
          <Label for="watch-path">{i18n.t('rules.watchTargetPath')}</Label>
          <div class="flex gap-2 w-full">
            <Input
              id="watch-path"
              bind:value={watchPath}
              required
              placeholder="C:\Users\Name\Downloads"
            />
            <Button type="button" variant="outline" onclick={browseWatchPath}>
              {i18n.t('settings.browse')}
            </Button>
          </div>
        </div>

        <div class="flex flex-col gap-1.5">
          <Label for="rule-priority">{i18n.t('rules.priority')}</Label>
          <Input id="rule-priority" type="number" bind:value={priority} />
        </div>

        <div class="flex flex-col gap-1.5">
          <Label for="rule-mode">{i18n.t('rules.mode')}</Label>
          <Select.Root type="single" bind:value={mode}>
            <Select.Trigger id="rule-mode" class="w-full">
              <span data-slot="select-value">{modeLabel(mode)}</span>
            </Select.Trigger>
            <Select.Content>
              <Select.Item value="PreviewOnly" label={i18n.t('rules.modePreviewOnly')} />
              <Select.Item value="AskFirst" label={i18n.t('rules.modeAskFirst')} />
              <Select.Item value="Automatic" label={i18n.t('rules.modeAutomatic')} />
            </Select.Content>
          </Select.Root>
        </div>
      </div>
    </Card.Content>
  </Card.Root>

  <!-- Section 2: Match Conditions -->
  <Card.Root>
    <Card.Content class="space-y-4">
      <div class="flex items-center justify-between border-b pb-2">
        <h3 class="text-sm font-semibold text-primary">
          {i18n.t('rules.matchConditions')}
        </h3>
      </div>
      <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
        <div class="flex flex-col gap-1.5">
          <Label for="extensions">{i18n.t('rules.extensions')}</Label>
          <Input
            id="extensions"
            bind:value={extensions}
            placeholder={i18n.t('rules.extensionsPlaceholder')}
          />
        </div>

        <div class="flex flex-col gap-1.5">
          <Label for="filename-globs">{i18n.t('rules.filenameGlobs')}</Label>
          <Input
            id="filename-globs"
            bind:value={filenameGlobs}
            placeholder={i18n.t('rules.filenameGlobsPlaceholder')}
          />
        </div>

        <div class="flex flex-col gap-1.5">
          <Label for="filename-regexes">{i18n.t('rules.filenameRegexes')}</Label>
          <Input
            id="filename-regexes"
            bind:value={filenameRegexes}
            placeholder={i18n.t('rules.filenameRegexesPlaceholder')}
          />
        </div>

        <div class="flex flex-col gap-1.5">
          <Label for="source-domains">{i18n.t('rules.sourceDomains')}</Label>
          <Input
            id="source-domains"
            bind:value={sourceDomains}
            placeholder={i18n.t('rules.sourceDomainsPlaceholder')}
          />
        </div>
        <!-- </div> -->

        <!-- Size Match Grid -->
        <!-- <div class="grid grid-cols-1 md:grid-cols-2 gap-4 pt-2"> -->
        <div class="flex flex-col gap-1.5">
          <Label for="size-criteria">{i18n.t('rules.fileSizeCriteria')}</Label>
          <Select.Root type="single" bind:value={sizeKind}>
            <Select.Trigger id="size-criteria" class="w-full">
              <span data-slot="select-value">{sizeKindLabel(sizeKind)}</span>
            </Select.Trigger>
            <Select.Content>
              <Select.Item value="Any" label={i18n.t('rules.anySize')} />
              <Select.Item value="LessThan" label={i18n.t('rules.lessThan')} />
              <Select.Item value="GreaterThan" label={i18n.t('rules.greaterThan')} />
              <Select.Item value="Between" label={i18n.t('rules.between')} />
            </Select.Content>
          </Select.Root>
        </div>

        <div class="grid grid-cols-2 gap-2">
          <div class="flex flex-col gap-1.5">
            <Label for="min-size-mb">{i18n.t('rules.minSizeMb')}</Label>
            <Input
              id="min-size-mb"
              min="0"
              type="number"
              bind:value={sizeMinMb}
              disabled={sizeKind === 'LessThan' || sizeKind === 'Any'}
            />
          </div>

          <div class="flex flex-col gap-1.5">
            <Label for="max-size-mb">{i18n.t('rules.maxSizeMb')}</Label>
            <Input
              id="max-size-mb"
              min="0"
              type="number"
              bind:value={sizeMaxMb}
              disabled={sizeKind === 'GreaterThan' || sizeKind === 'Any'}
            />
          </div>
        </div>
      </div>
    </Card.Content>
  </Card.Root>

  <!-- Section 3: Action Execution -->
  <Card.Root>
    <Card.Content class="space-y-4">
      <div class="flex items-center justify-between border-b pb-2">
        <h3 class="text-sm font-semibold text-primary">
          {i18n.t('rules.action')}
        </h3>
      </div>
      <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
        <div class="flex flex-col gap-1.5">
          <Label for="action-kind">{i18n.t('rules.action')}</Label>
          <Select.Root type="single" bind:value={actionKind}>
            <Select.Trigger id="action-kind" class="w-full">
              <span data-slot="select-value">{actionKindLabel(actionKind)}</span>
            </Select.Trigger>
            <Select.Content>
              <Select.Item value="Ignore" label={i18n.t('rules.actionIgnoreLabel')} />
              <Select.Item value="Trash" label={i18n.t('file.trash')} />
              <Select.Item value="Move" label={i18n.t('rules.actionMoveLabel')} />
            </Select.Content>
          </Select.Root>
        </div>

        <div class="flex flex-col gap-1.5">
          <Label for="ttl-days">{i18n.t('rules.ttlDaysLabel')}</Label>
          <Input
            id="ttl-days"
            min="1"
            type="number"
            bind:value={ttlDays}
            disabled={actionKind === 'Ignore'}
          />
        </div>

        <div class="flex flex-col gap-1.5">
          <Label for="destination-path">{i18n.t('rules.destinationPath')}</Label>
          <div class="flex gap-2 w-full">
            <Input
              id="destination-path"
              bind:value={destinationFolder}
              placeholder="C:\SafeFolder"
              required={actionKind === 'Move'}
              disabled={actionKind !== 'Move'}
            />
            <Button
              type="button"
              variant="outline"
              onclick={browseDestinationPath}
              disabled={actionKind !== 'Move'}
            >
              {i18n.t('settings.browse')}
            </Button>
          </div>
        </div>
        <div class="flex flex-col gap-1.5">
          <Label for="rename-template">{i18n.t('rules.renameTemplate')}</Label>
          <Input
            id="rename-template"
            bind:value={renameTemplate}
            placeholder={'{date}-{name}.{ext}'}
            disabled={actionKind !== 'Move'}
          />
        </div>
      </div>
    </Card.Content>
  </Card.Root>

  <!-- Footer actions -->
  <div class="flex items-center justify-end gap-2 border-t pt-4">
    <Button variant="outline" type="button" onclick={preview} disabled={testing}>
      {#if testing}
        Testing...
      {:else}
        {i18n.t('rules.testRule')}
      {/if}
    </Button>
    <Button variant="outline" type="button" onclick={onCancel}>
      {i18n.t('dialog.cancel')}
    </Button>
    <Button type="submit" disabled={saving}>
      {i18n.t('rules.saveRule')}
    </Button>
  </div>

  <!-- Live Test Panel -->
  {#if testResults.length > 0}
    <Card.Root>
      <Card.Content class="space-y-4">
        <h5 class="text-xs font-semibold">
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
                <span class="text-xs text-muted-foreground flex-shrink-0">
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
