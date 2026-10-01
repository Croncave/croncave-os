<script lang="ts">
	import { untrack } from 'svelte';
	import { get, patch, post, message } from '$lib/api';
	import { onChange, onLive, throttle } from '$lib/live';
	import { session } from '$lib/session.svelte';
	import { ago, bytes, span } from '$lib/format';
	import { uploadFile } from '$lib/upload';
	import type { Job } from '$lib/types';
	import Button from '$lib/ui/Button.svelte';
	import Icon from '$lib/ui/Icon.svelte';
	import FilePicker from '$lib/ui/FilePicker.svelte';
	import FolderPicker from '$lib/ui/FolderPicker.svelte';
	import Output from './Output.svelte';
	import { findPackages, runtimeName, runtimeOf, scripts } from './store.svelte';

	// Add a script, or change one (its Settings tab): five sections that fold away as
	// they're filled in, the setup in plain words, and a test run before it's saved.
	let { job = null, onsaved }: { job?: Job | null; onsaved: (job: Job) => void } = $props();

	const j0 = untrack(() => job);
	const computer = $derived(job?.computer_id ?? session.computerId);
	const c = $derived(session.me?.computers.find((x) => x.id === computer));
	const versions = $derived<Record<string, string>>((c?.health as { runtimes?: Record<string, string> })?.runtimes ?? {});

	// --- The setup -----------------------------------------------------------------------
	let path = $state<string>(j0?.setup.path ?? '');
	let name = $state(j0?.name ?? '');
	let fileInfo = $state<{ size: number } | null>(null);
	let runtime = $state<string>(j0?.setup.runtime ?? '');
	let install = $state<boolean>(j0?.setup.install ?? true);
	let args = $state(((j0?.setup.args as string[]) ?? []).join(' '));
	let secrets = $state<{ name: string; value: string }[]>([]);
	let trigger = $state(j0?.trigger ?? 'manual');
	let schedule = $state(j0?.schedule?.replace(/^0 /, '') ?? '');
	let customSchedule = $state(false);
	let windowOn = $state(j0?.window_start != null);
	let from = $state(toTime(j0?.window_start ?? 360));
	let to = $state(toTime(j0?.window_end ?? 1440));
	let watchPath = $state(j0?.watch_path ?? '');
	let settle = $state((j0?.settle_secs ?? 60) > 0);
	let tell = $state(j0 ? (j0.notify?.failed === false ? 'never' : j0.notify?.finished ? 'always' : 'fails') : 'fails');
	let maxMinutes = $state(Math.max(1, Math.round((j0?.max_runtime_secs ?? 600) / 60)));
	let retries = $state(j0?.retries ?? 0);
	let overlap = $state(j0?.overlap ?? 'skip');

	let open = $state<Record<number, boolean>>(j0 ? {} : { 1: true });
	let picking = $state<'file' | 'folder' | null>(null);
	let error = $state('');
	let busy = $state(false);
	// eslint-disable-next-line @typescript-eslint/no-explicit-any
	let templates = $state<any[]>([]);
	let packages = $state<{ names: string[]; from: string } | null>(null);
	let uploadEl = $state<HTMLInputElement>();

	function toTime(min: number) {
		const m = min % 1440;
		return `${String(Math.floor(m / 60)).padStart(2, '0')}:${String(m % 60).padStart(2, '0')}`;
	}
	const toMin = (t: string, end = false) => {
		const [h, m] = t.split(':').map(Number);
		const v = (h || 0) * 60 + (m || 0);
		return end && v === 0 ? 1440 : v;
	};
	const timeWords = (t: string) => {
		const v = toMin(t) % 1440;
		if (v === 0) return 'midnight';
		if (v === 720) return 'noon';
		const h = Math.floor(v / 60);
		const m = v % 60;
		return `${h % 12 || 12}${m ? ':' + String(m).padStart(2, '0') : ''} ${h < 12 ? 'AM' : 'PM'}`;
	};

	$effect(() => {
		if (!j0) get('/scripts/templates').then((r) => (templates = r.templates));
	});

	const file = $derived(path.split('/').pop() ?? '');
	const folder = $derived(path.split('/').slice(0, -1).join('/'));
	const rt = $derived(runtimeOf(path, runtime));
	const rtName = $derived(runtimeName(rt, versions));

	// The file's size, and the packages it needs (from requirements.txt or package.json beside it).
	$effect(() => {
		const p = path;
		const dir = folder;
		const kind = rt;
		fileInfo = null;
		packages = null;
		if (!p || !computer) return;
		get(`/computers/${computer}/files?path=${encodeURIComponent(dir)}`)
			.then((r) => (fileInfo = r.entries.find((e: { path: string }) => e.path === p) ?? null))
			.catch(() => {});
		findPackages(computer, p, kind).then((r) => (packages = r));
	});

	// --- Schedules -----------------------------------------------------------------------
	const presets = $derived(
		[
			{ cron: '*/15 * * * *', label: 'every 15 minutes', secs: 900 },
			{ cron: '0 * * * *', label: 'every hour', secs: 3600 },
			{ cron: '0 */3 * * *', label: 'every 3 hours', secs: 10800 },
			{ cron: '0 */6 * * *', label: 'every 6 hours', secs: 21600 },
			{ cron: '0 9 * * *', label: 'every day at 9 AM', secs: 86400 },
			{ cron: '0 9 * * 1-5', label: 'weekdays at 9 AM', secs: 86400 }
		].map((p) => ({ ...p, off: p.secs < scripts.minSecs }))
	);
	const preset = $derived(presets.find((p) => p.cron === schedule));
	$effect(() => {
		if (trigger === 'schedule' && !schedule) schedule = presets.find((p) => !p.off)?.cron ?? '0 * * * *';
	});
	$effect(() => {
		if (schedule && !preset) customSchedule = true;
	});

	// --- In plain words ------------------------------------------------------------------
	const tellWords = $derived(tell === 'fails' ? 'Only tell me if it fails.' : tell === 'always' ? 'Tell me every time it finishes.' : "Don't tell me.");
	const windowWords = $derived(windowOn ? ` from ${timeWords(from)} to ${timeWords(to)}` : '');
	const argWords = $derived(args.trim() ? args.trim() : '');

	async function chooseTemplate(id: string) {
		error = '';
		try {
			const r = await post(`/computers/${computer}/scripts/template`, { template: id });
			path = r.entry;
			if (!name) name = r.label.replace(/ \(.*\)$/, '');
			open = { ...open, 1: false, 2: true };
		} catch (e) {
			error = message(e);
		}
	}
	async function uploaded(files: FileList | null) {
		const f = files?.[0];
		if (!f || !computer) return;
		error = '';
		try {
			const dest = `Scripts/${f.name}`;
			await uploadFile(computer, f, dest, () => {});
			path = dest;
			if (!name) name = f.name;
			open = { ...open, 1: false, 2: true };
		} catch (e) {
			error = message(e);
		}
	}

	// --- Test run and save ---------------------------------------------------------------
	let draft = $state<Job | null>(null);
	// eslint-disable-next-line @typescript-eslint/no-explicit-any
	let test = $state<any>(null);
	let testId = $state('');
	let fullOutput = $state(false);
	const target = $derived(job ?? draft);

	function body() {
		const sixCron = trigger === 'schedule' ? schedule.trim() : undefined;
		return {
			name: name.trim() || file,
			trigger,
			schedule: sixCron,
			watch_path: trigger === 'files' ? watchPath : undefined,
			window: trigger === 'schedule' && windowOn ? { start_min: toMin(from), end_min: toMin(to, true) } : {},
			settle_secs: trigger === 'files' && settle ? 60 : 0,
			max_runtime_secs: maxMinutes * 60,
			retries,
			overlap,
			notify: { failed: tell !== 'never', finished: tell === 'always', found: true }
		};
	}
	const setup = () => ({ path, runtime: runtime || undefined, args: args.trim() ? args.trim().split(/\s+/) : [], install });

	async function saveSecrets(id: string) {
		for (const s of secrets.filter((s) => s.name.trim())) await post(`/jobs/${id}/secrets`, { name: s.name.trim(), value: s.value });
		secrets = [];
	}

	async function runTest() {
		if (!path) return ((error = 'Choose a script first.'), (open = { 1: true }));
		busy = true;
		error = '';
		fullOutput = false;
		try {
			if (target) {
				await patch(`/jobs/${target.id}`, { ...body(), setup: setup() });
				await saveSecrets(target.id);
				testId = (await post(`/jobs/${target.id}/test`)).run_id;
			} else {
				const r = await post(`/computers/${computer}/scripts`, {
					...body(),
					...setup(),
					secrets: secrets.filter((s) => s.name.trim()),
					status: 'draft',
					test_now: true
				});
				secrets = [];
				draft = r.job;
				testId = r.run_id;
			}
			await loadTest();
		} catch (e) {
			error = message(e);
		} finally {
			busy = false;
		}
	}
	async function loadTest() {
		if (testId) test = await get(`/runs/${testId}`);
	}
	$effect(() => onChange(throttle(loadTest, 400)));
	$effect(() => {
		const refresh = throttle(loadTest, 1000);
		return onLive((m) => m.kind === 'progress' && m.id === testId && refresh());
	});

	async function save() {
		if (!path) return ((error = 'Choose a script first.'), (open = { 1: true }));
		busy = true;
		error = '';
		try {
			let saved: Job;
			if (target) {
				const status = target.status === 'draft' ? 'active' : undefined;
				saved = await patch(`/jobs/${target.id}`, { ...body(), setup: setup(), ...(status ? { status } : {}) });
				await saveSecrets(saved.id);
			} else {
				saved = (await post(`/computers/${computer}/scripts`, { ...body(), ...setup(), secrets: secrets.filter((s) => s.name.trim()) })).job;
				secrets = [];
			}
			onsaved(saved);
		} catch (e) {
			error = message(e);
		} finally {
			busy = false;
		}
	}

	const testing = $derived(test && ['queued', 'waiting', 'starting', 'running'].includes(test.status));
	const testTook = $derived(test?.started_at && test?.ended_at ? (new Date(test.ended_at).getTime() - new Date(test.started_at).getTime()) / 1000 : null);
	const testFiles = $derived<{ path: string; size: number; kind: string }[]>((test?.changes ?? []).filter((x: { kind: string }) => x.kind !== 'deleted').slice(0, 5));
	const testTail = $derived<{ text: string }[]>((test?.output ?? []).slice(-3));

	// --- Section summaries ---------------------------------------------------------------
	const sum = $derived({
		1: path ? file : 'Choose the file to run',
		2: path ? `${rtName}${packages?.names.length ? ` · ${packages.names.length} ${packages.names.length === 1 ? 'package' : 'packages'} to install` : ''}` : 'Python, Node.js or shell',
		3: [args.trim() ? 'Arguments' : '', (job?.secret_names.length ?? 0) + secrets.filter((s) => s.name).length ? 'secrets' : ''].filter(Boolean).join(', ') || 'None',
		4:
			trigger === 'schedule'
				? `${preset ? preset.label[0].toUpperCase() + preset.label.slice(1) : `On the schedule ${schedule}`}${windowWords}`
				: trigger === 'files'
					? `When new files land in ${watchPath || 'a folder'}`
					: 'Only when I press Run',
		5: tell === 'fails' ? 'Only if it fails' : tell === 'always' ? 'Every time it finishes' : 'Never'
	});
	const done = $derived({ 1: !!path, 2: !!path, 3: true, 4: trigger !== 'files' || !!watchPath, 5: true });
	const toggle = (n: number) => (open = { ...open, [n]: !open[n] });
