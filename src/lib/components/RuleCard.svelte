<script lang="ts">
  import { i18n } from '$lib/i18n/i18n.svelte';
  import type { AutomationRule } from '$lib/types';

  let {
    rule,
    testing = false,
    onEdit,
    onTest,
    onDelete,
    onToggleEnabled,
  } = $props<{
    rule: AutomationRule;
    testing?: boolean;
    onEdit: (rule: AutomationRule) => void;
    onTest: (rule: AutomationRule) => void;
    onDelete: (rule: AutomationRule) => void;
    onToggleEnabled: (rule: AutomationRule) => void;
  }>();
</script>

<div
  class="fluent-card flex flex-col md:flex-row md:items-center justify-between gap-4 p-4 hover:border-fluent-accent/30"
>
  <div class="space-y-1 min-w-0 flex-1">
    <div class="flex items-center gap-2.5">
      <span
        class="font-semibold text-sm tracking-tight text-fluent-text-light dark:text-fluent-text-dark"
      >
        {rule.name}
      </span>
      <span
        class="text-[10px] uppercase font-bold tracking-wider px-2 py-0.5 rounded bg-black/5 dark:bg-white/5 text-fluent-muted-light dark:text-fluent-muted-dark"
      >
        {i18n.t('rules.priority')}: {rule.priority}
      </span>
    </div>

    <p
      class="text-xs text-fluent-muted-light dark:text-fluent-muted-dark truncate"
      title={rule.watch_path}
    >
      {i18n.t('rules.watchTarget', { path: rule.watch_path })}
    </p>

    <div
      class="flex flex-wrap items-center gap-1.5 pt-1 text-[10px] font-medium text-fluent-muted-light dark:text-fluent-muted-dark"
    >
      <span
        class="px-1.5 py-0.5 rounded bg-blue-100 dark:bg-blue-950/40 text-blue-700 dark:text-blue-300"
      >
        {i18n.t('rules.mode')}: {rule.mode}
      </span>
      {#if typeof rule.action === 'string'}
        <span
          class="px-1.5 py-0.5 rounded bg-purple-100 dark:bg-purple-950/40 text-purple-700 dark:text-purple-300"
        >
          {i18n.t('rules.action')}: {rule.action}
        </span>
      {:else if 'Move' in rule.action}
        <span
          class="px-1.5 py-0.5 rounded bg-purple-100 dark:bg-purple-950/40 text-purple-700 dark:text-purple-300"
        >
          {i18n.t('rules.action')}: {rule.action.Move.destination_path.split('/').pop() ||
            rule.action.Move.destination_path}
        </span>
      {:else if 'Rename' in rule.action}
        <span
          class="px-1.5 py-0.5 rounded bg-purple-100 dark:bg-purple-950/40 text-purple-700 dark:text-purple-300"
        >
          {i18n.t('rules.action')}: {rule.action.Rename.template}
        </span>
      {/if}
      {#if rule.action !== 'Ignore'}
        <span
          class="px-1.5 py-0.5 rounded bg-neutral-100 dark:bg-neutral-800 text-neutral-600 dark:text-neutral-400"
        >
          {i18n.t('rules.ttlDays', { days: Math.round(rule.ttl_seconds / 86400) })}
        </span>
      {/if}
    </div>
  </div>

  <div class="flex items-center gap-3.5 flex-shrink-0">
    <div class="flex items-center gap-2">
      <span class="text-xs text-fluent-muted-light dark:text-fluent-muted-dark">
        {rule.enabled ? i18n.t('rules.enabled') : i18n.t('rules.disabled')}
      </span>
      <label class="fluent-switch">
        <input
          type="checkbox"
          class="fluent-switch-input"
          checked={rule.enabled}
          onchange={() => onToggleEnabled(rule)}
        />
        <span class="fluent-switch-track">
          <span class="fluent-switch-thumb"></span>
        </span>
      </label>
    </div>

    <div class="flex items-center gap-1.5">
      <button
        class="fluent-button p-1.5 text-xs font-semibold"
        onclick={() => onEdit(rule)}
        aria-label="Edit Rule"
      >
        {i18n.t('rules.edit')}
      </button>
      <button
        class="fluent-button p-1.5 text-xs font-semibold"
        onclick={() => onTest(rule)}
        disabled={testing}
        aria-label="Test Rule"
      >
        {#if testing}
          {i18n.t('rules.testing')}
        {:else}
          {i18n.t('rules.testRule')}
        {/if}
      </button>
      <button
        class="fluent-button p-1.5 text-xs font-semibold text-red-600 dark:text-red-400"
        onclick={() => onDelete(rule)}
        aria-label="Delete Rule"
      >
        {i18n.t('rules.delete')}
      </button>
    </div>
  </div>
</div>
