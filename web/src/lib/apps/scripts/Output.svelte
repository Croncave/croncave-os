<script lang="ts">
	import { untrack } from 'svelte';
	import { get } from '$lib/api';
	import { onLive } from '$lib/live';

	// A run's output with line numbers, live while it runs. Errors are tinted; `tail` shows
	// only the last lines; `mark` highlights matching lines (where a run stopped).
	type Line = { id: number; stream: string; text: string };
	let {
		runId,
		initial = [],
		tail = 0,
		height = 320,
		live = true
	}: { runId: string; initial?: Line[]; tail?: number; height?: number; live?: boolean } = $props();
	let lines = $state<Line[]>([]);
	let box: HTMLDivElement | undefined = $state();
	let follow = $state(true);

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
					lines = [...lines, ...r.output.filter((l: Line) => !seen.has(l.id))];
				} finally {
					fetching = false;
				}
			};
			if (!live) {
				// A finished run shown on its own (no lines passed in) loads its output once.
				if (!initial.length) catchUp();
				return;
			}
			const off = onLive((m) => {
				if (m.kind === 'output' && m.id === id) {
					const d = m.data as Line;
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
	const start = $derived(tail ? Math.max(0, lines.length - tail) : 0);
	const shown = $derived(lines.slice(start));
	const isError = (l: Line) => l.stream === 'stderr' && /error|failed|traceback|exception|not found|denied|expired/i.test(l.text);
</script>

<div class="out mono" bind:this={box} style="max-height: {height}px" onscroll={() => box && (follow = box.scrollTop + box.clientHeight >= box.scrollHeight - 8)} data-testid="output">
	{#each shown as l, i (l.id)}
		<div class="l {l.stream}" class:err={isError(l)}><span class="n">{start + i + 1}</span><span class="t">{l.text}</span></div>
	{:else}
		<div class="empty">No output yet.</div>
	{/each}
</div>

<style>
	.out {
		background: var(--pane);
		border: 1px solid var(--line);
		border-radius: 10px;
		padding: 10px 0;
		min-height: 60px;
		overflow: auto;
		font-size: 12.5px;
		line-height: 20px;
	}
	.l {
		display: flex;
		gap: 12px;
		padding: 0 14px;
	}
	.n {
		width: 28px;
		flex-shrink: 0;
		text-align: right;
		color: var(--low);
		user-select: none;
	}
	.t {
		white-space: pre-wrap;
		word-break: break-word;
		min-width: 0;
	}
	.system .t {
		color: var(--mid);
	}
	.stderr .t {
		color: var(--code-error);
	}
	.err {
		background: var(--failed-soft);
	}
	.err .t {
		color: var(--failed);
	}
	.empty {
		padding: 0 14px;
		color: var(--low);
	}
</style>
