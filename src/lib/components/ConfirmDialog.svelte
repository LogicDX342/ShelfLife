<script lang="ts">
  import type { Snippet } from 'svelte';
  import { i18n } from '$lib/i18n/i18n.svelte';

  let {
    open = false,
    title = '',
    message = '',
    confirmLabel = 'Confirm',
    cancelLabel = i18n.t('dialog.no'),
    disabled = false,
    children,
    onConfirm,
    onCancel,
  } = $props<{
    open: boolean;
    title: string;
    message?: string;
    confirmLabel?: string;
    cancelLabel?: string;
    disabled?: boolean;
    children?: Snippet;
    onConfirm: () => void;
    onCancel: () => void;
  }>();
</script>

{#if open}
  <div
    class="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/40 backdrop-blur-xs transition-opacity duration-200"
  >
    <!-- Modal Dialog card -->
    <div
      class="bg-fluent-card-light dark:bg-fluent-card-dark border border-fluent-border-light dark:border-fluent-border-dark rounded-lg shadow-xl max-w-md w-full overflow-hidden transform transition-all duration-200 scale-100 flex flex-col"
      role="alertdialog"
      aria-modal="true"
      aria-labelledby="dialog-title"
      aria-describedby="dialog-desc"
    >
      <!-- Dialog Header -->
      <div class="px-6 py-5 border-b border-fluent-border-light dark:border-fluent-border-dark">
        <h2
          id="dialog-title"
          class="text-base font-semibold tracking-tight text-fluent-text-light dark:text-fluent-text-dark"
        >
          {title}
        </h2>
      </div>

      <!-- Dialog Body -->
      {#if message || children}
        <div class="px-6 py-5 flex-1 space-y-4">
          {#if message}
            <p
              id="dialog-desc"
              class="text-sm leading-normal text-fluent-muted-light dark:text-fluent-muted-dark whitespace-pre-wrap"
            >
              {message}
            </p>
          {/if}
          {#if children}
            {@render children()}
          {/if}
        </div>
      {/if}

      <!-- Dialog Footer Action Buttons -->
      <div
        class="px-6 py-4 bg-black/5 dark:bg-white/5 border-t border-fluent-border-light dark:border-fluent-border-dark flex items-center justify-end gap-2"
      >
        <button onclick={onCancel} class="fluent-button text-xs font-semibold" {disabled}>
          {cancelLabel}
        </button>
        <button
          onclick={onConfirm}
          class="fluent-button fluent-button-primary text-xs font-semibold"
          {disabled}
        >
          {confirmLabel}
        </button>
      </div>
    </div>
  </div>
{/if}
