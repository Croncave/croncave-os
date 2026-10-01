<script lang="ts">
	import { get } from '$lib/api';
	import { onLive } from '$lib/live';

	// A run's output, live from any device, with the newest line kept in view.
	let { runId, initial = [] }: { runId: string; initial?: { id: number; stream: string; text: string }[] } = $props();
	let lines = $state<{ id: number; stream: string; text: string }[]>([]);
	let box: HTMLDivElement | undefined = $state();
	let follow = $state(true);

	$effect(() => {
		lines = [...initial];
		const id = runId;
		const last = () => lines.at(-1)?.id ?? 0;
		const catchUp = async () => {
			const r = await get(`/runs/${id}/output?after=${last()}`);
			lines = [...lines, ...r.output];
		};
		const off = onLive((m) => {
			if (m.kind === 'output' && m.id === id) {
				const d = m.data as { id: number; stream: string; text: string };
				if (d.id && d.id <= last()) return;
				if (d.id && d.id > last() + 1 && last() !== 0) catchUp();
				else lines = [...lines, d];
			} else if (m.kind === 'resync') catchUp();
		});
		if (!initial.length) catchUp();
		return off;
	});

	$effect(() => {
		void lines.length;
		if (follow && box) box.scrollTop = box.scrollHeight;
	});
</script>

<div class="out mono" bind:this={box} onscroll={() => box && (follow = box.scrollTop + box.clientHeight >= box.scrollHeight - 8)} data-testid="output">
	{#each lines as l (l.id)}
		<div class="l {l.stream}">{l.text}</div>
	{:else}
		<div class="low">No output yet.</div>
	{/each}
</div>

<style>
	.out {
		background: var(--pane);
		border: 1px solid var(--line);
		border-radius: 10px;
		padding: 10px 12px;
		max-height: 340px;
		min-height: 80px;
		overflow: auto;
		white-space: pre-wrap;
		word-break: break-word;
	}
	.stderr {
		color: var(--failed);
	}
	.system {
		color: var(--low);
	}
</style>
