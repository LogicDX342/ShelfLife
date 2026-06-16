<script lang="ts">
  import { i18n } from '$lib/i18n/i18n.svelte';
  import type { RuleMatchExplanation } from '$lib/types';
  import { formatBytes } from '$lib/utils/format';
  import { Button } from '$lib/components/ui/button';
  import * as Card from '$lib/components/ui/card';

  let { results, ruleName, onClear } = $props<{
    results: RuleMatchExplanation[];
    ruleName: string | null;
    onClear: () => void;
  }>();
</script>

{#if results.length > 0}
  <Card.Root>
    <Card.Content class="space-y-4">
      <div
        class="flex items-center justify-between border-b border-fluent-border-light dark:border-fluent-border-dark pb-2"
      >
        <h3
          class="text-sm font-semibold tracking-tight text-fluent-text-light dark:text-fluent-text-dark"
        >
          {i18n.t('rules.testResults')}:
          <span class="text-primary">{ruleName}</span>
        </h3>
        <Button variant="link" onclick={onClear}>
          {i18n.t('rules.clearResults')}
        </Button>
      </div>

      <div class="flex flex-col gap-2 max-h-72 overflow-y-auto pr-1">
        {#each results as result (result.file_path)}
          <div
            class="p-3 bg-black/5 dark:bg-white/5 border border-fluent-border-light dark:border-fluent-border-dark rounded-md flex flex-col md:flex-row md:items-center justify-between gap-3 text-xs"
          >
            <div class="space-y-0.5 min-w-0 flex-1">
              <p class="font-semibold text-neutral-800 dark:text-neutral-200 truncate">
                {result.file_path.split('/').pop() || result.file_path}
              </p>
              <p class="text-[10px] text-fluent-muted-light dark:text-fluent-muted-dark truncate">
                {result.file_path}
              </p>
            </div>
            {#if result.size_bytes !== null}
              <span
                class="text-xs font-medium text-fluent-muted-light dark:text-fluent-muted-dark flex-shrink-0"
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
