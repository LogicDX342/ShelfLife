<script lang="ts">
  import type { Component } from 'svelte';

  import * as Empty from '$lib/components/ui/empty';

  let {
    icon: Icon,
    title,
    description,
  } = $props<{
    icon: Component<Record<string, unknown>>;
    title: string;
    description: string;
  }>();
</script>

<Empty.Root class="border bg-muted/30">
  <Empty.Header>
    <Empty.Media>
      <Icon class="h-12 w-12 text-fluent-muted-light dark:text-fluent-muted-dark opacity-50" />
    </Empty.Media>
    <Empty.Title>{title}</Empty.Title>
    <Empty.Description>{description}</Empty.Description>
  </Empty.Header>
</Empty.Root>
