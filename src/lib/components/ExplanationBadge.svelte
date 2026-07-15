<script lang="ts">
  import IconEyeOff from '@lucide/svelte/icons/eye-off';
  import IconFolderArrowRight from '@lucide/svelte/icons/folder-input';
  import IconDelete from '@lucide/svelte/icons/trash-2';

  import { i18n } from '$lib/i18n/i18n.svelte';
  import type { RuleMatchExplanation } from '$lib/types';

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
            'border-rose-200 bg-rose-50/20 dark:border-rose-950/40 dark:bg-rose-950/10 text-neutral-800 dark:text-neutral-200 hover:bg-rose-50/30 dark:hover:bg-rose-950/20',
          leftSide:
            'border-rose-200 dark:border-rose-950/40 bg-rose-100/20 dark:bg-rose-950/30 text-rose-700 dark:text-rose-400',
        };
      case 'Move':
        return {
          container:
            'border-violet-200 bg-violet-50/20 dark:border-violet-950/40 dark:bg-violet-950/10 text-neutral-800 dark:text-neutral-200 hover:bg-violet-50/30 dark:hover:bg-violet-950/20',
          leftSide:
            'border-violet-200 dark:border-violet-950/40 bg-violet-100/20 dark:bg-violet-950/30 text-violet-700 dark:text-violet-400',
        };
      case 'Ignore':
        return {
          container:
            'border-neutral-200 bg-neutral-50/30 dark:border-neutral-800 dark:bg-neutral-900/10 text-neutral-800 dark:text-neutral-200 hover:bg-neutral-50/40 dark:hover:bg-neutral-900/20',
          leftSide:
            'border-neutral-200 dark:border-neutral-800 bg-neutral-100/30 dark:bg-neutral-800/30 text-neutral-600 dark:text-neutral-400',
        };
      default:
        return {
          container:
            'border-neutral-200 bg-neutral-50/30 dark:border-fluent-border-dark dark:bg-white/5 text-fluent-text-light dark:text-fluent-text-dark',
          leftSide:
            'border-neutral-200 dark:border-fluent-border-dark bg-black/5 dark:bg-white/5 text-fluent-muted-light dark:text-fluent-muted-dark',
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

  let modeBadgeClasses = $derived.by(() => {
    switch (explanation.mode) {
      case 'Automatic':
        return 'bg-emerald-100 text-emerald-800 dark:bg-emerald-950/60 dark:text-emerald-300 border border-emerald-200/50 dark:border-emerald-900/50';
      case 'AskFirst':
        return 'bg-blue-100 text-blue-800 dark:bg-blue-950/60 dark:text-blue-300 border border-blue-200/50 dark:border-blue-900/50';
      default:
        return 'bg-neutral-100 text-neutral-700 dark:bg-neutral-800 dark:text-neutral-400 border border-neutral-200/50 dark:border-neutral-700/50';
    }
  });
</script>

<div
  class="inline-flex items-stretch self-start rounded-md border text-[10px] shadow-sm transition-all select-none {styleClasses.container}"
>
  <!-- Left Side: Action / Status Block -->
  <div class="flex items-center gap-1.5 px-2 py-0.5 border-r font-semibold {styleClasses.leftSide}">
    {#if actionType === 'Trash'}
      <!-- Clean outline trash bin icon (no internal vertical lines) -->
      <IconDelete class="w-3.5 h-3.5" />
    {:else if actionType === 'Move'}
      <!-- Move / Folder icon -->
      <IconFolderArrowRight class="w-3 h-3" />
    {:else if actionType === 'Ignore'}
      <!-- Ignore / Ban icon -->
      <IconEyeOff class="w-3 h-3" />
    {/if}
    <span>{actionLabel}</span>
  </div>

  <!-- Middle: Rule Name + Message details -->
  <div class="flex items-center gap-1.5 px-2.5 py-0.5">
    <span
      class="font-bold text-neutral-700 dark:text-neutral-300 truncate max-w-[130px]"
      title={explanation.rule_name ?? i18n.t('file.noRuleMatched')}
    >
      {explanation.rule_name ?? i18n.t('file.noRule')}
    </span>
    <span class="h-2.5 w-px bg-neutral-300 dark:bg-neutral-800"></span>
    <span
      class="text-neutral-500 dark:text-neutral-400 font-medium truncate max-w-[180px]"
      title={explanation.message}
    >
      {explanation.message}
    </span>
  </div>

  <!-- Right Side: Mode Badging -->
  {#if explanation.mode}
    <div class="flex items-center pr-1.5 py-0.5">
      <span
        class="px-1.5 py-0.2 rounded text-[8px] font-bold uppercase tracking-wider {modeBadgeClasses}"
      >
        {modeLabel}
      </span>
    </div>
  {/if}
</div>
