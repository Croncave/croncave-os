<script lang="ts">
	// A small line of recent values, in one of the status or chart colors.
	let { values, max, tone = 'working', height = 28 }: { values: number[]; max?: number; tone?: string; height?: number } = $props();
	const top = $derived(max ?? Math.max(1, ...values));
	const points = $derived(
		values.length > 1 ? values.map((v, i) => `${(i / (values.length - 1)) * 100},${height - 2 - (Math.min(v, top) / top) * (height - 4)}`).join(' ') : ''
	);
</script>

<svg viewBox="0 0 100 {height}" preserveAspectRatio="none" width="100%" {height} aria-hidden="true">
	{#if points}<polyline {points} fill="none" style="stroke: var(--{tone})" stroke-width="1.5" vector-effect="non-scaling-stroke" />{/if}
</svg>
