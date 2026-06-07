<script lang="ts">
  import '../app.css';
  import type { Snippet } from 'svelte';
  import { i18n } from '$lib/i18n/i18n.svelte';
  import Sidebar from '$lib/components/Sidebar.svelte';

  i18n.init();

  let { children } = $props<{ children: Snippet }>();
</script>

<div
  class="app-shell h-screen overflow-hidden flex flex-col md:flex-row bg-fluent-bg-light dark:bg-fluent-bg-dark text-fluent-text-light dark:text-fluent-text-dark transition-colors duration-200"
>
  <!-- Sidebar -->
  <Sidebar />

  <!-- Page Content -->
  <main class="flex-1 overflow-y-auto px-6 py-6 md:px-10 md:py-8 flex justify-center">
    <div class="max-w-6xl w-full">
      {@render children()}
    </div>
  </main>
</div>
