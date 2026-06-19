<script lang="ts">
  import IconChevronDown from '@lucide/svelte/icons/chevron-down';

  import { Button } from '$lib/components/ui/button';
  import * as Card from '$lib/components/ui/card';
  import { Input } from '$lib/components/ui/input';
  import { Label } from '$lib/components/ui/label';
  import { Switch } from '$lib/components/ui/switch';
  import { i18n } from '$lib/i18n/i18n.svelte';
  import type { WatchTarget } from '$lib/types';
  import { ttlDaysInputFromSeconds, ttlSecondsFromDaysInput } from '$lib/utils/watch-target-config';

  let {
    target,
    globalTtlSeconds,
    rejected = false,
    onUpdate,
    onRemove,
  } = $props<{
    target: WatchTarget;
    globalTtlSeconds: number;
    rejected?: boolean;
    onUpdate: (target: WatchTarget) => Promise<boolean>;
    onRemove: (target: WatchTarget) => void;
  }>();

  let ttlDays = $state('');
  let syncedTtlKey = $state('');
  let expanded = $state(false);

  let inheritedTtlDays = $derived(Math.max(1, Math.round(globalTtlSeconds / 86400)));
  let targetTtlKey = $derived(`${target.id}:${target.default_ttl_seconds ?? 'inherit'}`);

  $effect(() => {
    if (syncedTtlKey === targetTtlKey) return;
    syncedTtlKey = targetTtlKey;
    ttlDays = ttlDaysInputFromSeconds(target.default_ttl_seconds);
  });

  async function commitTtl() {
    const nextTtlSeconds = ttlSecondsFromDaysInput(ttlDays);
    ttlDays = ttlDaysInputFromSeconds(nextTtlSeconds);
    await onUpdate({ ...target, default_ttl_seconds: nextTtlSeconds });
  }
</script>

<Card.Root class="p-3.5 flex flex-col gap-3 text-xs">
  <div class="flex flex-col md:flex-row md:items-start justify-between gap-3">
    <div class="min-w-0 flex-1 space-y-1">
      <p class="font-semibold text-neutral-800 dark:text-neutral-200 truncate" title={target.path}>
        {target.path}
      </p>
      <p
        class="text-[10px] text-fluent-muted-light dark:text-fluent-muted-dark flex items-center gap-2"
      >
        <span class="inline-flex items-center gap-1">
          <span
            class="w-1.5 h-1.5 rounded-full {target.enabled ? 'bg-green-500' : 'bg-neutral-400'}"
          ></span>
          {target.enabled ? i18n.t('settings.enabled') : i18n.t('settings.disabled')}
        </span>
        <span>•</span>
        <span>
          {target.recursive ? i18n.t('settings.recursiveLabel') : i18n.t('settings.topLevel')}
        </span>
        {#if target.default_ttl_seconds !== null}
          <span>•</span>
          <span>
            {i18n.t('settings.targetTtlOverride', { days: ttlDays || inheritedTtlDays })}
          </span>
        {/if}
      </p>
    </div>

    <div class="flex items-center gap-3.5 flex-shrink-0">
      <div class={rejected ? 'switch-rejected' : ''} title={i18n.t('settings.toggleTarget')}>
        <Switch
          checked={target.enabled}
          onCheckedChange={(enabled) => onUpdate({ ...target, enabled })}
          aria-label={i18n.t('settings.toggleTarget')}
        />
      </div>

      <Button
        variant="outline"
        onclick={() => onUpdate({ ...target, recursive: !target.recursive })}
      >
        {target.recursive ? i18n.t('settings.topLevel') : i18n.t('settings.recursiveLabel')}
      </Button>

      <Button variant="destructive" onclick={() => onRemove(target)}>
        {i18n.t('settings.remove')}
      </Button>

      <Button variant="outline" onclick={() => (expanded = !expanded)} aria-label="Toggle details">
        <IconChevronDown
          class="w-4 h-4 transform transition-transform duration-200 {expanded ? 'rotate-180' : ''}"
        />
      </Button>
    </div>
  </div>

  {#if expanded}
    <div
      class="grid grid-cols-1 md:grid-cols-[minmax(0,12rem)_1fr] gap-3 md:items-end border-t border-fluent-border-light dark:border-fluent-border-dark pt-3 animate-expand"
    >
      <div class="flex flex-col gap-1.5">
        <Label for="target-ttl-{target.id}">{i18n.t('settings.targetTtlDays')}</Label>
        <Input
          id="target-ttl-{target.id}"
          type="number"
          min="1"
          placeholder={inheritedTtlDays.toString()}
          bind:value={ttlDays}
          onchange={commitTtl}
        />
      </div>
    </div>
  {/if}
</Card.Root>

<style>
  @keyframes expand {
    from {
      opacity: 0;
      transform: translateY(-5px);
    }
    to {
      opacity: 1;
      transform: translateY(0);
    }
  }
  .animate-expand {
    animation: expand 0.2s cubic-bezier(0.25, 0.8, 0.25, 1) forwards;
  }
</style>
