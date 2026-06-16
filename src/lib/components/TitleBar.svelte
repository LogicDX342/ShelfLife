<script lang="ts">
  import {
    minimizeWindow,
    toggleMaximizeWindow,
    closeWindow,
    isWindowMaximized,
    onWindowResized,
  } from '$lib/api/window';
  import IconMinimize from '@lucide/svelte/icons/minus';
  import IconMaximize from '@lucide/svelte/icons/square';
  import IconRestore from '@lucide/svelte/icons/copy';
  import IconDismiss from '@lucide/svelte/icons/x';

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
      <IconMinimize class="w-3.5 h-3.5" />
    </button>

    <!-- Maximize / Restore Button -->
    <button
      type="button"
      class="w-[46px] h-full flex items-center justify-center text-fluent-text-light dark:text-fluent-text-dark hover:bg-black/10 dark:hover:bg-white/10 active:bg-black/20 dark:active:bg-white/20 transition-colors duration-100 focus:outline-none"
      onclick={toggleMaximizeWindow}
      title={isMaximized ? 'Restore' : 'Maximize'}
    >
      {#if isMaximized}
        <IconRestore class="w-3.5 h-3.5" />
      {:else}
        <IconMaximize class="w-3.5 h-3.5" />
      {/if}
    </button>

    <!-- Close Button -->
    <button
      type="button"
      class="w-[46px] h-full flex items-center justify-center text-fluent-text-light dark:text-fluent-text-dark hover:bg-[#e81123] hover:text-white active:bg-[#f1707a] active:text-white transition-colors duration-100 focus:outline-none"
      onclick={closeWindow}
      title="Close"
    >
      <IconDismiss class="w-3.5 h-3.5" />
    </button>
  </div>
</div>
