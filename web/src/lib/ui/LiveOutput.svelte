<script lang="ts">
	import { untrack } from 'svelte';
	import { get } from '$lib/api';
	import { onLive } from '$lib/live';

	// A run's output, live from any device, with the newest line kept in view.
	let { runId, initial = [] }: { runId: string; initial?: { id: number; stream: string; text: string }[] } = $props();
	let lines = $state<{ id: number; stream: string; text: string }[]>([]);
	let box: HTMLDivElement | undefined = $state();
	let follow = $state(true);

	// Subscribe once per run. Everything inside is untracked, so updating `lines` never
	// re-runs this effect (reading `lines` here would make it loop).
	$effect(() => {
		const id = runId;
		return untrack(() => {
			lines = [...initial];
			const last = () => lines.at(-1)?.id ?? 0;
			let fetching = false;
			const catchUp = async () => {
				if (fetching) return;
				fetching = true;
				try {
					const r = await get(`/runs/${id}/output?after=${last()}`);
					const seen = new Set(lines.map((l) => l.id));
					lines = [...lines, ...r.output.filter((l: { id: number }) => !seen.has(l.id))];
				} finally {
					fetching = false;
				}
			};
			const off = onLive((m) => {
				if (m.kind === 'output' && m.id === id) {
					const d = m.data as { id: number; stream: string; text: string };
					// Ids are shared by every run's output, so only order matters here.
					if (d.id && d.id <= last()) return;
					lines = [...lines, d];
				} else if (m.kind === 'resync') catchUp();
			});
			if (!initial.length) catchUp();
			return off;
		});
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
