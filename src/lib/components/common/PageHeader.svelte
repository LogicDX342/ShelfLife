<script lang="ts">
  import type { Snippet } from 'svelte';

  let {
    title,
    subtitle = '',
    actions,
    extra,
  } = $props<{
    title: string;
    subtitle?: string;
    actions?: Snippet;
    extra?: Snippet;
  }>();
</script>

<header
  class="flex flex-col md:flex-row md:items-center justify-between border-b border-fluent-border-light dark:border-fluent-border-dark pb-4 flex-shrink-0 gap-4"
>
  <div>
    <h1 class="text-2xl font-bold tracking-tight">{title}</h1>
    {#if subtitle}
      <p class="text-sm text-fluent-muted-light dark:text-fluent-muted-dark mt-1">
        {subtitle}
      </p>
    {/if}
    {#if extra}
      {@render extra()}
    {/if}
  </div>

  {#if actions}
    <div class="flex items-center gap-2">
      {@render actions()}
    </div>
  {/if}
</header>
