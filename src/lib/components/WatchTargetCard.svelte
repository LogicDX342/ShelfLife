<script lang="ts">
  import { Button } from '$lib/components/ui/button';
  import * as Card from '$lib/components/ui/card';
  import { Switch } from '$lib/components/ui/switch';
  import { i18n } from '$lib/i18n/i18n.svelte';
  import type { WatchTarget } from '$lib/types';
  import { cn } from '$lib/utils';

  let { target, onUpdate, onRemove } = $props<{
    target: WatchTarget;
    onUpdate: (target: WatchTarget) => Promise<boolean>;
    onRemove: (target: WatchTarget) => void;
  }>();
</script>

<Card.Root size="sm">
  <Card.Content>
    <div class="flex flex-col md:flex-row md:items-start justify-between gap-3">
      <div class="min-w-0 flex flex-1 flex-col gap-1">
        <p class="font-semibold truncate" title={target.path}>
          {target.path}
        </p>
        <p class="text-xs text-muted-foreground flex items-center gap-2">
          <span class="inline-flex items-center gap-1">
            <span
              class={cn(
                'size-1.5 rounded-full',
                target.enabled ? 'bg-success' : 'bg-muted-foreground',
              )}
            ></span>
            {target.enabled ? i18n.t('settings.enabled') : i18n.t('settings.disabled')}
          </span>
          <span>•</span>
          <span>
            {target.recursive ? i18n.t('settings.recursiveLabel') : i18n.t('settings.topLevel')}
          </span>
        </p>
      </div>

      <div class="flex items-center gap-3.5 flex-shrink-0">
        <div title={i18n.t('settings.toggleTarget')}>
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
      </div>
    </div>
  </Card.Content>
</Card.Root>
