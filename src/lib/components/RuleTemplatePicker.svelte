<script lang="ts">
  import { Badge } from '$lib/components/ui/badge';
  import { Button } from '$lib/components/ui/button';
  import * as Card from '$lib/components/ui/card';
  import { i18n } from '$lib/i18n/i18n.svelte';
  import {
    preferredTemplateWatchPath,
    type RuleTemplate,
    STARTER_RULE_TEMPLATES,
  } from '$lib/rules/templates';
  import type { AppConfig } from '$lib/types';

  let { config = null, onSelect } = $props<{
    config?: AppConfig | null;
    onSelect: (template: RuleTemplate, localizedName: string) => void;
  }>();

  let watchPath = $derived(preferredTemplateWatchPath(config));

  function templateModeLabel(template: RuleTemplate) {
    return template.mode === 'Automatic'
      ? i18n.t('rules.modeAutomatic')
      : i18n.t('rules.modeAskFirst');
  }

  function templateTimingLabel(template: RuleTemplate) {
    return template.timing === 'OnArrival'
      ? i18n.t('rules.timingOnArrival')
      : i18n.t('rules.ttlDays', {
          days: Math.max(1, Math.round(template.timing.AfterSeconds / 86400)),
        });
  }

  function templateActionLabel(template: RuleTemplate) {
    return template.action === 'MoveToDefaultDestination'
      ? i18n.t('rules.actionMoveLabel')
      : i18n.t('rules.actionTrashLabel');
  }
</script>

<section class="flex flex-col gap-4" aria-labelledby="starter-rule-templates-title">
  <div class="flex flex-col gap-1">
    <h2 id="starter-rule-templates-title" class="text-lg font-semibold">
      {i18n.t('rules.templateSetTitle')}
    </h2>
    <p class="text-sm text-muted-foreground">{i18n.t('rules.templateSetDescription')}</p>
    <p class="text-xs text-muted-foreground">
      {#if watchPath}
        {i18n.t('rules.templateContext', { path: watchPath })}
      {:else}
        {i18n.t('rules.templateContextNoTarget')}
      {/if}
    </p>
  </div>

  <div class="grid grid-cols-1 gap-4 lg:grid-cols-3">
    {#each STARTER_RULE_TEMPLATES as template (template.id)}
      <Card.Root size="sm">
        <Card.Header>
          <Card.Title>{i18n.t(template.nameKey)}</Card.Title>
          <Card.Description>{i18n.t(template.descriptionKey)}</Card.Description>
          <Card.Action>
            <Badge variant="secondary">{templateModeLabel(template)}</Badge>
          </Card.Action>
        </Card.Header>
        <Card.Content>
          <div class="flex flex-wrap gap-2">
            <Badge variant="outline">{i18n.t(template.summaryKey)}</Badge>
            <Badge variant="outline">{templateTimingLabel(template)}</Badge>
            <Badge variant="outline">{templateActionLabel(template)}</Badge>
            {#if template.action === 'MoveToDefaultDestination' && !config?.default_move_destination}
              <Badge variant="secondary">{i18n.t('rules.templateDestinationRequired')}</Badge>
            {/if}
          </div>
        </Card.Content>
        <Card.Footer>
          <Button
            class="w-full"
            variant="outline"
            onclick={() => onSelect(template, i18n.t(template.nameKey))}
          >
            {i18n.t('rules.useTemplate')}
          </Button>
        </Card.Footer>
      </Card.Root>
    {/each}
  </div>
</section>
