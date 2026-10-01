<script lang="ts">
	import { onMount } from 'svelte';
	import Modal from '$lib/ui/Modal.svelte';
	import { get } from '$lib/api';
	import { cap } from '$lib/plans';

	// What draws from free usage, with the catalog's own rates.
	let { onclose }: { onclose: () => void } = $props();
	// eslint-disable-next-line @typescript-eslint/no-explicit-any
	let data = $state<any>(null);
	onMount(() => {
		get('/plans').then((r) => (data = r));
	});
	const rate = (n: number) => `$${n < 0.1 ? n.toFixed(3) : n.toFixed(2)}`;
</script>

<Modal title="What counts as usage" lede="Usage is what your computers cost to run. It comes out of your free usage first; when that's gone, work pauses unless you turn on overage." {onclose}>
	{#if data}
		<div class="table">
			{#each Object.entries(data.sizes) as [id, s] (id)}
				{@const sz = s as { cpu: number; memory_gb: number; provider_hourly: number }}
				<div class="r"><span>{cap(id)} computer, while awake <span class="mono low">{sz.cpu} CPU · {sz.memory_gb} GB</span></span><span class="mono">{rate(sz.provider_hourly * (1 + data.markup))} / hr</span></div>
			{/each}
			<div class="r"><span>Storage, kept even while asleep</span><span class="mono">{rate(data.disk_gb_month * (1 + data.markup))} / GB a month</span></div>
			<div class="r"><span>Built-in AI, only if you turn it on</span><span class="mono">at cost, no markup</span></div>
		</div>
		<p class="mid">A sleeping computer costs only its storage. Computers sleep about 30 seconds after nothing is running and wake by themselves for anything scheduled.</p>
	{:else}
		<p class="mid">Loading…</p>
	{/if}
</Modal>

<style>
	.table {
		border: 1px solid var(--line);
		border-radius: 10px;
		background: var(--pane);
	}
	.r {
		display: flex;
		justify-content: space-between;
		gap: 16px;
		padding: 10px 14px;
		font-size: 13px;
	}
	.r + .r {
		border-top: 1px solid var(--line);
	}
	.r .low {
		font-size: 12px;
		margin-left: 6px;
	}
	p {
		font-size: 13px;
		line-height: 19px;
	}
</style>
