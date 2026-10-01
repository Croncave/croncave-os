<script lang="ts">
	import { untrack } from 'svelte';
	import { get, post, patch, message } from '$lib/api';
	import { session } from '$lib/session.svelte';
	import type { Job } from '$lib/types';
	import Button from '$lib/ui/Button.svelte';
	import Field from '$lib/ui/Field.svelte';
	import SchedulePicker from '$lib/ui/SchedulePicker.svelte';
	import JobLimits from '$lib/ui/JobLimits.svelte';
	import FilePicker from '$lib/ui/FilePicker.svelte';
	import { toast } from '$lib/ui/toast.svelte';

	// Set up (or change) a script job, and see it in plain words before saving.
	let { job = null, minSecs = 60, onsaved }: { job?: Job | null; minSecs?: number; onsaved: (job: Job, run?: string) => void } = $props();

	// The form starts from the job as it was when opened.
	const j0 = untrack(() => job);
	let name = $state(j0?.name ?? '');
	let path = $state(j0?.setup.path ?? '');
	let runtime = $state<string>(j0?.setup.runtime ?? '');
	let args = $state((j0?.setup.args ?? []).join(' '));
	let trigger = $state(j0?.trigger ?? 'manual');
	let schedule = $state(j0?.schedule?.replace(/^0 /, '') ?? '');
	let watchPath = $state(j0?.watch_path ?? '');
	let maxRuntime = $state(j0?.max_runtime_secs ?? 600);
	let retries = $state(j0?.retries ?? 0);
	let overlap = $state(j0?.overlap ?? 'skip');
	let notifyFinished = $state(j0?.notify?.finished ?? false);
	let notifyFailed = $state(j0?.notify?.failed ?? true);
	let secrets = $state<{ name: string; value: string }[]>([]);
	let picking = $state(false);
	let busy = $state(false);
	let error = $state('');
	// eslint-disable-next-line @typescript-eslint/no-explicit-any
	let templates = $state<any[]>([]);

	$effect(() => {
		get('/scripts/templates').then((r) => (templates = r.templates));
	});

	const plain = $derived(
		`Run ${path || 'your script'}${runtime ? ` with ${runtime}` : ''} ` +
			(trigger === 'schedule' ? `on a schedule (${schedule})` : trigger === 'files' ? `when files in ${watchPath || 'a folder'} change` : 'when I press Run now') +
			`, stop it after ${maxRuntime / 60 >= 1 ? `${maxRuntime / 60} min` : `${maxRuntime}s`}` +
			(retries ? `, retry ${retries === 1 ? 'once' : `up to ${retries} times`}` : '') +
			(notifyFailed ? ', and tell me if it fails.' : '.')
	);

	async function useTemplate(id: string) {
		try {
			const r = await post(`/computers/${session.computerId}/scripts/template`, { template: id });
			path = r.entry;
			if (!name) name = r.label.replace(/ \(.*\)$/, '');
			toast(`Added ${r.files.length} files to Files`);
		} catch (e) {
			toast(message(e), true);
		}
	}

	async function save(runNow: boolean) {
		busy = true;
		error = '';
		const opts = {
			name,
			trigger,
			schedule: trigger === 'schedule' ? schedule : undefined,
			watch_path: trigger === 'files' ? watchPath : undefined,
			max_runtime_secs: maxRuntime,
			retries,
			overlap,
			notify: { finished: notifyFinished, failed: notifyFailed, found: true }
		};
		try {
			const setup = { path, runtime: runtime || undefined, args: args.trim() ? args.trim().split(/\s+/) : [] };
			if (job) {
				const saved = await patch(`/jobs/${job.id}`, { ...opts, setup });
				for (const s of secrets.filter((s) => s.name)) await post(`/jobs/${job.id}/secrets`, s);
				let run: string | undefined;
				if (runNow) run = (await post(`/jobs/${job.id}/run`)).run_id;
				onsaved(saved, run);
			} else {
				const r = await post(`/computers/${session.computerId}/scripts`, { ...opts, ...setup, secrets: secrets.filter((s) => s.name), run_now: runNow });
				onsaved(r.job, r.run_id ?? undefined);
			}
		} catch (e) {
			error = message(e);
		} finally {
			busy = false;
		}
	}
</script>

<div class="stack">
	{#if !job && templates.length}
		<div class="pane stack" style="gap: 6px">
			<span class="mid">Start from a sample, or choose your own script below.</span>
			<div class="row">{#each templates as t (t.id)}<Button size="sm" onclick={() => useTemplate(t.id)}>{t.label}</Button>{/each}</div>
		</div>
	{/if}
	<Field label="Name"><input bind:value={name} placeholder="Nightly report" aria-label="Job name" /></Field>
	<Field group label="Script" help="Python (.py), Node.js (.js) or shell (.sh). Packages in requirements.txt or package.json next to it are installed for you.">
		<div class="row" style="flex-wrap: nowrap"><input bind:value={path} placeholder="Scripts/report.py" class="mono" aria-label="Script path" /><Button onclick={() => (picking = true)}>Choose…</Button></div>
	</Field>
	<div class="grid2">
		<Field label="Runs with"><select bind:value={runtime}><option value="">Decide from the file name</option><option value="python">Python</option><option value="node">Node.js</option><option value="shell">Shell</option></select></Field>
		<Field label="Arguments (optional)"><input bind:value={args} class="mono" /></Field>
	</div>
	<Field group label="When it runs"><SchedulePicker bind:trigger bind:schedule bind:watchPath {minSecs} /></Field>
	<JobLimits bind:maxRuntime bind:retries bind:overlap bind:notifyFinished bind:notifyFailed />
	<Field group label="Secrets" help={`Given to the script as environment variables and hidden in its output.${job?.secret_names.length ? ` Saved: ${job.secret_names.join(', ')}.` : ''}`}>
		{#each secrets as s, i (i)}
			<div class="row" style="flex-wrap: nowrap"><input bind:value={s.name} placeholder="API_KEY" class="mono" aria-label="Secret name" /><input bind:value={s.value} type="password" placeholder="value" aria-label="Secret value" /></div>
		{/each}
		<Button size="sm" onclick={() => (secrets = [...secrets, { name: '', value: '' }])}>Add a secret</Button>
	</Field>
	<div class="pane" data-testid="plain-words"><span class="label">In plain words</span><p>{plain}</p></div>
	{#if error}<div class="banner bad">{error}{#if error.includes('plan')} <a href="/plans">See plans</a>{/if}</div>{/if}
	<div class="row">
		<Button variant="primary" onclick={() => save(true)} {busy}>Save and run now</Button>
		<Button onclick={() => save(false)} {busy}>Save</Button>
	</div>
</div>

{#if picking}
	<FilePicker computer={session.computerId} filter={(n) => /\.(py|js|mjs|cjs|sh|bash)$/i.test(n)} onclose={() => (picking = false)} onpick={(p) => ((path = p), (picking = false), !name && (name = p.split('/').pop() ?? ''))} />
{/if}
