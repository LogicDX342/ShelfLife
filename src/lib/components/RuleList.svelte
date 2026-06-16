<script lang="ts">
  import IconDocumentBulletList from '@lucide/svelte/icons/list-checks';

  import * as Empty from '$lib/components/ui/empty';
  import { Spinner } from '$lib/components/ui/spinner';
  import { i18n } from '$lib/i18n/i18n.svelte';
  import type { AutomationRule } from '$lib/types';

  import RuleCard from './RuleCard.svelte';

  let {
    rules,
    loading = false,
    testingRuleId = null,
    onEdit,
    onTest,
    onDelete,
    onToggleEnabled,
  } = $props<{
    rules: AutomationRule[];
    loading?: boolean;
    testingRuleId?: string | null;
    onEdit: (rule: AutomationRule) => void;
    onTest: (rule: AutomationRule) => void;
    onDelete: (rule: AutomationRule) => void;
    onToggleEnabled: (rule: AutomationRule) => void;
  }>();
</script>

{#if loading && rules.length === 0}
  <div class="py-12 flex flex-col items-center justify-center gap-3">
    <Spinner class="h-8 w-8 text-primary" />
    <span class="text-sm text-muted-foreground">
      {i18n.t('rules.loading')}
    </span>
  </div>
{:else if rules.length === 0}
  <Empty.Root class="border bg-muted/30">
    <Empty.Header>
      <Empty.Media>
        <IconDocumentBulletList
          class="h-12 w-12 text-fluent-muted-light dark:text-fluent-muted-dark opacity-50"
        />
      </Empty.Media>
      <Empty.Title>{i18n.t('rules.noRules')}</Empty.Title>
      <Empty.Description>
        {i18n.t('rules.noRulesDesc')}
      </Empty.Description>
    </Empty.Header>
  </Empty.Root>
{:else}
  <section class="space-y-3">
    {#each rules as rule (rule.id)}
      <RuleCard
        {rule}
        testing={testingRuleId === rule.id}
        {onEdit}
        {onTest}
        {onDelete}
        {onToggleEnabled}
      />
    {/each}
  </section>
{/if}
