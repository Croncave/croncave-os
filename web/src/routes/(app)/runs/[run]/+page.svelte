<script lang="ts">
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import { get, post, message } from '$lib/api';
	import { onLive, throttle } from '$lib/live';
	import { setComputer, session } from '$lib/session.svelte';
	import { appName, between, when } from '$lib/format';
	import Status from '$lib/ui/Status.svelte';
	import Button from '$lib/ui/Button.svelte';
	import LiveOutput from '$lib/ui/LiveOutput.svelte';
	import RunResult from '$lib/ui/RunResult.svelte';
	import WatchDetail from '$lib/apps/WatchDetail.svelte';
	import { toast } from '$lib/ui/toast.svelte';

	// eslint-disable-next-line @typescript-eslint/no-explicit-any
	let run = $state<any>(null);
	let error = $state('');
	const id = $derived(page.params.run ?? '');
	const active = $derived(run && ['queued', 'waiting', 'starting', 'running', 'held'].includes(run.status));

	async function load() {
		try {
			run = await get(`/runs/${id}`);
			if (run.computer_id !== session.computerId) setComputer(run.computer_id);
			// A script's runs live on its page, under Runs.
			if (run.job.app === 'scripts' && run.trigger !== 'test') goto(`/scripts/${run.job.id}?tab=runs&run=${run.id}`, { replaceState: true });
		} catch (e) {
			error = message(e);
		}
	}
	$effect(() => {
		void id;
		load();
	});
	$effect(() => {
		const refresh = throttle(load, 300);
		return onLive((m) => {
			if ((m.kind === 'run' || m.kind === 'progress') && m.id === id) refresh();
			if (m.kind === 'resync') refresh();
		});
	});

	async function stop() {
		try {
			await post(`/runs/${id}/stop`);
			toast('Stopping');
		} catch (e) {
			toast(message(e), true);
		}
	}
	async function answer(req: string, approved: boolean) {
		try {
			await post(`/runs/${id}/approvals/${req}`, { approved });
			load();
		} catch (e) {
			toast(message(e), true);
		}
	}
	const back = $derived(run ? (run.job.app === 'code' ? '/code' : `/${run.job.app}/${run.job.id}`) : '/home');
	const why = (t: string) => ({ schedule: 'On its schedule', manual: 'Started by you', test: 'A test', retry: 'A retry', files: 'Files changed' })[t] ?? t;
</script>

<div class="page">
	{#if error}<div class="banner bad">{error}</div>{/if}
	{#if run}
		<div class="page-head">
			<div class="stack" style="gap: 4px">
				<a href={back} class="low">← {appName(run.job.app)} · {run.job.name}</a>
				<h1>{run.trigger === 'test' ? 'Test of' : 'Run of'} {run.job.name}</h1>
				<div class="row low"><Status status={run.data?.couldnt_check ? 'couldnt_check' : run.status} /><span>{why(run.trigger)}</span><span class="mono">{when(run.queued_at)}</span>{#if run.started_at}<span class="mono">· {between(run.started_at, run.ended_at)}</span>{/if}</div>
			</div>
			<div class="row">
				{#if active}<Button variant="danger" onclick={stop}>Stop</Button>{/if}
			</div>
		</div>
		{#if run.status_note && active}<div class="banner">{run.status_note}{#if run.status === 'held'} <a href="/plans">Raise the cap</a>{/if}</div>{/if}
		{#each run.approvals.filter((a: { status: string }) => a.status === 'pending') as a (a.request_id)}
			<div class="banner" data-testid="approval">
				<Status status="needs_you" />
				<div class="stack" style="gap: 2px"><strong>The agent wants to run <code>{a.command}</code></strong><span class="mid">{a.reason}</span></div>
				<span class="spacer"></span>
				<Button variant="primary" onclick={() => answer(a.request_id, true)}>Approve</Button>
				<Button onclick={() => answer(a.request_id, false)}>Deny</Button>
			</div>
		{/each}
		{#if !active}
			<section class="card"><RunResult {run} /></section>
		{/if}
		{#if run.job.app === 'watcher' && run.data?.detail}
			<section class="card"><WatchDetail data={run.data.detail} /></section>
		{/if}
		{#if run.job.kind === 'agent_task' && run.status === 'succeeded'}
			<div class="banner good"><span>The agent's changes are ready to review.</span><span class="spacer"></span><Button variant="primary" href={`/code?review=${run.id}`}>Review changes</Button></div>
		{/if}
		<section class="stack">
			<div class="row"><h2>Output</h2>{#if run.progress && active}<span class="low mono">CPU {Math.round(run.progress.cpu_percent)}% · {run.progress.memory_mb} MB</span>{/if}</div>
			<LiveOutput runId={run.id} initial={run.output} />
		</section>
	{/if}
</div>
