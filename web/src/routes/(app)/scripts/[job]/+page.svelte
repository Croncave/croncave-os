<script lang="ts">
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import { get, post, patch, del, message } from '$lib/api';
	import { onLive, throttle } from '$lib/live';
	import { when } from '$lib/format';
	import type { Job } from '$lib/types';
	import Status from '$lib/ui/Status.svelte';
	import Button from '$lib/ui/Button.svelte';
	import Modal from '$lib/ui/Modal.svelte';
	import RunList from '$lib/ui/RunList.svelte';
	import ScriptForm from '$lib/apps/ScriptForm.svelte';
	import { toast } from '$lib/ui/toast.svelte';

	let job = $state<Job | null>(null);
	let editing = $state(false);
	const id = $derived(page.params.job ?? '');

	async function load() {
		job = await get(`/jobs/${id}`);
	}
	$effect(() => {
		void id;
		load();
	});
	$effect(() => onLive(throttle(load, 400)));

	async function runNow() {
		try {
			const r = await post(`/jobs/${id}/run`);
			goto(`/runs/${r.run_id}`);
		} catch (e) {
			toast(message(e), true);
		}
	}
	async function setStatus(status: string) {
		try {
			job = await patch(`/jobs/${id}`, { status });
			toast(status === 'paused' ? 'Paused' : 'Resumed');
			load();
		} catch (e) {
			toast(message(e), true);
		}
	}
	async function remove() {
		if (!confirm('Delete this script job? Its history goes too. The script file stays in Files.')) return;
		await del(`/jobs/${id}`);
		goto('/scripts');
	}
	async function removeSecret(name: string) {
		await del(`/jobs/${id}/secrets/${name}`);
		load();
	}
</script>

<div class="page">
	{#if job}
		<div class="page-head">
			<div class="stack" style="gap: 4px">
				<a href="/scripts" class="low">← Scripts</a>
				<h1>{job.name}</h1>
				<div class="row"><Status status={job.status} /><span class="mid">{job.trigger === 'schedule' ? job.schedule_words : job.trigger === 'files' ? `When files in ${job.watch_path || 'your files'} change` : 'By hand'}</span>{#if job.next_due_at}<span class="low mono">next {when(job.next_due_at)}</span>{/if}</div>
			</div>
			<div class="row">
				<Button variant="primary" onclick={runNow}>Run now</Button>
				{#if job.status === 'paused'}<Button onclick={() => setStatus('active')}>Resume</Button>{:else}<Button onclick={() => setStatus('paused')}>Pause</Button>{/if}
				<Button onclick={() => (editing = true)}>Change</Button>
				<Button variant="danger" onclick={remove}>Delete</Button>
			</div>
		</div>
		<section class="card stack">
			<div class="row mono"><span class="low">Runs</span> {job.setup.path} {job.setup.args?.join(' ') ?? ''}</div>
			<div class="row low">Stops after {Math.round(job.max_runtime_secs / 60)} min · {job.retries ? `retries ${job.retries}×` : 'no retries'} · {job.overlap === 'skip' ? 'skips a run if the last is still going' : 'queues behind the last run'}</div>
			{#if job.secret_names.length}
				<div class="row">Secrets: {#each job.secret_names as s (s)}<span class="chip mono">{s} <button class="x" onclick={() => removeSecret(s)} aria-label="Remove {s}">✕</button></span>{/each}</div>
			{/if}
		</section>
		<section class="stack"><h2>History</h2><RunList runs={job.runs ?? []} /></section>
	{/if}
</div>

{#if editing && job}
	<Modal title="Change {job.name}" wide onclose={() => (editing = false)}>
		<ScriptForm {job} onsaved={(_, run) => ((editing = false), load(), run && goto(`/runs/${run}`))} />
	</Modal>
{/if}

<style>
	.x {
		background: none;
		border: 0;
		color: var(--mid);
		cursor: pointer;
		padding: 0;
	}
</style>
