<script lang="ts">
  import type { Snippet } from 'svelte';

  let { children } = $props<{
    children: Snippet;
  }>();
</script>

<div class="flex-1 min-h-0 overflow-y-auto mt-6 [scrollbar-gutter:stable_both-edges]">
  <div class="max-w-6xl mx-auto w-full px-6 md:px-10 pb-8 space-y-6">
    {@render children()}
  </div>
</div>
