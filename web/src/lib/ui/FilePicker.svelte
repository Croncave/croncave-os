<script lang="ts">
	import Modal from './Modal.svelte';
	import { get, message } from '$lib/api';
	import { bytes } from '$lib/format';

	// Pick a file from the computer (wakes it, like opening Files).
	let { computer, onpick, onclose, filter }: { computer: string; onpick: (path: string) => void; onclose: () => void; filter?: (name: string) => boolean } = $props();
	let path = $state('');
	// eslint-disable-next-line @typescript-eslint/no-explicit-any
	let entries = $state<any[]>([]);
	let error = $state('');
	let loading = $state(true);

	$effect(() => {
		const p = path;
		loading = true;
		get(`/computers/${computer}/files?path=${encodeURIComponent(p)}`)
			.then((r) => ((entries = r.entries), (error = '')))
			.catch((e) => (error = message(e)))
			.finally(() => (loading = false));
	});
	const up = () => (path = path.split('/').slice(0, -1).join('/'));
</script>

<Modal title="Choose a file" {onclose}>
	<div class="row mono low">/{path}</div>
	{#if loading}<div class="low">Opening your files… (your computer wakes if it's asleep)</div>{/if}
	{#if error}<div class="error">{error}</div>{/if}
	<div class="list pick">
		{#if path}<button class="item" onclick={up}>← Up</button>{/if}
		{#each entries as e (e.path)}
			{#if e.is_dir}
				<button class="item" onclick={() => (path = e.path)}>📁 {e.name}</button>
			{:else}
				<button class="item" disabled={filter && !filter(e.name)} onclick={() => onpick(e.path)}>📄 {e.name} <span class="low">{bytes(e.size)}</span></button>
			{/if}
		{/each}
	</div>
</Modal>

<style>
	.pick {
		max-height: 360px;
		overflow: auto;
	}
	.item {
		text-align: left;
		background: none;
		border: 0;
		border-bottom: 1px solid var(--line);
		color: var(--ink);
		font: inherit;
		padding: 8px 4px;
		cursor: pointer;
		display: flex;
		gap: 8px;
	}
	.item:disabled {
		color: var(--low);
		cursor: default;
	}
</style>
