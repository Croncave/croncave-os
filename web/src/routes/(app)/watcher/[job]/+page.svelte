<script lang="ts">
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import { get, post, patch, del, message } from '$lib/api';
	import { onLive, throttle } from '$lib/live';
	import { ago, when } from '$lib/format';
	import type { Job, RunBrief } from '$lib/types';
	import Status from '$lib/ui/Status.svelte';
	import Button from '$lib/ui/Button.svelte';
	import Chart from '$lib/ui/Chart.svelte';
	import RunList from '$lib/ui/RunList.svelte';
	import Modal from '$lib/ui/Modal.svelte';
	import SchedulePicker from '$lib/ui/SchedulePicker.svelte';
	import WatchDetail from '$lib/apps/WatchDetail.svelte';
	import { toast } from '$lib/ui/toast.svelte';

	let job = $state<Job | null>(null);
	let editing = $state(false);
	let trigger = $state('schedule');
	let schedule = $state('');
	let watchPath = $state('');
	const id = $derived(page.params.job ?? '');

	async function load() {
		job = await get(`/jobs/${id}`);
	}
	$effect(() => {
		void id;
		load();
	});
	$effect(() => onLive(throttle(load, 400)));

	const runs = $derived((job?.runs ?? []).filter((r) => r.trigger !== 'test'));
	const latest = $derived(runs.find((r) => r.ended_at && r.status === 'succeeded'));
	const kind = (r: RunBrief) => (r.data?.couldnt_check ? 'couldnt' : r.data?.matched ? 'match' : r.status === 'succeeded' ? 'same' : r.ended_at ? 'couldnt' : 'pending');

	async function check() {
		try {
			const r = await post(`/jobs/${id}/run`);
			goto(`/runs/${r.run_id}`);
		} catch (e) {
			toast(message(e), true);
		}
	}
	async function setStatus(status: string) {
		await patch(`/jobs/${id}`, { status });
		load();
	}
	async function saveWhen() {
		try {
			await patch(`/jobs/${id}`, { trigger, schedule: trigger === 'schedule' ? schedule : undefined, watch_path: watchPath });
			editing = false;
			load();
		} catch (e) {
			toast(message(e), true);
		}
	}
	async function remove() {
		if (!confirm('Delete this watch and its history?')) return;
		await del(`/jobs/${id}`);
		goto('/watcher');
	}
</script>

<div class="page">
	{#if job}
		<div class="page-head">
			<div class="stack" style="gap: 4px">
				<a href="/watcher" class="low">← Watcher</a>
				<h1>{job.name}</h1>
				<p class="mid" data-testid="rule">
					{#if job.rule}{job.rule}{/if}
					<button class="link" onclick={() => ((trigger = job!.trigger), (schedule = job!.schedule?.replace(/^0 /, '') ?? ''), (editing = true))}>change when</button>
				</p>
			</div>
			<div class="row">
				<Button variant="primary" onclick={check}>Check now</Button>
				{#if job.status === 'paused'}<Button onclick={() => setStatus('active')}>Resume</Button>{:else}<Button onclick={() => setStatus('paused')}>Pause</Button>{/if}
				<Button variant="danger" onclick={remove}>Delete</Button>
			</div>
		</div>
		<section class="card stack">
			<div class="row"><h2>At a glance</h2><span class="spacer"></span>{#if job.next_due_at && job.status === 'active'}<span class="low mono">next check {when(job.next_due_at)}</span>{/if}</div>
			<div class="dots" aria-label="Check history">
				{#each [...runs].reverse() as r (r.id)}<a href="/runs/{r.id}" class="d {kind(r)}" title="{r.headline ?? r.status} · {ago(r.ended_at ?? r.queued_at)}"></a>{/each}
			</div>
			<div class="row low small"><span class="d match"></span>found something <span class="d same"></span>no change <span class="d couldnt"></span>couldn't check</div>
			{#if latest?.data?.couldnt_check === undefined && runs[0]?.data?.couldnt_check}
				<div class="banner bad"><Status status="couldnt_check" /><span>{runs[0].error_plain}</span></div>
			{/if}
		</section>
		{#if latest?.data?.detail?.history}
			<section class="card stack"><h2>{job.setup.inputs.symbol} price</h2><Chart points={latest.data.detail.history.map((h: [number, number]) => ({ x: h[0], y: h[1] }))} threshold={latest.data.detail.limit} /></section>
		{/if}
		{#if latest?.data?.detail}
			<section class="card stack"><h2>Latest check</h2><p class="mid">{latest.headline} · {ago(latest.ended_at)}</p><WatchDetail data={latest.data.detail} /></section>
		{/if}
		<section class="stack"><h2>History</h2><RunList runs={runs} empty="Not checked yet." /></section>
	{/if}
</div>

{#if editing}
	<Modal title="When to check" onclose={() => (editing = false)}>
		<SchedulePicker bind:trigger bind:schedule bind:watchPath />
		<div class="row"><Button variant="primary" onclick={saveWhen}>Save</Button></div>
	</Modal>
{/if}

<style>
	.dots {
		display: flex;
		gap: 4px;
		flex-wrap: wrap;
	}
	.d {
		width: 12px;
		height: 12px;
		border-radius: 3px;
		display: inline-block;
		background: var(--asleep);
	}
	.d.match {
		background: var(--live);
	}
	.d.couldnt {
		background: var(--failed);
	}
	.d.pending {
		background: var(--working);
	}
	.small {
		font-size: 12px;
	}
	.link {
		background: none;
		border: 0;
		color: var(--accent-ink);
		cursor: pointer;
		font: inherit;
		padding: 0;
	}
</style>
