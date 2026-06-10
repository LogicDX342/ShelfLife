<script lang="ts">
  import type { RuleMatchExplanation } from '$lib/types';
  import { i18n } from '$lib/i18n/i18n.svelte';

  let { explanation } = $props<{ explanation: RuleMatchExplanation }>();
</script>

<div
  class="inline-flex items-center gap-1.5 p-1 px-2.5 rounded bg-black/5 dark:bg-white/5 border border-fluent-border-light dark:border-fluent-border-dark text-[10px] font-semibold text-fluent-text-light dark:text-fluent-text-dark select-none max-w-sm"
>
  <!-- Status Indicator Dot -->
  <span
    class="h-1.5 w-1.5 rounded-full {explanation.blocked_by_protected_pattern
      ? 'bg-red-500'
      : 'bg-fluent-accent'}"
  ></span>

  <!-- Rule Name -->
  <span
    class="truncate max-w-[120px] font-bold text-neutral-700 dark:text-neutral-300"
    title={explanation.rule_name ?? i18n.t('file.noRuleMatched')}
  >
    {explanation.rule_name ?? i18n.t('file.noRule')}
  </span>

  <span
    class="text-fluent-muted-light dark:text-fluent-muted-dark font-normal truncate"
    title={explanation.message}
  >
    ({explanation.message})
  </span>

  {#if explanation.blocked_by_protected_pattern}
    <span
      class="ml-1 px-1 py-0.2 bg-red-100 dark:bg-red-950/40 text-red-700 dark:text-red-300 rounded text-[9px] uppercase font-bold tracking-wider"
    >
      {i18n.t('file.protected')}
    </span>
  {:else if explanation.mode}
    <span
      class="ml-1 px-1 py-0.2 bg-neutral-200 dark:bg-neutral-800 text-neutral-600 dark:text-neutral-400 rounded text-[9px] font-mono"
    >
      {explanation.mode}
    </span>
  {/if}
</div>
