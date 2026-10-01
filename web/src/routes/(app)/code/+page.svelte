<script lang="ts">
	import { page } from '$app/state';
	import { goto } from '$app/navigation';
	import { get, post, api, message } from '$lib/api';
	import { onLive, throttle } from '$lib/live';
	import { session } from '$lib/session.svelte';
	import { keepAwake } from '$lib/presence';
	import { ago } from '$lib/format';
	import Button from '$lib/ui/Button.svelte';
	import Status from '$lib/ui/Status.svelte';
	import Tabs from '$lib/ui/Tabs.svelte';
	import LiveOutput from '$lib/ui/LiveOutput.svelte';
	import CodeEditor from '$lib/ui/CodeEditor.svelte';
	import Review from '$lib/apps/Review.svelte';
	import { toast } from '$lib/ui/toast.svelte';

	// eslint-disable-next-line @typescript-eslint/no-explicit-any
	type Any = any;
	let projects = $state<Any[]>([]);
	let project = $state(page.url.searchParams.get('project') ?? '');
	let files = $state<Any[]>([]);
	let open = $state('');
	let text = $state('');
	let saved = $state('');
	let tab = $state(page.url.searchParams.get('review') ? 'agent' : 'agent');
	let tasks = $state<Any[]>([]);
	let prompt = $state('');
	let current = $state<Any>(null);
	let reviewing = $state(page.url.searchParams.get('review') ?? '');
	let server = $state<Any>(null);
	let command = $state('python3 -m http.server $PORT --bind 127.0.0.1');
	let port = $state(5173);
	let preview = $state<{ url: string; origin: string } | null>(null);
	let width = $state<'laptop' | 'phone'>('laptop');
	let newName = $state('');
	let loading = $state(true);
	let error = $state('');

	$effect(() => keepAwake(() => session.computerId, 'app', 'code'));

	async function loadProjects() {
		loading = true;
		try {
			projects = (await get(`/computers/${session.computerId}/code/projects`)).projects;
			if (!project && projects[0]) project = projects[0].path;
			error = '';
		} catch (e) {
			error = message(e);
		} finally {
			loading = false;
		}
	}
	$effect(() => {
		void session.computerId;
		loadProjects();
	});

	async function loadProject() {
		if (!project) return;
		files = (await get(`/computers/${session.computerId}/code/tree?project=${encodeURIComponent(project)}`)).files;
		tasks = (await get(`/computers/${session.computerId}/code/tasks?project=${encodeURIComponent(project)}`)).tasks;
		server = (await get(`/computers/${session.computerId}/code/devserver?project=${encodeURIComponent(project)}`)).server;
		if (server) {
			command = server.command;
			port = server.port;
		}
		if (reviewing) current = await get(`/runs/${reviewing}`);
		else if (!current && tasks[0]?.run_id) current = await get(`/runs/${tasks[0].run_id}`);
	}
	$effect(() => {
		void project;
		loadProject();
	});
	$effect(() =>
		onLive(
			throttle(async () => {
				await loadProject();
				if (current) current = await get(`/runs/${current.id}`);
			}, 500)
		)
	);

	async function openFile(path: string) {
		if (text !== saved && !confirm('Discard unsaved changes?')) return;
		const r = await fetch(`/api/computers/${session.computerId}/files/download?path=${encodeURIComponent(path)}`);
		text = saved = await r.text();
		open = path;
	}
	async function save() {
		try {
			await api(`/computers/${session.computerId}/files/write?path=${encodeURIComponent(open)}`, { method: 'PUT', raw: text });
			saved = text;
			toast('Saved');
		} catch (e) {
			toast(message(e), true);
		}
	}
	async function createProject() {
		try {
			const r = await post(`/computers/${session.computerId}/code/projects`, { name: newName });
			newName = '';
			await loadProjects();
			project = r.project;
		} catch (e) {
			toast(message(e), true);
		}
	}
	async function startTask() {
		try {
			const r = await post(`/computers/${session.computerId}/code/tasks`, { project, prompt });
			prompt = '';
			reviewing = '';
			current = await get(`/runs/${r.run_id}`);
			loadProject();
		} catch (e) {
			toast(message(e), true);
		}
	}
	async function answer(req: string, approved: boolean) {
		await post(`/runs/${current.id}/approvals/${req}`, { approved });
		current = await get(`/runs/${current.id}`);
	}
	async function startServer() {
		try {
			await post(`/computers/${session.computerId}/code/devserver`, { project, command, port });
			loadProject();
		} catch (e) {
			toast(message(e), true);
		}
	}
	async function stopServer() {
		await post(`/runs/${server.run_id}/stop`);
		preview = null;
		loadProject();
	}
	async function openPreview() {
		preview = await post(`/computers/${session.computerId}/previews`, { port });
	}
	async function newTab() {
		const p = await post(`/computers/${session.computerId}/previews`, { port });
		window.open(p.url, '_blank');
	}
	const running = $derived(server && ['queued', 'waiting', 'starting', 'running'].includes(server.status));
	const pendingApprovals = $derived((current?.approvals ?? []).filter((a: Any) => a.status === 'pending'));
