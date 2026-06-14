<script lang="ts">
  import { notifications } from '$lib/stores/notifications.svelte';
  import { fade, fly } from 'svelte/transition';
  import IconCheckmarkCircle from '~icons/fluent/checkmark-circle-24-regular';
  import IconErrorCircle from '~icons/fluent/error-circle-24-regular';
  import IconWarning from '~icons/fluent/warning-24-regular';
  import IconInfo from '~icons/fluent/info-24-regular';
  import IconDismiss from '~icons/fluent/dismiss-16-regular';
</script>

<div
  class="fixed bottom-6 right-6 z-50 flex flex-col gap-3 w-80 max-w-[calc(100vw-3rem)] pointer-events-none"
  aria-live="polite"
>
  {#each notifications.toasts as toast (toast.id)}
    <div
      in:fly={{ x: 320, duration: 300 }}
      out:fade={{ duration: 150 }}
      onclick={() => notifications.cancelTimer(toast.id)}
      onkeydown={(e) => {
        if (e.key === 'Enter' || e.key === ' ') {
          e.preventDefault();
          notifications.cancelTimer(toast.id);
        }
      }}
      class="acrylic-card rounded-lg shadow-lg pointer-events-auto p-4 flex gap-3 items-start border-l-4 overflow-hidden relative select-none transition-all duration-200 hover:scale-[1.02] active:scale-[0.98] cursor-pointer
        {toast.type === 'success' ? 'border-l-green-500' : ''}
        {toast.type === 'error' ? 'border-l-red-500' : ''}
        {toast.type === 'warning' ? 'border-l-amber-500' : ''}
        {toast.type === 'info' ? 'border-l-fluent-accent' : ''}"
      role="button"
      tabindex="0"
    >
      <!-- Icon indicator -->
      <div class="flex-shrink-0 mt-0.5">
        {#if toast.type === 'success'}
          <IconCheckmarkCircle class="w-5 h-5 text-green-500" />
        {:else if toast.type === 'error'}
          <IconErrorCircle class="w-5 h-5 text-red-500" />
        {:else if toast.type === 'warning'}
          <IconWarning class="w-5 h-5 text-amber-500" />
        {:else}
          <IconInfo class="w-5 h-5 text-fluent-accent" />
        {/if}
      </div>

      <!-- Message text content -->
      <div class="flex-1 min-w-0 pr-2">
        <p
          class="text-xs font-semibold text-fluent-text-light dark:text-fluent-text-dark whitespace-pre-wrap break-words leading-relaxed"
        >
          {toast.message}
        </p>
      </div>

      <!-- Close button -->
      <button
        onclick={(e) => {
          e.stopPropagation();
          notifications.dismiss(toast.id);
        }}
        class="text-fluent-muted-light dark:text-fluent-muted-dark hover:text-fluent-text-light dark:hover:text-fluent-text-dark flex-shrink-0 transition-colors p-1 rounded-full hover:bg-black/5 dark:hover:bg-white/5"
        aria-label="Dismiss notification"
      >
        <IconDismiss class="w-3.5 h-3.5" />
      </button>

      {#if toast.duration > 0}
        <div
          class="absolute bottom-0 left-0 h-[3px] opacity-75 toast-progress
            {toast.type === 'success' ? 'bg-green-500' : ''}
            {toast.type === 'error' ? 'bg-red-500' : ''}
            {toast.type === 'warning' ? 'bg-amber-500' : ''}
            {toast.type === 'info' ? 'bg-fluent-accent' : ''}"
          style="--duration: {toast.duration}ms"
        ></div>
      {/if}
    </div>
  {/each}
</div>

<style>
  @keyframes shrink-progress {
    from {
      width: 100%;
    }
    to {
      width: 0%;
    }
  }
  .toast-progress {
    animation: shrink-progress var(--duration) linear forwards;
  }
</style>
