<script lang="ts">
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import { get, post, message } from '$lib/api';
	import { refreshMe, session, setComputer } from '$lib/session.svelte';
	import { money } from '$lib/format';
	import Button from '$lib/ui/Button.svelte';
	import Field from '$lib/ui/Field.svelte';

	// eslint-disable-next-line @typescript-eslint/no-explicit-any
	let catalog = $state<any>(null);
	let name = $state(session.me?.computers.length ? '' : 'My computer');
	let size = $state('small');
	let apps = $state({ scripts: true, watcher: true, code: true });
	let error = $state('');
	let busy = $state(false);
	let plan = $derived(catalog?.plans.find((p: { id: string }) => p.id === (session.me?.account.trial_plan ?? session.me?.account.plan))?.plan);

	onMount(async () => (catalog = await get('/plans')));
	const order = ['small', 'medium', 'large'];
	const allowed = (s: string) => !plan || order.indexOf(s) <= order.indexOf(plan.largest_size);
	const hourly = (s: string) => catalog.sizes[s].provider_hourly * (1 + catalog.markup) * 1e6;

	async function create() {
		busy = true;
		error = '';
		try {
			const c = await post('/computers', { name, size, apps: Object.entries(apps).filter(([, v]) => v).map(([k]) => k) });
			setComputer(c.id);
			await refreshMe();
			goto('/home');
		} catch (e) {
			error = message(e);
		} finally {
			busy = false;
		}
	}
</script>

<div class="page">
	<h1>New computer</h1>
	<p class="mid">It sleeps when nothing is happening and wakes for work, so you pay for the hours it's awake and the files it keeps.</p>
	{#if catalog}
		<Field label="Name"><input bind:value={name} aria-label="Computer name" /></Field>
		<div class="sizes">
			{#each order as s (s)}
				{@const z = catalog.sizes[s]}
				<button class="size card" class:on={size === s} disabled={!allowed(s)} onclick={() => (size = s)} data-size={s}>
					<strong>{z.label}</strong>
					<span class="mid">{z.suits}</span>
					<span class="mono">{money(hourly(s))}/hour awake</span>
					<span class="low">{z.cpu} CPU · {z.memory_gb} GB memory · {z.disk_gb} GB storage</span>
					{#if !allowed(s)}<span class="chip">Needs a bigger plan</span>{/if}
				</button>
			{/each}
		</div>
		<div class="pane">
			<strong>What it costs.</strong>
			<span class="mid">Awake: <span class="mono">{money(hourly(size))}</span> an hour. Asleep: nothing but storage, <span class="mono">${(catalog.disk_gb_month * (1 + catalog.markup)).toFixed(4)}</span> per GB stored a month. A check that runs every hour costs pennies a month.</span>
		</div>
		<Field label="Starting apps" help="Files is always there.">
			<div class="row">
				<label class="row"><input type="checkbox" bind:checked={apps.scripts} /> Scripts</label>
				<label class="row"><input type="checkbox" bind:checked={apps.watcher} /> Watcher</label>
				<label class="row"><input type="checkbox" bind:checked={apps.code} /> Code</label>
			</div>
		</Field>
		{#if error}<div class="banner bad">{error} {#if error.includes('plan')}<a href="/plans">See plans</a>{/if}</div>{/if}
		<div class="row"><Button variant="primary" onclick={create} {busy}>Create computer</Button></div>
	{/if}
</div>

<style>
	.sizes {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(220px, 1fr));
		gap: 12px;
	}
	.size {
		display: grid;
		gap: 6px;
		text-align: left;
		color: var(--ink);
		font: inherit;
		cursor: pointer;
	}
	.size.on {
		border-color: var(--accent);
		box-shadow: 0 0 0 1px var(--accent);
	}
	.size:disabled {
		opacity: 0.55;
		cursor: default;
	}
</style>
