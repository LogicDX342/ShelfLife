<script lang="ts">
  import IconEyeOff from '@lucide/svelte/icons/eye-off';
  import IconFolderArrowRight from '@lucide/svelte/icons/folder-input';
  import IconDelete from '@lucide/svelte/icons/trash-2';

  import { Badge, type BadgeVariant } from '$lib/components/ui/badge';
  import { i18n } from '$lib/i18n/i18n.svelte';
  import type { RuleMatchExplanation } from '$lib/types';
  import { cn } from '$lib/utils';

  let { explanation } = $props<{ explanation: RuleMatchExplanation }>();

  // Determine action type and details
  let actionType = $derived.by(() => {
    const action = explanation.proposed_action;
    if (!action) return 'None';
    if (action === 'Trash') return 'Trash';
    if (action === 'Ignore') return 'Ignore';
    if (typeof action === 'object' && 'Move' in action) return 'Move';
    return 'Other';
  });

  let actionLabel = $derived.by(() => {
    switch (actionType) {
      case 'Trash':
        return i18n.t('file.trash');
      case 'Ignore':
        return i18n.t('file.ignore');
      case 'Move':
        return i18n.t('file.actionMove');
      default:
        return i18n.t('file.actionLabel');
    }
  });

  let styleClasses = $derived.by(() => {
    switch (actionType) {
      case 'Trash':
        return {
          container:
            'border-destructive/20 bg-destructive/5 text-foreground hover:bg-destructive/10',
          leftSide: 'border-destructive/20 bg-destructive/10 text-destructive',
        };
      case 'Move':
        return {
          container: 'border-info/20 bg-info/5 text-foreground hover:bg-info/10',
          leftSide: 'border-info/20 bg-info/10 text-info',
        };
      case 'Ignore':
        return {
          container: 'border-border bg-muted/30 text-foreground hover:bg-muted/50',
          leftSide: 'border-border bg-muted text-muted-foreground',
        };
      default:
        return {
          container: 'border-border bg-muted/30 text-foreground',
          leftSide: 'border-border bg-muted text-muted-foreground',
        };
    }
  });

  let modeLabel = $derived.by(() => {
    switch (explanation.mode) {
      case 'Automatic':
        return i18n.t('rules.modeAutomatic') ?? 'Run automatically';
      case 'AskFirst':
        return i18n.t('rules.modeAskFirst') ?? 'Ask before acting';
      case 'PreviewOnly':
        return i18n.t('rules.modePreviewOnly') ?? 'Preview changes';
      default:
        return '';
    }
  });

  let modeBadgeVariant = $derived<BadgeVariant>(
    explanation.mode === 'PreviewOnly' ? 'secondary' : 'outline',
  );

  let modeBadgeClasses = $derived(
    cn(
      explanation.mode === 'Automatic' && 'border-success/20 bg-success/10 text-success',
      explanation.mode === 'AskFirst' && 'border-info/20 bg-info/10 text-info',
    ),
  );
</script>

<div
  class={cn(
    'inline-flex items-stretch self-start rounded-md border text-[10px] shadow-sm transition-all select-none',
    styleClasses.container,
  )}
>
  <!-- Left Side: Action / Status Block -->
  <div
    class={cn(
      'flex items-center gap-1.5 px-2 py-0.5 border-r font-semibold',
      styleClasses.leftSide,
    )}
  >
    {#if actionType === 'Trash'}
      <!-- Clean outline trash bin icon (no internal vertical lines) -->
      <IconDelete class="size-3.5" />
    {:else if actionType === 'Move'}
      <!-- Move / Folder icon -->
      <IconFolderArrowRight class="size-3" />
    {:else if actionType === 'Ignore'}
      <!-- Ignore / Ban icon -->
      <IconEyeOff class="size-3" />
    {/if}
    <span>{actionLabel}</span>
  </div>

  <!-- Middle: Rule Name + Message details -->
  <div class="flex items-center gap-1.5 px-2.5 py-0.5">
    <span
      class="font-bold text-foreground truncate max-w-[130px]"
      title={explanation.rule_name ?? i18n.t('file.noRuleMatched')}
    >
      {explanation.rule_name ?? i18n.t('file.noRule')}
    </span>
    <span class="h-2.5 w-px bg-border"></span>
    <span
      class="text-muted-foreground font-medium truncate max-w-[180px]"
      title={explanation.message}
    >
      {explanation.message}
    </span>
  </div>

  <!-- Right Side: Mode Badging -->
  {#if explanation.mode}
    <div class="flex items-center pr-1.5 py-0.5">
      <Badge variant={modeBadgeVariant} class={modeBadgeClasses}>{modeLabel}</Badge>
    </div>
  {/if}
</div>
