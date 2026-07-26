<script lang="ts">
  import * as Card from '$lib/components/ui/card';
  import { cn } from '$lib/utils';

  let { label, value, description, tone } = $props<{
    label: string;
    value: number;
    description: string;
    tone: 'warning' | 'success' | 'muted';
  }>();

  let valueClass = $derived(
    tone === 'warning'
      ? 'text-warning'
      : tone === 'success'
        ? 'text-success'
        : 'text-muted-foreground',
  );
</script>

<Card.Root class="h-32 justify-between">
  <Card.Header>
    <Card.Title class="text-xs font-semibold uppercase tracking-wider text-muted-foreground">
      {label}
    </Card.Title>
  </Card.Header>
  <Card.Content>
    <span class={cn('block text-4xl font-extrabold', valueClass)}>
      {value}
    </span>
    <p class="mt-1 text-[10px] text-muted-foreground">
      {description}
    </p>
  </Card.Content>
</Card.Root>
