<script lang="ts">
	import Status from './Status.svelte';
	import Button from './Button.svelte';
	import { between, when } from '$lib/format';
	import { post, message } from '$lib/api';
	import { toast } from './toast.svelte';
	import { goto } from '$app/navigation';

	// What a run produced: headline, values, files it made, and why it failed with the fix.
	// eslint-disable-next-line @typescript-eslint/no-explicit-any
	let { run }: { run: any } = $props();
	const changes = $derived((run.changes ?? []) as { path: string; kind: string; size: number }[]);
	async function retry() {
		try {
			const r = await post(`/runs/${run.id}/retry`);
			goto(`/runs/${r.run_id}`);
		} catch (e) {
			toast(message(e), true);
		}
	}
</script>

<div class="stack">
	<div class="row">
		<Status status={run.status} />
		<strong data-testid="headline">{run.headline ?? run.status_note ?? ''}</strong>
	</div>
	{#if run.status_note && run.headline}<p class="mid">{run.status_note}</p>{/if}
	{#if run.error_plain}
		<div class="banner bad" data-testid="why">
			<div class="stack" style="gap: 4px">
				<strong>{run.error_plain}</strong>
				{#if run.error_fix}<span class="mid">Fix: {run.error_fix}</span>{/if}
			</div>
			<span class="spacer"></span>
			<Button onclick={retry}>Retry</Button>
		</div>
	{/if}
	{#if run.summary?.values?.length}
		<div class="values">
			{#each run.summary.values as v (v.label)}
				<div class="pane"><div class="label">{v.label}</div><div class="mono">{v.value}</div></div>
			{/each}
		</div>
	{/if}
	{#if changes.length}
		<div>
			<div class="label">Files this run {changes.some((c) => c.kind !== 'deleted') ? 'made or changed' : 'deleted'}</div>
			<div class="list">
				{#each changes as c (c.path)}
					<div class="row">
						<span class="chip">{c.kind}</span>
						{#if c.kind === 'deleted'}<span>{c.path}</span><span class="low">kept in Trash</span>{:else}<a href="/files?path={encodeURIComponent(c.path.split('/').slice(0, -1).join('/'))}&open={encodeURIComponent(c.path)}">{c.path}</a>{/if}
					</div>
				{/each}
			</div>
		</div>
	{/if}
	<div class="low mono small">
		{#if run.started_at}Started {when(run.started_at)} · took {between(run.started_at, run.ended_at)}{/if}
		{#if run.exit_code !== null && run.exit_code !== undefined} · exit code {run.exit_code}{/if}
		{#if run.attempt > 1} · attempt {run.attempt}{/if}
	</div>
</div>

<style>
	.values {
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(150px, 1fr));
		gap: 8px;
	}
	.small {
		font-size: 11.5px;
	}
</style>
