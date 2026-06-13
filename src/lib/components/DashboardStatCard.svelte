<script lang="ts">
  let { label, value, description, valueClass } = $props<{
    label: string;
    value: number;
    description: string;
    valueClass: string;
  }>();
</script>

<div
  class="fluent-card p-6 flex flex-col justify-between h-32 bg-fluent-card-light dark:bg-fluent-card-dark"
>
  <span
    class="text-xs font-semibold text-fluent-muted-light dark:text-fluent-muted-dark uppercase tracking-wider"
  >
    {label}
  </span>
  <span class="text-3xl font-extrabold mt-2 {valueClass}">
    {value}
  </span>
  <p class="text-[10px] text-fluent-muted-light dark:text-fluent-muted-dark mt-1">
    {description}
  </p>
</div>
