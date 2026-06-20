<script lang="ts">
  import { Slider as SliderPrimitive } from 'bits-ui';

  import * as Item from '$lib/components/ui/item';
  import { i18n } from '$lib/i18n/i18n.svelte';
  import { cn } from '$lib/utils';

  type ThumbItem = {
    index: number;
    value: number;
  };

  let {
    value = $bindable([5, 29, 30]),
    max = 90,
    onCommit = () => {},
  }: {
    value?: number[];
    max?: number;
    onCommit?: (value: number[]) => void;
  } = $props();

  let trackBackground = $derived.by(() => {
    const staleDays = value[0];
    const decayStartDays = value[1];
    const expiryDays = value[2];

    const stalePercent = (staleDays / max) * 100;
    const decayPercent = (decayStartDays / max) * 100;
    const expiryPercent = (expiryDays / max) * 100;

    return `linear-gradient(to right, 
      #22c55e 0%, #22c55e ${stalePercent}%, 
      #eab308 ${stalePercent}%, #eab308 ${decayPercent}%, 
      #ef4444 ${decayPercent}%, #ef4444 ${expiryPercent}%, 
      #9ca3af ${expiryPercent}%, #9ca3af 100%)`;
  });

  function handleValueChange(nextValues: number[]) {
    if (nextValues.length !== 3) return;

    let staleDays = nextValues[0];
    let decayStartDays = nextValues[1];
    let expiryDays = nextValues[2];

    // Ensure minimal gap of 1 day between each zone to prevent overlapping thumbs
    if (staleDays < 1) staleDays = 1;

    if (staleDays >= decayStartDays) {
      if (nextValues[0] !== value[0]) {
        staleDays = decayStartDays - 1;
      } else {
        decayStartDays = staleDays + 1;
      }
    }

    if (decayStartDays >= expiryDays) {
      if (nextValues[1] !== value[1]) {
        decayStartDays = expiryDays - 1;
        if (decayStartDays <= staleDays) {
          staleDays = decayStartDays - 1;
        }
      } else {
        expiryDays = decayStartDays + 1;
      }
    }

    if (expiryDays > max) expiryDays = max;
    if (staleDays < 1) staleDays = 1;

    value = [staleDays, decayStartDays, expiryDays];
  }

  type ZoneConfig = {
    key: string;
    bgClass: string;
    titleClass: string;
    dotClass: string;
    title: string;
    range: string;
    desc: string;
  };

  let zones = $derived<ZoneConfig[]>([
    {
      key: 'fresh',
      bgClass: 'bg-green-500/5 dark:bg-green-500/10 border-green-500/20 dark:border-green-500/30',
      titleClass: 'text-green-600 dark:text-green-400',
      dotClass: 'bg-green-500',
      title: i18n.t('settings.freshZone'),
      range: `0 - ${value[0]} ${value[0] === 1 ? i18n.t('settings.dayUnit') : i18n.t('settings.daysUnit')}`,
      desc: i18n.t('settings.freshZoneDesc'),
    },
    {
      key: 'stale',
      bgClass:
        'bg-yellow-500/5 dark:bg-yellow-500/10 border-yellow-500/20 dark:border-yellow-500/30',
      titleClass: 'text-yellow-600 dark:text-yellow-400',
      dotClass: 'bg-yellow-500',
      title: i18n.t('settings.staleZone'),
      range: `${value[0]} - ${value[1]} ${i18n.t('settings.daysUnit')}`,
      desc: i18n.t('settings.staleZoneDesc'),
    },
    {
      key: 'decaying',
      bgClass: 'bg-red-500/5 dark:bg-red-500/10 border-red-500/20 dark:border-red-500/30',
      titleClass: 'text-red-600 dark:text-red-400',
      dotClass: 'bg-red-500',
      title: i18n.t('settings.decayingZone'),
      range: `${value[1]} - ${value[2]} ${i18n.t('settings.daysUnit')}`,
      desc: i18n.t('settings.decayingZoneDesc'),
    },
    {
      key: 'expired',
      bgClass: 'bg-zinc-500/5 dark:bg-zinc-500/10 border-zinc-500/20 dark:border-zinc-500/30',
      titleClass: 'text-zinc-500 dark:text-zinc-400',
      dotClass: 'bg-zinc-400',
      title: i18n.t('settings.expiredZone'),
      range: `> ${value[2]} ${i18n.t('settings.daysUnit')}`,
      desc: i18n.t('settings.expiredZoneDesc'),
    },
  ]);
</script>

<div class="flex flex-col gap-4 w-full select-none">
  <div class="relative py-3 px-1">
    <!-- Slider using bits-ui directly -->
    <SliderPrimitive.Root
      type="multiple"
      bind:value={value as never}
      min={1}
      {max}
      step={1}
      class="relative flex w-full touch-none items-center select-none data-disabled:opacity-50 h-0"
      onValueChange={handleValueChange}
      onValueCommit={onCommit}
    >
      {#snippet children({ thumbItems }: { thumbItems: ThumbItem[] })}
        <span
          data-slot="slider-track"
          class="relative h-2.5 w-full grow rounded-full shadow-inner border border-black/5 dark:border-white/5"
          style="background: {trackBackground};"
        >
        </span>
        {#each thumbItems as thumb (thumb.index)}
          <SliderPrimitive.Thumb
            data-slot="slider-thumb"
            index={thumb.index}
            class={cn(
              'relative size-5 rounded-full border border-zinc-300 dark:border-zinc-700 bg-white dark:bg-zinc-900 shadow-md transition-[color,box-shadow,transform] hover:ring-2 hover:ring-primary/20 focus-visible:ring-2 focus-visible:ring-primary/30 focus-visible:outline-none active:scale-110 block shrink-0 select-none disabled:pointer-events-none disabled:opacity-50 cursor-pointer',
              thumb.index === 0 &&
                'border-green-500 hover:ring-green-500/20 focus-visible:ring-green-500/30',
              thumb.index === 1 &&
                'border-yellow-500 hover:ring-yellow-500/20 focus-visible:ring-yellow-500/30',
              thumb.index === 2 &&
                'border-red-500 hover:ring-red-500/20 focus-visible:ring-red-500/30',
            )}
          />
        {/each}
      {/snippet}
    </SliderPrimitive.Root>
  </div>

  <!-- Legend Zones (using shadcn Item layout & i18n) -->
  <Item.Group class="grid grid-cols-1 sm:grid-cols-4 gap-3 text-xs pt-1">
    {#each zones as zone (zone.key)}
      <Item.Root class={cn('flex flex-col items-start gap-0.5 p-3 shadow-xs', zone.bgClass)}>
        <Item.Content class="w-full flex flex-col gap-0.5">
          <Item.Title class={cn('font-bold flex items-center gap-1.5', zone.titleClass)}>
            <span class={cn('size-2 rounded-full', zone.dotClass)}></span>
            {zone.title}
          </Item.Title>
          <div
            class="text-fluent-muted-light dark:text-fluent-muted-dark text-[11px] font-medium leading-none mt-0.5"
          >
            {zone.range}
          </div>
          <Item.Description
            class="text-[10px] text-fluent-muted-light dark:text-fluent-muted-dark leading-relaxed mt-1 line-clamp-none"
          >
            {zone.desc}
          </Item.Description>
        </Item.Content>
      </Item.Root>
    {/each}
  </Item.Group>
</div>
