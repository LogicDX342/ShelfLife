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

<div class="w-full flex-shrink-0 px-6 pt-8 md:px-10 md:pt-10">
  <div class="max-w-6xl mx-auto w-full">
    <header
      class="flex flex-col md:flex-row md:items-center justify-between border-b border-fluent-border-light dark:border-fluent-border-dark pb-2 gap-4"
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
  </div>
</div>
