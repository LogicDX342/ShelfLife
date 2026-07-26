<script lang="ts">
  import IconRestore from '@lucide/svelte/icons/copy';
  import IconMinimize from '@lucide/svelte/icons/minus';
  import IconMaximize from '@lucide/svelte/icons/square';
  import IconDismiss from '@lucide/svelte/icons/x';
  import { getCurrentWindow } from '@tauri-apps/api/window';

  const appWindow = getCurrentWindow();
  let isMaximized = $state(false);

  $effect(() => {
    // Check initial state
    appWindow.isMaximized().then((val) => {
      isMaximized = val;
    });

    // Listen for resize events to update the state
    let unlisten: (() => void) | undefined;
    appWindow
      .onResized(async () => {
        isMaximized = await appWindow.isMaximized();
      })
      .then((unsub) => {
        unlisten = unsub;
      });

    return () => {
      unlisten?.();
    };
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
      class="w-[46px] h-full flex items-center justify-center text-foreground hover:bg-foreground/10 active:bg-foreground/20 transition-colors duration-100 focus:outline-none"
      onclick={() => appWindow.minimize()}
      title="Minimize"
    >
      <IconMinimize class="size-3.5" />
    </button>

    <!-- Maximize / Restore Button -->
    <button
      type="button"
      class="w-[46px] h-full flex items-center justify-center text-foreground hover:bg-foreground/10 active:bg-foreground/20 transition-colors duration-100 focus:outline-none"
      onclick={() => appWindow.toggleMaximize()}
      title={isMaximized ? 'Restore' : 'Maximize'}
    >
      {#if isMaximized}
        <IconRestore class="size-3.5" />
      {:else}
        <IconMaximize class="size-3.5" />
      {/if}
    </button>

    <!-- Close Button -->
    <button
      type="button"
      class="w-[46px] h-full flex items-center justify-center text-foreground hover:bg-[#e81123] hover:text-white active:bg-[#f1707a] active:text-white transition-colors duration-100 focus:outline-none"
      onclick={() => appWindow.close()}
      title="Close"
    >
      <IconDismiss class="size-3.5" />
    </button>
  </div>
</div>
