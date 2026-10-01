<script lang="ts">
	import { goto } from '$app/navigation';
	import { get } from '$lib/api';
	import { onChange, throttle } from '$lib/live';
	import { session } from '$lib/session.svelte';
	import { ago, when } from '$lib/format';
	import type { Job } from '$lib/types';
	import Status from '$lib/ui/Status.svelte';
	import Button from '$lib/ui/Button.svelte';

	let jobs = $state<Job[]>([]);
	async function load() {
		if (session.computerId) jobs = (await get(`/computers/${session.computerId}/jobs?app=watcher`)).jobs;
	}
	$effect(() => {
		void session.computerId;
		load();
	});
	$effect(() => onChange(throttle(load, 500)));
	const stateOf = (j: Job) => (j.status === 'paused' ? 'paused' : j.last_run?.data?.couldnt_check ? 'couldnt_check' : (j.last_run?.status ?? 'asleep'));
</script>

<div class="page">
	<div class="page-head">
		<div class="stack" style="gap: 4px"><h1>Watcher</h1><p class="mid">Watch pages, prices and listings. Your computer checks on a schedule and tells you when something changes.</p></div>
		<Button variant="primary" href="/watcher/new">New watch</Button>
	</div>
	<div class="grid2">
		{#each jobs as j (j.id)}
			<button class="card watch" onclick={() => goto(`/watcher/${j.id}`)} data-testid="watch">
				<div class="row"><strong>{j.name}</strong><span class="spacer"></span><Status status={stateOf(j)} word={j.status === 'paused' ? 'Paused' : stateOf(j) === 'succeeded' ? 'Watching' : undefined} /></div>
				<div class="low">{j.setup.watch_type?.name}</div>
				{#if j.last_run}<div class="mid">{j.last_run.headline} <span class="low">· {ago(j.last_run.ended_at ?? j.last_run.queued_at)}</span></div>{:else}<div class="low">Not checked yet</div>{/if}
				<div class="low mono small">{j.trigger === 'schedule' ? `${j.schedule_words} · next ${when(j.next_due_at)}` : 'Checks when you press Check now'}</div>
			</button>
		{:else}
			<div class="muted-box">No watches yet. Pick a type: a web page, the demo listings site or a stock price.</div>
		{/each}
	</div>
</div>

<style>
	.watch {
		text-align: left;
		color: var(--ink);
		font: inherit;
		cursor: pointer;
		display: grid;
		gap: 6px;
	}
	.watch:hover {
		border-color: var(--accent);
	}
	.small {
		font-size: 11.5px;
	}
</style>
