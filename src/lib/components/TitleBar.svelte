<script lang="ts">
  import {
    minimizeWindow,
    toggleMaximizeWindow,
    closeWindow,
    isWindowMaximized,
    onWindowResized,
  } from '$lib/api/window';

  let isMaximized = $state(false);

  $effect(() => {
    // Check initial state
    isWindowMaximized().then((val) => {
      isMaximized = val;
    });

    // Listen for resize events to update the state
    const cleanup = onWindowResized(async () => {
      isMaximized = await isWindowMaximized();
    });

    return cleanup;
  });
</script>

<div
  class="absolute top-0 left-0 w-full h-8 flex items-center justify-between select-none bg-transparent border-0 z-50"
  data-tauri-drag-region
>
  <!-- Draggable spacer / left area -->
  <div class="flex-1 h-full" data-tauri-drag-region></div>

  <!-- Window Controls -->
  <div class="flex h-full items-center">
    <!-- Minimize Button -->
    <button
      type="button"
      class="w-[46px] h-full flex items-center justify-center text-fluent-text-light dark:text-fluent-text-dark hover:bg-black/10 dark:hover:bg-white/10 active:bg-black/20 dark:active:bg-white/20 transition-colors duration-100 focus:outline-none"
      onclick={minimizeWindow}
      title="Minimize"
    >
      <svg
        class="w-3.5 h-3.5"
        fill="none"
        stroke="currentColor"
        stroke-width="1.2"
        viewBox="0 0 10 10"
      >
        <path d="M1 5.5h8" />
      </svg>
    </button>

    <!-- Maximize / Restore Button -->
    <button
      type="button"
      class="w-[46px] h-full flex items-center justify-center text-fluent-text-light dark:text-fluent-text-dark hover:bg-black/10 dark:hover:bg-white/10 active:bg-black/20 dark:active:bg-white/20 transition-colors duration-100 focus:outline-none"
      onclick={toggleMaximizeWindow}
      title={isMaximized ? 'Restore' : 'Maximize'}
    >
      {#if isMaximized}
        <svg
          class="w-3.5 h-3.5"
          fill="none"
          stroke="currentColor"
          stroke-width="1.2"
          viewBox="0 0 10 10"
        >
          <path d="M2 3.5h5v5H2z" />
          <path d="M3.5 3.5v-2h5v5h-1.5" />
        </svg>
      {:else}
        <svg
          class="w-3.5 h-3.5"
          fill="none"
          stroke="currentColor"
          stroke-width="1.2"
          viewBox="0 0 10 10"
        >
          <rect x="2" y="2" width="6" height="6" rx="0.5" />
        </svg>
      {/if}
    </button>

    <!-- Close Button -->
    <button
      type="button"
      class="w-[46px] h-full flex items-center justify-center text-fluent-text-light dark:text-fluent-text-dark hover:bg-[#e81123] hover:text-white active:bg-[#f1707a] active:text-white transition-colors duration-100 focus:outline-none"
      onclick={closeWindow}
      title="Close"
    >
      <svg
        class="w-3.5 h-3.5"
        fill="none"
        stroke="currentColor"
        stroke-width="1.2"
        viewBox="0 0 10 10"
      >
        <path d="M2 2l6 6M8 2L2 8" />
      </svg>
    </button>
  </div>
</div>