</script>

<div class="code">
	<div class="bar row">
		<h1>Code</h1>
		<select bind:value={project} aria-label="Project" data-testid="project">
			{#each projects as p (p.path)}<option value={p.path}>{p.name}</option>{/each}
			{#if !projects.length}<option value="">No projects yet</option>{/if}
		</select>
		<input bind:value={newName} placeholder="new-project" class="mono small-input" aria-label="New project name" />
		<Button size="sm" onclick={createProject} disabled={!newName.trim()}>New project</Button>
		<span class="spacer"></span>
		<span class="low">Coding agent: Mock agent (prototype). Claude and OpenAI sign in from Settings later.</span>
	</div>
	{#if error}<div class="banner bad">{error}</div>{/if}
	{#if loading && !projects.length}<div class="pad low">Waking your computer…</div>{/if}
	{#if project}
		<div class="cols">
			<nav class="tree" aria-label="Project files">
				{#each files.filter((f) => !f.is_dir) as f (f.path)}
					<button class:on={open === f.path} onclick={() => openFile(f.path)} style="padding-left: {8 + (f.path.split('/').length - project.split('/').length - 1) * 12}px">{f.name}</button>
				{/each}
			</nav>
			<section class="editor">
				{#if open}
					<div class="row"><span class="mono">{open}</span>{#if text !== saved}<span class="chip">unsaved</span>{/if}<span class="spacer"></span><Button size="sm" onclick={save} disabled={text === saved}>Save</Button></div>
					<CodeEditor value={text} path={open} onchange={(v) => (text = v)} onsave={save} />
				{:else}
					<div class="muted-box">Choose a file to edit, or hand the agent a task.</div>
				{/if}
			</section>
			<aside class="side">
				<Tabs tabs={[{ id: 'agent', label: 'Agent' }, { id: 'preview', label: running ? 'Preview ●' : 'Preview' }]} bind:value={tab} />
				{#if tab === 'agent'}
					<div class="stack">
						<textarea bind:value={prompt} rows="3" placeholder={'Make the heading say "Fresh bread daily"'} aria-label="Task for the agent"></textarea>
						<Button variant="primary" onclick={startTask} disabled={!prompt.trim()}>Hand it to the agent</Button>
						<span class="low">It keeps working after you close this tab. It asks before running commands, and nothing lands until you keep it.</span>
						{#if current}
							<div class="card stack">
								<div class="row"><Status status={pendingApprovals.length ? 'needs_you' : current.status} /><span class="mid">{current.job.name}</span></div>
								{#each pendingApprovals as a (a.request_id)}
									<div class="pane stack" style="gap: 6px" data-testid="approval">
										<span>Wants to run <code>{a.command}</code></span><span class="low">{a.reason}</span>
										<div class="row"><Button size="sm" variant="primary" onclick={() => answer(a.request_id, true)}>Approve</Button><Button size="sm" onclick={() => answer(a.request_id, false)}>Deny</Button></div>
									</div>
								{/each}
								<LiveOutput runId={current.id} />
								{#if current.status === 'succeeded' && reviewing !== current.id}<Button variant="primary" onclick={() => (reviewing = current.id)}>Review changes</Button>{/if}
								{#if ['queued', 'waiting', 'starting', 'running'].includes(current.status)}<Button size="sm" variant="danger" onclick={() => post(`/runs/${current.id}/stop`)}>Stop the agent</Button>{/if}
							</div>
						{/if}
						<h3>Earlier tasks</h3>
						<div class="list">
							{#each tasks as t (t.job_id)}
								<button class="task row" onclick={async () => ((current = await get(`/runs/${t.run_id}`)), (reviewing = ''))}><Status status={t.status ?? 'asleep'} /><span>{t.name}</span><span class="spacer"></span><span class="low mono">{ago(t.at)}</span></button>
							{/each}
						</div>
					</div>
				{:else}
					<div class="stack">
						<label class="stack" style="gap: 4px"><span>Command</span><input bind:value={command} class="mono" /></label>
						<label class="stack" style="gap: 4px"><span>Port</span><input type="number" bind:value={port} class="mono" /></label>
						<div class="row">
							{#if running}<Status status={server.listening ? 'awake' : 'waking'} word={server.listening ? 'Running' : 'Starting'} /><Button size="sm" onclick={stopServer}>Stop</Button>{:else}<Button variant="primary" onclick={startServer}>Start dev server</Button>{/if}
						</div>
						{#if running && server.listening}
							<div class="row"><Button variant="primary" onclick={openPreview}>Open preview</Button><Button onclick={newTab}>Open in a new tab</Button></div>
							<span class="low">Private to you: it opens on its own address and travels over your computer's own connection.</span>
						{/if}
					</div>
				{/if}
			</aside>
		</div>
		{#if reviewing}
			<section class="pad"><Review runId={reviewing} onchanged={loadProject} /></section>
		{/if}
		{#if preview}
			<section class="pad stack">
				<div class="row"><h2>Preview</h2><span class="mono low">{preview.origin}</span><span class="spacer"></span>
					<Button size="sm" variant={width === 'laptop' ? 'primary' : 'ghost'} onclick={() => (width = 'laptop')}>Laptop</Button>
					<Button size="sm" variant={width === 'phone' ? 'primary' : 'ghost'} onclick={() => (width = 'phone')}>Phone</Button>
					<Button size="sm" onclick={() => (preview = null)}>Close</Button>
				</div>
				<iframe title="Preview" src={preview.url} class={width} data-testid="preview-frame"></iframe>
			</section>
		{/if}
	{:else if !loading}
		<div class="pad"><div class="muted-box">Create a project to start: it comes with a small website you can edit, run and preview.</div></div>
	{/if}
</div>

<style>
	.code {
		display: grid;
		gap: 12px;
		padding: 16px 20px 40px;
	}
	.bar h1 {
		margin-right: 8px;
	}
	.small-input {
		width: 160px;
	}
	.cols {
		display: grid;
		grid-template-columns: 200px minmax(0, 1fr) 380px;
		gap: 12px;
		min-height: 480px;
	}
	.tree {
		display: grid;
		align-content: start;
		background: var(--pane);
		border: 1px solid var(--line);
		border-radius: 10px;
		padding: 6px;
	}
	.tree button {
		text-align: left;
		background: none;
		border: 0;
		color: var(--mid);
		font: inherit;
		font-family: 'JetBrains Mono', monospace;
		font-size: 12.5px;
		padding: 4px 8px;
		border-radius: 6px;
		cursor: pointer;
	}
	.tree button.on {
		background: var(--surface);
		color: var(--ink);
	}
	.editor {
		display: grid;
		grid-template-rows: auto 1fr;
		gap: 8px;
		min-width: 0;
	}
	.side {
		display: grid;
		gap: 10px;
		align-content: start;
	}
	.task {
		background: none;
		border: 0;
		color: var(--ink);
		font: inherit;
		text-align: left;
		cursor: pointer;
	}
	iframe {
		border: 1px solid var(--line);
		border-radius: 12px;
		height: 560px;
		background: var(--surface);
		justify-self: center;
	}
	iframe.laptop {
		width: 100%;
	}
	iframe.phone {
		width: 390px;
	}
	@media (max-width: 1100px) {
		.cols {
			grid-template-columns: 1fr;
		}
	}
</style>