</script>

{#snippet head(n: 1 | 2 | 3 | 4 | 5, title: string)}
	<button class="sh" onclick={() => toggle(n)} aria-expanded={!!open[n]}>
		<span class="num" class:ok={done[n] && !open[n]}>{#if done[n] && !open[n]}<Icon name="check" size={13} />{:else}{n}{/if}</span>
		<span class="st"><span class="stt">{title}</span><span class="sts">{sum[n]}</span></span>
		<span class="chev" class:up={open[n]}><Icon name="chevron-down" size={16} /></span>
	</button>
{/snippet}

<div class="add">
	<div class="main">
		{#if !job}
			<header><h1>Add a script</h1><p>Run your own code by hand, on a schedule, or when something changes.</p></header>
		{/if}

		<section class="sec">
			{@render head(1, 'Script')}
			{#if open[1]}
				<div class="body">
					{#if path}
						<div class="filecard">
							<Icon name="file" size={16} />
							<span class="fc"><span class="mono fn">{file}</span><span class="fm">Files{#each folder.split('/').filter(Boolean) as part, i (i)} › {part}{/each}{fileInfo ? ` · ${bytes(fileInfo.size)}` : ''} · {rtName}</span></span>
							<Button variant="quiet" size="sm" onclick={() => (picking = 'file')}>Change</Button>
						</div>
					{:else}
						<div class="empty">
							<Button icon="folder" onclick={() => (picking = 'file')}>Choose from Files</Button>
							<Button icon="upload" onclick={() => uploadEl?.click()}>Upload a file</Button>
						</div>
					{/if}
					<label class="nm"><span>Name</span><input bind:value={name} placeholder={file || 'Nightly report'} aria-label="Script name" /></label>
					<p class="hint">Or <button class="link" onclick={() => uploadEl?.click()}>upload a file</button>, or pick one from Files. Python, Node.js and shell scripts work.</p>
					{#if templates.length && !path}
						<div class="samples"><span class="hint">Start from a sample:</span>{#each templates as t (t.id)}<Button size="sm" onclick={() => chooseTemplate(t.id)}>{t.label}</Button>{/each}</div>
					{/if}
					<input class="hide" type="file" accept=".py,.js,.mjs,.cjs,.sh,.bash" bind:this={uploadEl} onchange={(e) => uploaded(e.currentTarget.files)} data-testid="script-upload" />
				</div>
			{/if}
		</section>

		<section class="sec">
			{@render head(2, 'What it needs')}
			{#if open[2]}
				<div class="body">
					<div class="table">
						<div class="tr">
							<span>Runs with</span>
							<select bind:value={runtime} aria-label="Runs with">
								<option value="">{rt ? `${runtimeName(rt, versions)} (from the file name)` : 'Decide from the file name'}</option>
								<option value="python">{runtimeName('python', versions)}</option>
								<option value="node">{runtimeName('node', versions)}</option>
								<option value="shell">{runtimeName('shell', versions)}</option>
							</select>
						</div>
						<div class="tr">
							<span>Packages</span>
							<span class="pk">
								{#if packages?.names.length}<span class="mono">{packages.names.join(', ')}</span> <span class="mid">found in {packages.from}</span>
								{:else}<span class="mid">None found{rt === 'python' ? ' (add a requirements.txt beside it)' : rt === 'node' ? ' (add a package.json beside it)' : ''}</span>{/if}
							</span>
						</div>
					</div>
					<label class="tick"><input type="checkbox" bind:checked={install} /> Install packages before the first run</label>
				</div>
			{/if}
		</section>

		<section class="sec">
			{@render head(3, 'Inputs')}
			{#if open[3]}
				<div class="body two">
					<label class="fl"><span>Arguments</span><input class="mono" bind:value={args} placeholder="--since 24h" aria-label="Arguments" /></label>
					<div class="fl">
						<span>Secrets</span>
						{#if job?.secret_names.length}<span class="saved">{#each job.secret_names as s (s)}<span class="chip mono"><Icon name="key" size={12} /> {s}</span>{/each}</span>{/if}
						{#each secrets as s, i (i)}
							<div class="pair"><input class="mono" bind:value={s.name} placeholder="API_KEY" aria-label="Secret name" /><input type="password" bind:value={s.value} placeholder="Value" aria-label="Secret value" /></div>
						{/each}
						<button class="addsec" onclick={() => (secrets = [...secrets, { name: '', value: '' }])}>{secrets.length || job?.secret_names.length ? 'Add another secret' : 'None needed'}<Icon name="plus" size={13} /></button>
						<span class="hint">Given to the script as environment variables, hidden in its output.</span>
					</div>
				</div>
			{/if}
		</section>

		<section class="sec">
			{@render head(4, 'When to run')}
			{#if open[4]}
				<div class="body">
					<div class="seg" role="radiogroup" aria-label="When to run">
						{#each [['manual', 'Only when I press Run'], ['schedule', 'On a schedule'], ['files', 'When files change']] as [v, l] (v)}
							<button role="radio" aria-checked={trigger === v} class:on={trigger === v} onclick={() => (trigger = v)}>{l}</button>
						{/each}
					</div>
					{#if trigger === 'schedule'}
						<div class="line">
							<span class="mid">Run</span>
							{#if customSchedule}
								<input class="mono cron" bind:value={schedule} placeholder="0 */3 * * *" aria-label="Cron schedule" />
							{:else}
								<select bind:value={schedule} aria-label="How often">
									{#each presets as p (p.cron)}<option value={p.cron} disabled={p.off}>{p.label}{p.off ? ' (bigger plan)' : ''}</option>{/each}
								</select>
							{/if}
							<button class="link" onclick={() => (customSchedule = !customSchedule)}>{customSchedule ? 'Choose from a list' : 'Write a custom schedule'}</button>
						</div>
						{#if customSchedule}<p class="hint">Minute, hour, day of month, month, day of week, in the computer's time zone.</p>{/if}
						<div class="line">
							<label class="tick"><input type="checkbox" bind:checked={windowOn} /> Only between</label>
							<input type="time" bind:value={from} disabled={!windowOn} aria-label="Start time" />
							<span class="mid">and</span>
							<input type="time" bind:value={to} disabled={!windowOn} aria-label="End time" />
						</div>
					{:else if trigger === 'files'}
						<div class="line">
							<span class="mid">Watch the folder</span>
							<button class="pickbtn" onclick={() => (picking = 'folder')} aria-label="Watch the folder">{watchPath || 'Choose a folder'}<Icon name="chevron-down" size={14} /></button>
							<label class="tick"><input type="checkbox" bind:checked={settle} /> Wait 1 minute for more files</label>
						</div>
					{/if}
				</div>
			{/if}
		</section>

		<section class="sec">
			{@render head(5, 'Tell me')}
			{#if open[5]}
				<div class="body">
					<div class="seg" role="radiogroup" aria-label="Tell me">
						{#each [['fails', 'Only if it fails'], ['always', 'Every time it finishes'], ['never', 'Never']] as [v, l] (v)}
							<button role="radio" aria-checked={tell === v} class:on={tell === v} onclick={() => (tell = v)}>{l}</button>
						{/each}
					</div>
					<div class="table">
						<div class="tr"><span>Stop it after</span><span class="inl"><input type="number" min="1" bind:value={maxMinutes} aria-label="Stop it after (minutes)" /> minutes</span></div>
						<div class="tr">
							<span>If it fails</span>
							<select bind:value={retries} aria-label="If it fails"><option value={0}>Don't try again</option><option value={1}>Try once more</option><option value={2}>Try up to 2 more times</option><option value={3}>Try up to 3 more times</option></select>
						</div>
						<div class="tr">
							<span>If the last run is still going</span>
							<select bind:value={overlap} aria-label="If the last run is still going"><option value="skip">Skip this run</option><option value="queue">Run it after</option></select>
						</div>
					</div>
				</div>
			{/if}
		</section>
	</div>

	<aside class="side">
		<div class="box" data-testid="plain-words">
			<span class="eyebrow">In plain words</span>
			<p class="pw">
				{#if trigger === 'files'}
					When <b>new files land in {watchPath || 'a folder'}</b>,{settle ? ' wait a minute, then' : ''} run <b class="mono">{file || 'your script'}</b> on them{argWords ? ' with ' : ''}{#if argWords}<b class="mono">{argWords}</b>{/if}.
				{:else if trigger === 'schedule'}
					Run <b class="mono">{file || 'your script'}</b> <b>{preset?.label ?? `on the schedule ${schedule}`}{windowWords}</b>{argWords ? ' with ' : ''}{#if argWords}<b class="mono">{argWords}</b>{/if}.
				{:else}
					Run <b class="mono">{file || 'your script'}</b>{argWords ? ' with ' : ''}{#if argWords}<b class="mono">{argWords}</b>{/if} <b>only when you press Run</b>.
				{/if}
				<b>{tellWords}</b>
			</p>
		</div>

		<div class="box" data-testid="test-run">
			<div class="th"><span class="eyebrow">Test run</span>{#if test?.ended_at}<span class="when mono">{ago(test.ended_at)}</span>{/if}</div>
			{#if !test}
				<p class="tp">Try it once before saving. The test runs on {c?.name ?? 'your computer'} and doesn't count toward its history.</p>
			{:else if testing}
				<p class="tp"><span class="dot"></span> Running… {test.progress?.total ? `${Math.round(((test.progress.done ?? 0) / test.progress.total) * 100)}%` : ''}</p>
				{#key testId}<Output runId={testId} initial={test.output ?? []} tail={4} height={120} />{/key}
			{:else if test.status === 'succeeded'}
				<p class="tp">{(test.headline ?? 'It worked').replace(/([^.!?])$/, '$1.')}{testTook != null ? ` Took ${testTook < 1 ? 'under a second' : span(testTook)}.` : ''}</p>
				{#if testFiles.length}
					<div class="tf">{#each testFiles as f (f.path)}<div class="tfr"><span class="mono">{f.path.split('/').pop()}</span><span class="mid">{f.kind === 'created' ? 'new' : 'changed'} · {bytes(f.size)}</span></div>{/each}</div>
				{:else if testTail.length}
					<div class="tf">{#each testTail as l, i (i)}<div class="tfr"><span class="mono">{l.text}</span></div>{/each}</div>
				{/if}
				<button class="link" onclick={() => (fullOutput = !fullOutput)}>{fullOutput ? 'Hide output' : 'See full output'}</button>
				{#if fullOutput}<Output runId={testId} initial={test.output ?? []} height={220} live={false} />{/if}
			{:else}
				<p class="tp bad" data-testid="why">{test.error_plain ?? "It didn't finish."}</p>
				{#if test.error_fix}<p class="tp">{test.error_fix}</p>{/if}
				<button class="link" onclick={() => (fullOutput = !fullOutput)}>{fullOutput ? 'Hide output' : 'See full output'}</button>
				{#if fullOutput}<Output runId={testId} initial={test.output ?? []} height={220} live={false} />{/if}
			{/if}
		</div>

		{#if error}<div class="banner bad" role="alert">{error}{#if /plan/i.test(error)} <a href="/plans">See plans</a>{/if}</div>{/if}

		<div class="acts">
			<Button variant="primary" icon="check" size="lg" onclick={save} {busy}>{job ? 'Save changes' : 'Save script'}</Button>
			<Button icon="refresh" size="lg" onclick={runTest} busy={busy || testing}>{test ? 'Test again' : 'Test it'}</Button>
		</div>
	</aside>
</div>

{#if picking === 'file' && computer}
	<FilePicker {computer} filter={(n) => /\.(py|js|mjs|cjs|sh|bash)$/i.test(n)} onclose={() => (picking = null)} onpick={(p) => ((path = p), (picking = null), !name && (name = p.split('/').pop() ?? ''), (open = { ...open, 1: false, 2: true }))} />
{/if}
{#if picking === 'folder' && computer}
	<FolderPicker {computer} computerName={c?.name} title="Watch a folder" lede="The script runs when new files land here." action="Watch this folder" start={watchPath} onclose={() => (picking = null)} onpick={(p) => ((watchPath = p), (picking = null))} />
{/if}

<style>
	.add {
		width: 100%;
		box-sizing: border-box;
		display: grid;
		grid-template-columns: minmax(0, 1fr) 360px;
		gap: 20px;
		padding: 24px 28px;
		align-items: start;
	}
	header h1 {
		font-size: 22px;
		margin: 0 0 4px;
	}
	header p {
		color: var(--mid);
		font-size: 14px;
		margin: 0 0 16px;
	}
	.main {
		display: flex;
		flex-direction: column;
		gap: 10px;
		min-width: 0;
	}
	.sec {
		border: 1px solid var(--line);
		border-radius: 12px;
		background: var(--pane);
	}
	.sh {
		width: 100%;
		display: flex;
		align-items: center;
		gap: 12px;
		padding: 16px 18px;
		background: none;
		border: 0;
		color: var(--mid);
		cursor: pointer;
		text-align: left;
		font: inherit;
	}
	.chev {
		display: flex;
		transition: transform 0.15s;
	}
	.chev.up {
		transform: rotate(180deg);
	}
	.num {
		width: 24px;
		height: 24px;
		border-radius: 50%;
		flex-shrink: 0;
		display: flex;
		align-items: center;
		justify-content: center;
		font-size: 12px;
		font-weight: 600;
		background: var(--accent);
		color: var(--on-accent);
	}
	.num.ok {
		background: var(--live-soft);
		color: var(--accent-ink);
	}
	.st {
		flex: 1;
		display: flex;
		flex-direction: column;
	}
	.stt {
		font-size: 16px;
		font-weight: 600;
		color: var(--ink);
	}
	.sts {
		font-size: 13px;
		color: var(--mid);
	}
	.body {
		padding: 0 18px 18px 54px;
		display: flex;
		flex-direction: column;
		gap: 12px;
	}
	.body.two {
		display: grid;
		grid-template-columns: 1fr 1fr;
		align-items: start;
	}
	.filecard {
		display: flex;
		align-items: center;
		gap: 12px;
		padding: 12px 14px;
		border: 1px solid var(--line);
		border-radius: 10px;
		background: var(--bg);
		color: var(--mid);
	}
	.fc {
		flex: 1;
		display: flex;
		flex-direction: column;
		min-width: 0;
	}
	.fn {
		color: var(--ink);
		font-size: 14px;
	}
	.fm {
		font-size: 12.5px;
	}
	.empty {
		display: flex;
		gap: 8px;
		padding: 16px;
		border: 1px dashed var(--line-strong);
		border-radius: 10px;
		justify-content: center;
	}
	.nm,
	.fl {
		display: flex;
		flex-direction: column;
		gap: 6px;
		font-size: 14px;
	}
	.hint {
		font-size: 13px;
		color: var(--mid);
		margin: 0;
	}
	.samples {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: 6px;
	}
	.link {
		background: none;
		border: 0;
		padding: 0;
		color: var(--accent-ink);
		text-decoration: underline;
		cursor: pointer;
		font: inherit;
		align-self: flex-start;
	}
	.hide {
		display: none;
	}
	.table {
		border: 1px solid var(--line);
		border-radius: 10px;
		background: var(--bg);
	}
	.tr {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 12px;
		padding: 10px 12px;
		min-height: 54px;
		font-size: 14px;
	}
	.tr + .tr {
		border-top: 1px solid var(--line);
	}
	.tr select {
		width: 300px;
	}
	.pk {
		font-size: 13px;
		text-align: right;
	}
	.inl {
		display: flex;
		align-items: center;
		gap: 8px;
		color: var(--mid);
	}
	.inl input {
		width: 80px;
	}
	.tick {
		display: flex;
		align-items: center;
		gap: 10px;
		font-size: 14px;
		cursor: pointer;
		white-space: nowrap;
	}
	.saved {
		display: flex;
		flex-wrap: wrap;
		gap: 6px;
	}
	.pair {
		display: grid;
		grid-template-columns: 1fr 1fr;
		gap: 6px;
	}
	.addsec {
		display: flex;
		align-items: center;
		justify-content: space-between;
		height: 38px;
		padding: 0 12px;
		background: var(--bg);
		border: 1px solid var(--line-strong);
		border-radius: 8px;
		color: var(--mid);
		font: inherit;
		font-size: 14px;
		cursor: pointer;
	}
	.seg {
		display: flex;
		flex-wrap: wrap;
		gap: 8px;
	}
	.seg button {
		height: 34px;
		padding: 0 14px;
		border: 1px solid var(--line-strong);
		border-radius: 8px;
		background: none;
		color: var(--ink);
		font: inherit;
		font-size: 14px;
		cursor: pointer;
	}
	.seg button.on {
		background: var(--accent);
		border-color: var(--accent);
		color: var(--on-accent);
	}
	.line {
		display: flex;
		align-items: center;
		flex-wrap: wrap;
		gap: 10px;
		font-size: 14px;
	}
	.line select {
		width: 220px;
	}
	.cron {
		width: 200px;
	}
	.line input[type='time'] {
		width: 130px;
	}
	.pickbtn {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 10px;
		min-width: 220px;
		height: 36px;
		padding: 0 10px;
		background: var(--bg);
		border: 1px solid var(--line-strong);
		border-radius: 8px;
		color: var(--ink);
		font: inherit;
		font-size: 14px;
		cursor: pointer;
	}
	.side {
		display: flex;
		flex-direction: column;
		gap: 14px;
		position: sticky;
		top: 0;
	}
	.box {
		border: 1px solid var(--line);
		border-radius: 12px;
		background: var(--pane);
		padding: 18px;
		display: flex;
		flex-direction: column;
		gap: 10px;
	}
	.eyebrow {
		font: 11px 'JetBrains Mono', ui-monospace, monospace;
		letter-spacing: 0.08em;
		text-transform: uppercase;
		color: var(--mid);
	}
	.pw {
		font-size: 16px;
		line-height: 1.55;
		margin: 0;
		color: var(--ink);
	}
	.pw b.mono {
		font-size: 14px;
	}
	.th {
		display: flex;
		justify-content: space-between;
	}
	.when {
		font-size: 12px;
		color: var(--mid);
	}
	.tp {
		margin: 0;
		font-size: 14px;
	}
	.tp.bad {
		color: var(--failed);
	}
	.dot {
		display: inline-block;
		margin-right: 6px;
		width: 7px;
		height: 7px;
		border-radius: 50%;
		background: var(--working);
	}
	.tf {
		border: 1px solid var(--line);
		border-radius: 8px;
		background: var(--bg);
	}
	.tfr {
		display: flex;
		justify-content: space-between;
		gap: 10px;
		padding: 7px 12px;
		font-size: 12.5px;
		overflow: hidden;
		white-space: nowrap;
	}
	.tfr + .tfr {
		border-top: 1px solid var(--line);
	}
	.acts {
		display: grid;
		grid-template-columns: 1.4fr 1fr;
		gap: 8px;
	}
	@media (max-width: 1000px) {
		.add {
			grid-template-columns: 1fr;
		}
		.side {
			position: static;
		}
	}
</style>
