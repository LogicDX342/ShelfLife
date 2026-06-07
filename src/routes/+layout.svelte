<script lang="ts">
  import '../app.css';
  import type { Snippet } from 'svelte';
  import { i18n } from '$lib/i18n/i18n.svelte';
  import Sidebar from '$lib/components/Sidebar.svelte';
  import TitleBar from '$lib/components/TitleBar.svelte';

  i18n.init();

  let { children } = $props<{ children: Snippet }>();
</script>

<div
  class="app-container h-screen overflow-hidden flex flex-col bg-fluent-bg-light dark:bg-fluent-bg-dark text-fluent-text-light dark:text-fluent-text-dark transition-colors duration-200 relative"
>
  <!-- Custom Title Bar -->
  <TitleBar />

  <div class="app-shell flex-1 overflow-hidden flex flex-row">
    <!-- Sidebar -->
    <Sidebar />

    <!-- Page Content -->
    <main
      class="flex-1 overflow-hidden px-6 pb-6 pt-12 md:px-10 md:pb-8 md:pt-14 flex justify-center"
    >
      <div class="max-w-6xl w-full h-full flex flex-col min-h-0">
        {@render children()}
      </div>
    </main>
  </div>
</div>
