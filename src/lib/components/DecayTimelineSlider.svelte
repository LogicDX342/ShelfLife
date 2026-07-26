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
      var(--success) 0%, var(--success) ${stalePercent}%,
      var(--warning) ${stalePercent}%, var(--warning) ${decayPercent}%,
      var(--destructive) ${decayPercent}%, var(--destructive) ${expiryPercent}%,
      var(--muted-foreground) ${expiryPercent}%, var(--muted-foreground) 100%)`;
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
      bgClass: 'bg-success/5 border-success/20',
      titleClass: 'text-success',
      dotClass: 'bg-success',
      title: i18n.t('settings.freshZone'),
      range: `0 - ${value[0]} ${value[0] === 1 ? i18n.t('settings.dayUnit') : i18n.t('settings.daysUnit')}`,
      desc: i18n.t('settings.freshZoneDesc'),
    },
    {
      key: 'stale',
      bgClass: 'bg-warning/5 border-warning/20',
      titleClass: 'text-warning',
      dotClass: 'bg-warning',
      title: i18n.t('settings.staleZone'),
      range: `${value[0]} - ${value[1]} ${i18n.t('settings.daysUnit')}`,
      desc: i18n.t('settings.staleZoneDesc'),
    },
    {
      key: 'decaying',
      bgClass: 'bg-destructive/5 border-destructive/20',
      titleClass: 'text-destructive',
      dotClass: 'bg-destructive',
      title: i18n.t('settings.decayingZone'),
      range: `${value[1]} - ${value[2]} ${i18n.t('settings.daysUnit')}`,
      desc: i18n.t('settings.decayingZoneDesc'),
    },
    {
      key: 'expired',
      bgClass: 'bg-muted/50 border-border',
      titleClass: 'text-muted-foreground',
      dotClass: 'bg-muted-foreground',
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
          class="relative h-2.5 w-full grow rounded-full shadow-inner border border-border"
          style="background: {trackBackground};"
        >
        </span>
        {#each thumbItems as thumb (thumb.index)}
          <SliderPrimitive.Thumb
            data-slot="slider-thumb"
            index={thumb.index}
            class={cn(
              'relative size-5 rounded-full border border-border bg-background shadow-md transition-[color,box-shadow,transform] hover:ring-2 hover:ring-ring/20 focus-visible:ring-2 focus-visible:ring-ring/30 focus-visible:outline-none active:scale-110 block shrink-0 select-none disabled:pointer-events-none disabled:opacity-50 cursor-pointer',
              thumb.index === 0 &&
                'border-success hover:ring-success/20 focus-visible:ring-success/30',
              thumb.index === 1 &&
                'border-warning hover:ring-warning/20 focus-visible:ring-warning/30',
              thumb.index === 2 &&
                'border-destructive hover:ring-destructive/20 focus-visible:ring-destructive/30',
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
          <div class="text-muted-foreground text-[11px] font-medium leading-none mt-0.5">
            {zone.range}
          </div>
          <Item.Description class="text-[10px] leading-relaxed mt-1 line-clamp-none">
            {zone.desc}
          </Item.Description>
        </Item.Content>
      </Item.Root>
    {/each}
  </Item.Group>
</div>
