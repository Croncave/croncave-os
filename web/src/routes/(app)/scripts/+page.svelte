<script lang="ts">
	import { goto } from '$app/navigation';
	import { get } from '$lib/api';
	import { onLive, throttle } from '$lib/live';
	import { session } from '$lib/session.svelte';
	import { ago, when } from '$lib/format';
	import type { Job } from '$lib/types';
	import Status from '$lib/ui/Status.svelte';
	import Button from '$lib/ui/Button.svelte';
	import Modal from '$lib/ui/Modal.svelte';
	import ScriptForm from '$lib/apps/ScriptForm.svelte';

	let jobs = $state<Job[]>([]);
	let adding = $state(false);
	let minSecs = $state(60);

	async function load() {
		if (!session.computerId) return;
		jobs = (await get(`/computers/${session.computerId}/jobs?app=scripts`)).jobs;
		const b = await get('/billing');
		minSecs = b.effective_plan.min_schedule_secs;
	}
	$effect(() => {
		void session.computerId;
		load();
	});
	$effect(() => onLive(throttle(load, 500)));
</script>

<div class="page">
	<div class="page-head">
		<div class="stack" style="gap: 4px"><h1>Scripts</h1><p class="mid">Your own Python, Node.js and shell scripts, by hand, on a schedule, or when files change.</p></div>
		<Button variant="primary" onclick={() => (adding = true)}>Add a script</Button>
	</div>
	{#if jobs.length}
		<section class="card">
			<table>
				<thead><tr><th>Script</th><th>When</th><th>Last run</th><th>Next</th></tr></thead>
				<tbody>
					{#each jobs as j (j.id)}
						<tr class="click" onclick={() => goto(`/scripts/${j.id}`)}>
							<td><a href="/scripts/{j.id}">{j.name}</a><div class="low mono small">{j.setup.path}</div></td>
							<td class="mid">{j.status === 'paused' ? 'Paused' : j.trigger === 'schedule' ? j.schedule_words : j.trigger === 'files' ? `When ${j.watch_path || 'files'} change` : 'By hand'}</td>
							<td>{#if j.last_run}<Status status={j.last_run.status} /> <span class="low">{j.last_run.headline ?? ''} · {ago(j.last_run.ended_at ?? j.last_run.queued_at)}</span>{:else}<span class="low">Never</span>{/if}</td>
							<td class="mono low">{when(j.next_due_at)}</td>
						</tr>
					{/each}
				</tbody>
			</table>
		</section>
	{:else}
		<div class="muted-box">No scripts yet. Add one, or start from a sample.</div>
	{/if}
</div>

{#if adding}
	<Modal title="Add a script" wide onclose={() => (adding = false)}>
		<ScriptForm {minSecs} onsaved={(job, run) => ((adding = false), goto(run ? `/runs/${run}` : `/scripts/${job.id}`))} />
	</Modal>
{/if}

<style>
	.small {
		font-size: 11.5px;
	}
</style>
