<script lang="ts">
  import { Badge } from '$lib/components/ui/badge';
  import { Button } from '$lib/components/ui/button';
  import * as Card from '$lib/components/ui/card';
  import { Switch } from '$lib/components/ui/switch';
  import { i18n } from '$lib/i18n/i18n.svelte';
  import type { AutomationRule, RuleAction, RuleMode, RuleTiming } from '$lib/types';

  let { rule, onEdit, onDelete, onToggleEnabled } = $props<{
    rule: AutomationRule;
    onEdit: (rule: AutomationRule) => void;
    onDelete: (rule: AutomationRule) => void;
    onToggleEnabled: (rule: AutomationRule) => void;
  }>();

  function modeLabel(mode: RuleMode) {
    if (mode === 'AskFirst') return i18n.t('rules.modeAskFirst');
    if (mode === 'Automatic') return i18n.t('rules.modeAutomatic');
    return i18n.t('rules.modePreviewOnly');
  }

  function actionLabel(action: RuleAction) {
    if (action === 'Trash') return i18n.t('file.trash');
    if (action === 'Ignore') return i18n.t('rules.actionIgnoreLabel');
    return i18n.t('rules.actionMoveLabel');
  }

  function folderName(path: string) {
    return path.split(/[\\/]/).pop() || path;
  }

  function timingLabel(timing: RuleTiming) {
    return timing === 'OnArrival'
      ? i18n.t('rules.timingOnArrival')
      : i18n.t('rules.ttlDays', { days: Math.round(timing.AfterSeconds / 86400) });
  }
</script>

<Card.Root class="flex flex-col justify-between gap-2 p-3 md:flex-row md:items-center">
  <div class="flex min-w-0 flex-1 flex-col gap-1">
    <div class="flex items-center gap-2.5">
      <span class="font-semibold text-sm tracking-tight text-foreground">
        {rule.name}
      </span>
      <Badge variant="secondary">
        {i18n.t('rules.priority')}: {rule.priority}
      </Badge>
    </div>

    <p class="text-xs text-muted-foreground truncate" title={rule.watch_path}>
      {i18n.t('rules.watchTarget', { path: rule.watch_path })}
    </p>

    <div
      class="flex flex-wrap items-center gap-1.5 pt-1 text-[10px] font-medium text-muted-foreground"
    >
      <Badge variant="outline">
        {i18n.t('rules.mode')}: {modeLabel(rule.mode)}
      </Badge>
      {#if typeof rule.action === 'string'}
        <Badge variant="outline">
          {i18n.t('rules.action')}: {actionLabel(rule.action)}
        </Badge>
      {:else if 'Move' in rule.action}
        <Badge variant="outline">
          {i18n.t('rules.action')}: {actionLabel(rule.action)} ({folderName(
            rule.action.Move.destination_folder,
          )})
        </Badge>
        {#if rule.action.Move.rename_template}
          <Badge variant="outline">
            {i18n.t('rules.renameTemplate')}: {rule.action.Move.rename_template}
          </Badge>
        {/if}
      {/if}
      {#if rule.action !== 'Ignore'}
        <Badge variant="secondary">
          {timingLabel(rule.timing)}
        </Badge>
      {/if}
    </div>
  </div>

  <div class="flex items-center gap-3.5 flex-shrink-0">
    <div class="flex items-center gap-2">
      <span class="text-xs text-muted-foreground">
        {rule.enabled ? i18n.t('rules.enabled') : i18n.t('rules.disabled')}
      </span>
      <Switch
        checked={rule.enabled}
        onCheckedChange={() => onToggleEnabled(rule)}
        aria-label={i18n.t('rules.enabled')}
      />
    </div>

    <div class="flex items-center gap-1.5">
      <Button variant="outline" onclick={() => onEdit(rule)} aria-label="Edit Rule">
        {i18n.t('rules.edit')}
      </Button>
      <Button variant="destructive" onclick={() => onDelete(rule)} aria-label="Delete Rule">
        {i18n.t('rules.delete')}
      </Button>
    </div>
  </div>
</Card.Root>
