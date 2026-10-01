<script lang="ts">
	import Chart from '$lib/ui/Chart.svelte';
	// What a check found: new items, a page change, or a price over time.
	// eslint-disable-next-line @typescript-eslint/no-explicit-any
	let { data }: { data: any } = $props();
	const items = $derived(data.new_items ?? data.matching ?? []);
</script>

<div class="stack">
	{#if data.new_items}
		<h3>{data.new_items.length ? 'New matches' : 'Matching now'}</h3>
		<div class="items">
			{#each items as it (it.id)}
				<div class="pane stack" style="gap: 2px" data-testid="match">
					<strong>{#if it.fields.link}<a href={String(it.fields.link).startsWith('/') ? it.fields.link : it.fields.link} target="_blank" rel="noreferrer">{it.fields.title}</a>{:else}{it.fields.title}{/if}</strong>
					<span class="mono">{it.fields.price ? `$${it.fields.price}` : ''} {it.fields.beds !== undefined ? `· ${it.fields.beds} bd` : ''}</span>
					<span class="low">{it.fields.area ?? ''}</span>
				</div>
			{:else}
				<div class="low">Nothing new this time.</div>
			{/each}
		</div>
	{/if}
	{#if data.diff && (data.diff.added?.length || data.diff.removed?.length)}
		<h3>What changed</h3>
		<div class="mono diff">
			{#each data.diff.removed as l, i (i)}<div class="del">− {l}</div>{/each}
			{#each data.diff.added as l, i (i)}<div class="add">+ {l}</div>{/each}
		</div>
	{/if}
	{#if data.history}
		<h3>Price</h3>
		<Chart points={data.history.map((h: [number, number]) => ({ x: h[0], y: h[1] }))} threshold={data.limit} />
	{/if}
	{#if data.excerpt && !data.diff?.added?.length}
		<h3>What it read</h3>
		<pre class="excerpt">{data.excerpt}</pre>
	{/if}
</div>

<style>
	.items {
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(220px, 1fr));
		gap: 8px;
	}
	.diff {
		background: var(--pane);
		border-radius: 8px;
		padding: 10px;
	}
	.add {
		color: var(--live);
	}
	.del {
		color: var(--failed);
	}
	.excerpt {
		white-space: pre-wrap;
		margin: 0;
		color: var(--mid);
		max-height: 200px;
		overflow: auto;
	}
</style>
