<script lang="ts">
	import { goto } from '$app/navigation';
	import { get, post, patch, message } from '$lib/api';
	import { onLive } from '$lib/live';
	import { session } from '$lib/session.svelte';
	import Button from '$lib/ui/Button.svelte';
	import Field from '$lib/ui/Field.svelte';
	import SchedulePicker from '$lib/ui/SchedulePicker.svelte';
	import RunResult from '$lib/ui/RunResult.svelte';
	import WatchDetail from '$lib/apps/WatchDetail.svelte';
	import { toast } from '$lib/ui/toast.svelte';

	// eslint-disable-next-line @typescript-eslint/no-explicit-any
	let types = $state<any[]>([]);
	// eslint-disable-next-line @typescript-eslint/no-explicit-any
	let type = $state<any>(null);
	let inputs = $state<Record<string, string>>({});
	let name = $state('');
	let trigger = $state('schedule');
	let schedule = $state('');
	let watchPath = $state('');
	let rule = $state('');
	let problems = $state<string[]>([]);
	let draftId = $state('');
	// eslint-disable-next-line @typescript-eslint/no-explicit-any
	let test = $state<any>(null);
	let testing = $state(false);
	let busy = $state(false);
	let error = $state('');
	let minSecs = $state(60);

	$effect(() => {
		get('/watcher/types').then((r) => (types = r.types.filter((t: { approved: boolean }) => t.approved)));
		get('/billing').then((b) => (minSecs = b.effective_plan.min_schedule_secs));
	});

	// eslint-disable-next-line @typescript-eslint/no-explicit-any
	function choose(t: any) {
		type = t;
		inputs = Object.fromEntries(t.config.inputs.map((i: { key: string; default?: unknown }) => [i.key, i.default != null ? String(i.default) : '']));
		name = t.config.name.replace(/ \(.*\)$/, '');
		draftId = '';
		test = null;
	}

	// The setup in plain words, as it's filled in.
	$effect(() => {
		if (!type) return;
		const body = { type_id: type.id, inputs: { ...inputs }, trigger, schedule };
		const t = setTimeout(async () => {
			try {
				const r = await post('/watcher/describe', body);
				rule = r.rule;
				problems = r.problems;
			} catch {
				// Shown on save.
			}
		}, 250);
		return () => clearTimeout(t);
	});

	const opts = () => ({ name, trigger, schedule: trigger === 'schedule' ? schedule : undefined, watch_path: trigger === 'files' ? watchPath : undefined });

	async function runTest() {
		testing = true;
		error = '';
		test = null;
		try {
			if (!draftId) {
				const r = await post(`/computers/${session.computerId}/watches`, { type_id: type.id, inputs, ...opts(), status: 'draft' });
				draftId = r.job.id;
			} else {
				await patch(`/jobs/${draftId}`, { ...opts(), setup: { watch_type: { id: type.id }, inputs } });
			}
			const { run_id } = await post(`/jobs/${draftId}/test`);
			const done = async () => {
				const r = await get(`/runs/${run_id}`);
				if (r.ended_at) {
					test = r;
					testing = false;
					off();
				}
			};
			const off = onLive((m) => m.kind === 'run' && m.id === run_id && done());
			done();
		} catch (e) {
			error = message(e);
			testing = false;
		}
	}

	async function save() {
		busy = true;
		error = '';
		try {
			let id = draftId;
			if (id) {
				await patch(`/jobs/${id}`, { ...opts(), status: 'active', setup: { watch_type: { id: type.id }, inputs } });
				await post(`/jobs/${id}/run`);
			} else {
				const r = await post(`/computers/${session.computerId}/watches`, { type_id: type.id, inputs, ...opts(), check_now: true });
				id = r.job.id;
			}
			toast('Saved. The first check is running.');
			goto(`/watcher/${id}`);
		} catch (e) {
			error = message(e);
		} finally {
			busy = false;
		}
	}
</script>

<div class="page">
	<a href="/watcher" class="low">← Watcher</a>
	<h1>New watch</h1>
	{#if !type}
		<div class="grid2">
			{#each types as t (t.id)}
				<button class="card type" onclick={() => choose(t)} data-type={t.id}>
					<strong>{t.config.name}</strong>
					<span class="mid">{t.config.description}</span>
					<span class="low small">{t.reads}</span>
				</button>
			{/each}
		</div>
	{:else}
		<section class="card stack">
			<div class="row"><h2>{type.config.name}</h2><span class="spacer"></span><Button size="sm" variant="quiet" onclick={() => (type = null)}>Choose another type</Button></div>
			<span class="low">{type.reads}</span>
			<Field label="Name"><input bind:value={name} aria-label="Watch name" /></Field>
			{#each type.config.inputs as i (i.key)}
				<Field label={i.label} help={i.help}>
					{#if i.kind.type === 'select'}
						<select bind:value={inputs[i.key]} aria-label={i.label}>{#each i.kind.options as o (o)}<option value={o}>{o}</option>{/each}</select>
					{:else}
						<input bind:value={inputs[i.key]} inputmode={i.kind.type === 'number' ? 'decimal' : undefined} aria-label={i.label} />
					{/if}
				</Field>
			{/each}
			<Field label="When to check"><SchedulePicker bind:trigger bind:schedule bind:watchPath {minSecs} /></Field>
			<div class="pane" data-testid="plain-words"><span class="label">In plain words</span><p>{rule}</p>{#each problems as p (p)}<p class="error">{p}</p>{/each}</div>
			{#if error}<div class="banner bad">{error}{#if error.includes('plan')} <a href="/plans">See plans</a>{/if}</div>{/if}
			<div class="row">
				<Button onclick={runTest} busy={testing}>Test it now</Button>
				<Button variant="primary" onclick={save} {busy}>Save and start watching</Button>
				<span class="low">A test checks for real, saves nothing and tells no one.</span>
			</div>
		</section>
		{#if test}
			<section class="card stack" data-testid="test-result">
				<h2>Test result</h2>
				<RunResult run={test} />
				{#if test.data?.detail}<WatchDetail data={test.data.detail} />{/if}
			</section>
		{/if}
	{/if}
</div>

<style>
	.type {
		text-align: left;
		color: var(--ink);
		font: inherit;
		cursor: pointer;
		display: grid;
		gap: 6px;
	}
	.type:hover {
		border-color: var(--accent);
	}
	.small {
		font-size: 12px;
	}
</style>
