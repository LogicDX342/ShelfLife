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
  let error = $state<string | null>(null);
  let testResults = $state<RuleMatchExplanation[]>([]);
  async function browseWatchPath() {
    try {
      const selected = await selectDirectory('Select Watch Target Path', watchPath);
      if (selected) {
        watchPath = selected;
      }
    } catch (reason) {
      error = getErrorMessage(reason, 'Could not select folder.');
    }
  }

  async function browseDestinationPath() {
    try {
      const selected = await selectDirectory('Select Destination Path', destinationPath);
      if (selected) {
        destinationPath = selected;
      }
    } catch (reason) {
      error = getErrorMessage(reason, 'Could not select folder.');
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
    error = null;
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
    error = null;
    try {
      await saveRule(buildRule());
      if (!rule) reset();
      await onSaved();
    } catch (reason) {
      error = getErrorMessage(reason, 'Could not save rule.');
    } finally {
      saving = false;
    }
  }

  async function preview() {
    testing = true;
    error = null;
    try {
      testResults = await testRule(buildRule());
    } catch (reason) {
      error = getErrorMessage(reason, 'Could not test rule.');
    } finally {
      testing = false;
    }
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
      General Settings
    </h4>
    <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
      <label class="flex flex-col gap-1">
        <span class="text-xs font-medium text-fluent-muted-light dark:text-fluent-muted-dark"
          >Rule Name</span
        >
        <input
          bind:value={name}
          required
          placeholder="e.g. Clean Temporary Downloads"
          class="fluent-input"
        />
      </label>

      <label class="flex flex-col gap-1">
        <span class="text-xs font-medium text-fluent-muted-light dark:text-fluent-muted-dark"
          >Watch Target Path</span
        >
        <div class="flex gap-2">
          <input
            bind:value={watchPath}
            required
            placeholder="C:\Users\Name\Downloads"
            class="fluent-input flex-1 min-w-0"
          />
          <button
            type="button"
            class="fluent-button text-xs font-semibold px-3 flex-shrink-0"
            onclick={browseWatchPath}
          >
            Browse...
          </button>
        </div>
      </label>

      <div class="grid grid-cols-2 gap-2">
        <label class="flex flex-col gap-1 {actionKind === 'Ignore' ? 'col-span-2' : ''}">
          <span class="text-xs font-medium text-fluent-muted-light dark:text-fluent-muted-dark"
            >{i18n.t('rules.priority')}</span
          >
          <input type="number" bind:value={priority} class="fluent-input" />
        </label>
        {#if actionKind !== 'Ignore'}
          <label class="flex flex-col gap-1">
            <span class="text-xs font-medium text-fluent-muted-light dark:text-fluent-muted-dark"
              >TTL (Days)</span
            >
            <input min="1" type="number" bind:value={ttlDays} class="fluent-input" />
          </label>
        {/if}
      </div>

      <label class="flex flex-col gap-1">
        <span class="text-xs font-medium text-fluent-muted-light dark:text-fluent-muted-dark"
          >{i18n.t('rules.mode')}</span
        >
        <select bind:value={mode} class="fluent-input">
          <option value="PreviewOnly">PreviewOnly</option>
          <option value="AskFirst">AskFirst</option>
          <option value="Automatic">Automatic</option>
        </select>
      </label>

      <div class="flex items-center gap-3 pt-6 select-none">
        <span class="text-xs font-medium text-fluent-muted-light dark:text-fluent-muted-dark"
          >{i18n.t('rules.enabled')}</span
        >
        <label class="fluent-switch">
          <input
            type="checkbox"
            class="fluent-switch-input"
            checked={enabled}
            onchange={() => (enabled = !enabled)}
          />
          <span class="fluent-switch-track">
            <span class="fluent-switch-thumb"></span>
          </span>
        </label>
      </div>
    </div>
  </div>

  <!-- Section 2: Match Conditions -->
  <div class="space-y-3">
    <h4
      class="text-xs font-semibold text-fluent-accent uppercase tracking-wider border-b border-fluent-border-light dark:border-fluent-border-dark pb-1"
    >
      Match Conditions
    </h4>
    <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
      <label class="flex flex-col gap-1">
        <span class="text-xs font-medium text-fluent-muted-light dark:text-fluent-muted-dark"
          >Extensions</span
        >
        <input
          bind:value={extensions}
          placeholder="pdf, zip, png (comma-sep)"
          class="fluent-input"
        />
      </label>

      <label class="flex flex-col gap-1">
        <span class="text-xs font-medium text-fluent-muted-light dark:text-fluent-muted-dark"
          >Filename Globs</span
        >
        <input bind:value={filenameGlobs} placeholder="*.tmp, temp_*" class="fluent-input" />
      </label>

      <label class="flex flex-col gap-1">
        <span class="text-xs font-medium text-fluent-muted-light dark:text-fluent-muted-dark"
          >Filename Regexes</span
        >
        <input bind:value={filenameRegexes} placeholder="(?i)copy" class="fluent-input" />
      </label>

      <label class="flex flex-col gap-1">
        <span class="text-xs font-medium text-fluent-muted-light dark:text-fluent-muted-dark"
          >Source Domains</span
        >
        <input
          bind:value={sourceDomains}
          placeholder="google.com, github.com"
          class="fluent-input"
        />
      </label>
    </div>

    <!-- Size Match Grid -->
    <div class="grid grid-cols-1 md:grid-cols-3 gap-4 pt-2">
      <label class="flex flex-col gap-1">
        <span class="text-xs font-medium text-fluent-muted-light dark:text-fluent-muted-dark"
          >File Size Criteria</span
        >
        <select bind:value={sizeKind} class="fluent-input">
          <option value="Any">Any Size</option>
          <option value="LessThan">Less Than</option>
          <option value="GreaterThan">Greater Than</option>
          <option value="Between">Between</option>
        </select>
      </label>

      {#if sizeKind === 'GreaterThan' || sizeKind === 'Between'}
        <label class="flex flex-col gap-1">
          <span class="text-xs font-medium text-fluent-muted-light dark:text-fluent-muted-dark"
            >Min Size (MB)</span
          >
          <input min="0" type="number" bind:value={sizeMinMb} class="fluent-input" />
        </label>
      {/if}

      {#if sizeKind === 'LessThan' || sizeKind === 'Between'}
        <label class="flex flex-col gap-1">
          <span class="text-xs font-medium text-fluent-muted-light dark:text-fluent-muted-dark"
            >Max Size (MB)</span
          >
          <input min="0" type="number" bind:value={sizeMaxMb} class="fluent-input" />
        </label>
      {/if}
    </div>
  </div>

  <!-- Section 3: Action Execution -->
  <div class="space-y-3">
    <h4
      class="text-xs font-semibold text-fluent-accent uppercase tracking-wider border-b border-fluent-border-light dark:border-fluent-border-dark pb-1"
    >
      Triage Action
    </h4>
    <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
      <label class="flex flex-col gap-1">
        <span class="text-xs font-medium text-fluent-muted-light dark:text-fluent-muted-dark"
          >{i18n.t('rules.action')}</span
        >
        <select bind:value={actionKind} class="fluent-input">
          <option value="Ignore">Ignore (Keep in place)</option>
          <option value="Trash">Trash Now</option>
          <option value="Move">Move to Directory</option>
          <option value="Rename">Rename File</option>
        </select>
      </label>

      {#if actionKind === 'Move'}
        <label class="flex flex-col gap-1">
          <span class="text-xs font-medium text-fluent-muted-light dark:text-fluent-muted-dark"
            >Destination Path</span
          >
          <div class="flex gap-2">
            <input
              bind:value={destinationPath}
              placeholder="C:\SafeFolder"
              class="fluent-input flex-1 min-w-0"
              required
            />
            <button
              type="button"
              class="fluent-button text-xs font-semibold px-3 flex-shrink-0"
              onclick={browseDestinationPath}
            >
              Browse...
            </button>
          </div>
        </label>
      {/if}

      {#if actionKind === 'Rename'}
        <label class="flex flex-col gap-1">
          <span class="text-xs font-medium text-fluent-muted-light dark:text-fluent-muted-dark"
            >Rename Template</span
          >
          <input
            bind:value={renameTemplate}
            placeholder="e.g. YYYY-MM-DD_{name}.ext"
            class="fluent-input"
            required
          />
        </label>
      {/if}
    </div>
  </div>

  {#if error}
    <div class="p-3 text-xs rounded bg-red-100 dark:bg-red-950/40 text-red-700 dark:text-red-300">
      {error}
    </div>
  {/if}

  <!-- Footer actions -->
  <div
    class="flex items-center justify-end gap-2 border-t border-fluent-border-light dark:border-fluent-border-dark pt-4"
  >
    <button class="fluent-button" type="button" onclick={preview} disabled={testing}>
      {#if testing}
        Testing...
      {:else}
        {i18n.t('rules.testRule')}
      {/if}
    </button>
    {#if onCancel}
      <button class="fluent-button" type="button" onclick={onCancel}>
        {i18n.t('dialog.no')}
      </button>
    {/if}
    <button class="fluent-button fluent-button-primary" type="submit" disabled={saving}>
      Save Rule
    </button>
  </div>

  <!-- Live Test Panel -->
  {#if testResults.length > 0}
    <div
      class="fluent-card p-4 space-y-3 bg-neutral-50 dark:bg-neutral-900/40 border border-fluent-accent/10"
    >
      <h5 class="text-xs font-semibold text-fluent-text-light dark:text-fluent-text-dark">
        Dry Run Match Results ({testResults.length} files matched)
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
    </div>
  {/if}
</form>
