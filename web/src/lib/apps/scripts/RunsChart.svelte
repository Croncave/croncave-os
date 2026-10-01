<script lang="ts">
	import type { RunBrief } from '$lib/types';

	// The last runs as bars: height is how long each took; failures in the failed color.
	let { runs, onpick }: { runs: RunBrief[]; onpick?: (id: string) => void } = $props();
	const done = $derived(runs.filter((r) => r.ended_at && r.started_at && r.trigger !== 'test').slice(0, 14).reverse());
	const secs = (r: RunBrief) => (new Date(r.ended_at!).getTime() - new Date(r.started_at!).getTime()) / 1000;
	const max = $derived(Math.max(1, ...done.map(secs)));
	const failed = $derived(done.filter((r) => ['failed', 'timed_out'].includes(r.status)).length);
	const day = (iso: string) => {
		const d = new Date(iso);
		return d.toDateString() === new Date().toDateString() ? 'today' : d.toLocaleDateString('en-US', { month: 'short', day: 'numeric' });
	};
</script>

<div class="chart">
	<div class="head"><span class="t">Last {done.length} run{done.length === 1 ? '' : 's'}</span><span class="note">Bar height is how long it took{failed ? ` · ${failed} failed` : ''}</span></div>
	{#if done.length}
		<div class="bars">
			{#each done as r (r.id)}
				<button
					class="bar"
					class:bad={['failed', 'timed_out'].includes(r.status)}
					style="height: {Math.max(14, (secs(r) / max) * 40)}px"
					title="{day(r.ended_at!)} · {Math.round(secs(r))} s · {r.status}"
					aria-label="Run on {day(r.ended_at!)}, {r.status}"
					onclick={() => onpick?.(r.id)}
				></button>
			{/each}
		</div>
		<div class="axis mono"><span>{day(done[0].ended_at!)}</span><span>{day(done.at(-1)!.ended_at!)}</span></div>
	{:else}
		<p class="note">No runs yet.</p>
	{/if}
</div>

<style>
	.chart {
		border: 1px solid var(--line);
		border-radius: 12px;
		padding: 16px 18px;
		display: flex;
		flex-direction: column;
		gap: 12px;
	}
	.head {
		display: flex;
		justify-content: space-between;
		align-items: baseline;
		gap: 12px;
	}
	.t {
		font-size: 16px;
		font-weight: 600;
	}
	.note {
		font-size: 13px;
		color: var(--mid);
	}
	.bars {
		display: flex;
		align-items: flex-end;
		gap: 4px;
		height: 40px;
	}
	.bar {
		flex: 1;
		max-width: 44px;
		border: 0;
		border-radius: 3px;
		background: var(--line-strong);
		cursor: pointer;
		padding: 0;
	}
	.bar:hover {
		background: var(--mid);
	}
	.bar.bad {
		background: var(--failed);
	}
	.axis {
		display: flex;
		justify-content: space-between;
		font-size: 11px;
		color: var(--low);
	}
</style>
