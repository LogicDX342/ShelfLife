<script lang="ts">
  import { Spinner } from '$lib/components/ui/spinner';

  let { label } = $props<{
    label: string;
  }>();
</script>

<div class="py-12 flex flex-col items-center justify-center gap-3">
  <Spinner class="h-8 w-8 text-primary" />
  <span class="text-sm text-muted-foreground">{label}</span>
</div>
