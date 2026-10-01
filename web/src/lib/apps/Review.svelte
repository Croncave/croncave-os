<script lang="ts">
	import { get, post, message } from '$lib/api';
	import { diffLines, hunks } from '$lib/diff';
	import Button from '$lib/ui/Button.svelte';
	import { toast } from '$lib/ui/toast.svelte';

	// Review an agent's changes file by file; keep or undo each. Nothing lands until you keep it.
	let { runId, onchanged }: { runId: string; onchanged: () => void } = $props();
	// eslint-disable-next-line @typescript-eslint/no-explicit-any
	let files = $state<any[]>([]);
	let error = $state('');

	async function load() {
		try {
			files = (await get(`/runs/${runId}/review`)).files;
		} catch (e) {
			error = message(e);
		}
	}
	$effect(() => {
		void runId;
		load();
	});

	async function decide(path: string, decision: 'keep' | 'undo') {
		try {
			await post(`/runs/${runId}/review`, { path, decision });
			toast(decision === 'keep' ? `Kept ${path}` : `Undid ${path}`);
			load();
			onchanged();
		} catch (e) {
			toast(message(e), true);
		}
	}
	const pending = $derived(files.filter((f) => f.decision === 'pending').length);
</script>

<div class="stack">
	<div class="row">
		<h2>Review changes</h2><span class="low">{pending ? `${pending} of ${files.length} to decide` : 'All decided'}</span><span class="spacer"></span>
		<Button size="sm" disabled title="Connect GitHub in Settings to open pull requests (coming with the GitHub connection)">Open a pull request</Button>
	</div>
	{#if error}<div class="error">{error}</div>{/if}
	{#each files as f (f.path)}
		<div class="file card stack" data-testid="review-file">
			<div class="row">
				<strong class="mono">{f.path}</strong><span class="chip">{f.kind}</span><span class="spacer"></span>
				{#if f.decision === 'pending'}
					<Button size="sm" variant="primary" onclick={() => decide(f.path, 'keep')}>Keep</Button>
					<Button size="sm" onclick={() => decide(f.path, 'undo')}>Undo</Button>
				{:else}
					<span class="chip">{f.decision === 'kept' ? 'Kept' : 'Undone'}</span>
				{/if}
			</div>
			<div class="diff mono">
				{#each hunks(diffLines(f.before ?? '', f.after ?? '')) as l, i (i)}
					{#if l.kind === 'gap'}<div class="gap">⋯ {l.count} unchanged lines</div>{:else}<div class={l.kind}><span class="n">{l.kind === 'add' ? '+' : l.kind === 'del' ? '−' : ' '}</span>{l.text}</div>{/if}
				{/each}
			</div>
		</div>
	{:else}
		<div class="muted-box">No changes to review.</div>
	{/each}
</div>

<style>
	.diff {
		background: var(--pane);
		border-radius: 8px;
		padding: 8px 0;
		overflow: auto;
		max-height: 360px;
		white-space: pre;
	}
	.diff > div {
		padding: 0 10px;
	}
	.add {
		background: var(--accent-soft);
		color: var(--ink);
	}
	.del {
		color: var(--failed);
	}
	.gap {
		color: var(--low);
		font-style: italic;
	}
	.n {
		display: inline-block;
		width: 16px;
		color: var(--low);
	}
</style>
