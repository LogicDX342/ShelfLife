<script lang="ts">
  import { i18n } from '$lib/i18n/i18n.svelte';
  import type { AutomationRule } from '$lib/types';
  import { Badge } from '$lib/components/ui/badge';
  import { Button } from '$lib/components/ui/button';
  import * as Card from '$lib/components/ui/card';
  import { Switch } from '$lib/components/ui/switch';

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

<Card.Root class="flex flex-col justify-between gap-4 rounded-lg p-4 md:flex-row md:items-center">
  <div class="space-y-1 min-w-0 flex-1">
    <div class="flex items-center gap-2.5">
      <span
        class="font-semibold text-sm tracking-tight text-fluent-text-light dark:text-fluent-text-dark"
      >
        {rule.name}
      </span>
      <Badge variant="secondary">
        {i18n.t('rules.priority')}: {rule.priority}
      </Badge>
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
      <Badge variant="outline">
        {i18n.t('rules.mode')}: {rule.mode}
      </Badge>
      {#if typeof rule.action === 'string'}
        <Badge variant="outline">
          {i18n.t('rules.action')}: {rule.action}
        </Badge>
      {:else if 'Move' in rule.action}
        <Badge variant="outline">
          {i18n.t('rules.action')}: {rule.action.Move.destination_path.split('/').pop() ||
            rule.action.Move.destination_path}
        </Badge>
      {:else if 'Rename' in rule.action}
        <Badge variant="outline">
          {i18n.t('rules.action')}: {rule.action.Rename.template}
        </Badge>
      {/if}
      {#if rule.action !== 'Ignore'}
        <Badge variant="secondary">
          {i18n.t('rules.ttlDays', { days: Math.round(rule.ttl_seconds / 86400) })}
        </Badge>
      {/if}
    </div>
  </div>

  <div class="flex items-center gap-3.5 flex-shrink-0">
    <div class="flex items-center gap-2">
      <span class="text-xs text-fluent-muted-light dark:text-fluent-muted-dark">
        {rule.enabled ? i18n.t('rules.enabled') : i18n.t('rules.disabled')}
      </span>
      <Switch
        checked={rule.enabled}
        onclick={() => onToggleEnabled(rule)}
        aria-label={i18n.t('rules.enabled')}
      />
    </div>

    <div class="flex items-center gap-1.5">
      <Button variant="outline" onclick={() => onEdit(rule)} aria-label="Edit Rule">
        {i18n.t('rules.edit')}
      </Button>
      <Button
        variant="outline"
        onclick={() => onTest(rule)}
        disabled={testing}
        aria-label="Test Rule"
      >
        {#if testing}
          {i18n.t('rules.testing')}
        {:else}
          {i18n.t('rules.testRule')}
        {/if}
      </Button>
      <Button variant="destructive" onclick={() => onDelete(rule)} aria-label="Delete Rule">
        {i18n.t('rules.delete')}
      </Button>
    </div>
  </div>
</Card.Root>
