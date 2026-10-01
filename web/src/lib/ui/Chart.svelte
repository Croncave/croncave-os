<script lang="ts">
	// A small line or bar chart in token colors.
	type Point = { x: number; y: number };
	let {
		points,
		kind = 'line',
		height = 120,
		threshold,
		format = (v: number) => v.toFixed(2)
	}: { points: Point[]; kind?: 'line' | 'bar'; height?: number; threshold?: number; format?: (v: number) => string } = $props();
	const W = 600;
	const pad = 4;
	const ys = $derived([...points.map((p) => p.y), ...(threshold !== undefined ? [threshold] : [])]);
	const min = $derived(kind === 'bar' ? 0 : Math.min(...ys));
	const max = $derived(Math.max(...ys, min + 1e-9));
	const sx = (i: number) => pad + (i * (W - 2 * pad)) / Math.max(1, points.length - 1);
	const sy = (v: number) => height - pad - ((v - min) * (height - 2 * pad)) / (max - min || 1);
	const path = $derived(points.map((p, i) => `${i ? 'L' : 'M'}${sx(i).toFixed(1)},${sy(p.y).toFixed(1)}`).join(''));
	const bw = $derived(Math.max(2, (W - 2 * pad) / Math.max(1, points.length) - 3));
</script>

{#if points.length === 0}
	<div class="muted-box">No data yet.</div>
{:else}
	<svg viewBox="0 0 {W} {height}" preserveAspectRatio="none" style="height: {height}px" role="img" aria-label="Chart">
		{#if threshold !== undefined}
			<line x1="0" x2={W} y1={sy(threshold)} y2={sy(threshold)} class="thr" />
		{/if}
		{#if kind === 'line'}
			<path d={path} class="line" />
		{:else}
			{#each points as p, i (i)}
				<rect x={pad + i * ((W - 2 * pad) / points.length)} y={sy(p.y)} width={bw} height={height - pad - sy(p.y)} class="bar"><title>{format(p.y)}</title></rect>
			{/each}
		{/if}
	</svg>
	<div class="row low mono scale"><span>{format(min)}</span><span class="spacer"></span><span>{format(max)}</span></div>
{/if}

<style>
	svg {
		width: 100%;
		display: block;
	}
	.line {
		fill: none;
		stroke: var(--chart-1);
		stroke-width: 2;
		vector-effect: non-scaling-stroke;
	}
	.bar {
		fill: var(--chart-2);
	}
	.thr {
		stroke: var(--needs);
		stroke-dasharray: 4 4;
		vector-effect: non-scaling-stroke;
	}
	.scale {
		font-size: 11px;
	}
</style>
