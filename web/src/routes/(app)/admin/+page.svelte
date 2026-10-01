<script lang="ts">
	import { get, post, message } from '$lib/api';
	import { ago, duration, money, when } from '$lib/format';
	import Tabs from '$lib/ui/Tabs.svelte';
	import Button from '$lib/ui/Button.svelte';
	import Modal from '$lib/ui/Modal.svelte';
	import Field from '$lib/ui/Field.svelte';
	import { toast } from '$lib/ui/toast.svelte';

	// eslint-disable-next-line @typescript-eslint/no-explicit-any
	type Any = any;
	let tab = $state('catalog');
	let catalog = $state<Any>(null);
	let draft = $state<Any>(null);
	let raw = $state('');
	let rawMode = $state(false);
	let note = $state('');
	let accounts = $state<Any[]>([]);
	let m = $state<Any>(null);
	let crediting = $state<Any>(null);
	let creditDollars = $state('5');
	let creditReason = $state('');
	let error = $state('');

	async function load() {
		try {
			if (tab === 'catalog') {
				catalog = await get('/admin/catalog');
				draft = JSON.parse(JSON.stringify(catalog.data));
				raw = JSON.stringify(catalog.data, null, 2);
			} else if (tab === 'accounts') accounts = (await get('/admin/accounts')).accounts;
			else m = await get('/admin/measurements');
			error = '';
		} catch (e) {
			error = message(e);
		}
	}
	$effect(() => {
		void tab;
		load();
	});

	async function saveCatalog() {
		try {
			const data = rawMode ? JSON.parse(raw) : draft;
			const r = await post('/admin/catalog', { data, note });
			toast(`Saved catalog version ${r.id}. New sign-ups use it; existing accounts keep theirs.`);
			note = '';
			load();
		} catch (e) {
			toast(message(e), true);
		}
	}
	async function giveCredit() {
		try {
			await post(`/admin/accounts/${crediting.id}/credit`, { dollars: Number(creditDollars), reason: creditReason });
			toast('Credit given');
			crediting = null;
			load();
		} catch (e) {
			toast(message(e), true);
		}
	}
	async function kill(model: string, killed: boolean) {
		await post('/admin/ai/kill', { model, killed });
		load();
	}
	const planFields: [string, string][] = [
		['price_monthly', 'Price/month $'],
		['award', 'Award $'],
		['allowance', 'Allowance $'],
		['computers', 'Computers'],
		['awake_at_once', 'Awake at once'],
		['min_schedule_secs', 'Fastest schedule (s)'],
		['keep_awake', 'Keep-awake'],
		['history_days', 'History days']
	];
</script>

<div class="page wide">
	<h1>Admin</h1>
	<Tabs tabs={[{ id: 'catalog', label: 'Plan catalog' }, { id: 'accounts', label: 'Accounts' }, { id: 'measurements', label: 'Measurements' }]} bind:value={tab} />
	{#if error}<div class="banner bad">{error}</div>{/if}

	{#if tab === 'catalog' && draft}
		<p class="mid">Every price, limit, award, trial and promo lives here. Saving makes a new version; accounts stay on the version they signed up on.</p>
		<div class="row"><label class="row"><input type="checkbox" bind:checked={rawMode} /> Edit as JSON</label></div>
		{#if rawMode}
			<textarea class="mono json" bind:value={raw} rows="30" aria-label="Catalog JSON"></textarea>
		{:else}
			<section class="card">
				<table>
					<thead><tr><th>Plan</th>{#each planFields as [, label] (label)}<th>{label}</th>{/each}<th>Trial</th><th>Overage</th></tr></thead>
					<tbody>
						{#each Object.entries(draft.plans) as [id, p] (id)}
							{@const plan = p as Any}
							<tr>
								<td><strong>{plan.name}</strong></td>
								{#each planFields as [k] (k)}<td><input class="mono cell" type="number" step="any" bind:value={plan[k]} aria-label="{plan.name} {k}" data-field="{id}.{k}" /></td>{/each}
								<td class="mono small">{plan.trial ? `${plan.trial.days}d ${plan.trial.plan}` : '—'}</td>
								<td><input type="checkbox" bind:checked={plan.overage} /></td>
							</tr>
						{/each}
					</tbody>
				</table>
			</section>
			<section class="card stack">
				<h2>Rates</h2>
				<div class="row">
					<label class="row">Markup <input class="mono cell" type="number" step="0.01" bind:value={draft.markup} /></label>
					<label class="row">Storage $/GB-month <input class="mono cell" type="number" step="0.01" bind:value={draft.disk_gb_month} /></label>
					{#each Object.entries(draft.sizes) as [id, s] (id)}
						{@const size = s as Any}<label class="row">{size.label} $/hour <input class="mono cell" type="number" step="0.0001" bind:value={size.provider_hourly} /></label>
					{/each}
				</div>
			</section>
		{/if}
		<div class="row" style="flex-wrap: nowrap"><input bind:value={note} placeholder="What changed, e.g. Max allowance to $25" aria-label="Change note" /><Button variant="primary" onclick={saveCatalog} disabled={!note.trim()}>Save new version</Button></div>
		<section class="card stack">
			<h2>Versions</h2>
			<div class="list">{#each catalog.versions as v (v.id)}<div class="row"><span class="mono">v{v.id}</span><span>{v.note}</span><span class="spacer"></span><span class="low">{v.accounts} accounts · {when(v.created_at)}</span></div>{/each}</div>
		</section>
	{/if}

	{#if tab === 'accounts'}
		<section class="card">
			<table>
				<thead><tr><th>Account</th><th>Plan</th><th>Catalog</th><th>Computers</th><th>Left</th><th>Used this month</th><th></th></tr></thead>
				<tbody>
					{#each accounts as a (a.id)}
						<tr>
							<td>{a.email}<div class="low small">{ago(a.created_at)}</div></td>
							<td>{a.plan ?? '—'}{#if a.trial_plan} <span class="chip">trial {a.trial_plan}</span>{/if}{#if a.paused} <span class="chip">paused</span>{/if}</td>
							<td class="mono">v{a.catalog_version}</td><td class="mono">{a.computers}</td>
							<td class="mono">{money(a.left_micros)}</td><td class="mono">{money(a.used_this_period_micros)}</td>
							<td><Button size="sm" onclick={() => ((crediting = a), (creditReason = ''))}>Give credit</Button></td>
						</tr>
					{/each}
				</tbody>
			</table>
		</section>
	{/if}

	{#if tab === 'measurements' && m}
		<p class="mid">What the alpha measures, beside what the pricing model assumed. Counts, durations and costs only.</p>
		<div class="grid2">
			<section class="card stack">
				<h2>Wake time</h2>
				<p class="mid">{m.wake.under_target} of {m.wake.total} wakes under 5 seconds (target: {m.wake.target}).</p>
				<table><thead><tr><th>Size</th><th>Wakes</th><th>p50</th><th>p95</th><th>Slowest</th></tr></thead>
					<tbody>{#each m.wake.by_size as w (w.size)}<tr><td>{w.size}</td><td class="mono">{w.count}</td><td class="mono">{Math.round(w.p50_ms)} ms</td><td class="mono">{Math.round(w.p95_ms)} ms</td><td class="mono">{w.max_ms} ms</td></tr>{/each}</tbody></table>
			</section>
			<section class="card stack">
				<h2>Why computers were awake</h2>
				{#each m.awake_by_cause as c (c.cause)}
					<div class="stack" style="gap: 2px"><div class="row"><span>{c.cause.replaceAll('_', ' ')}</span><span class="spacer"></span><span class="mono low">{duration(c.seconds)}</span></div><div class="hbar"><span style="width: {Math.round(c.share * 100)}%"></span></div></div>
				{:else}<div class="low">No awake time yet.</div>{/each}
			</section>
			<section class="card stack">
				<h2>Plan funnel</h2>
				<div class="list small">{#each m.funnel as f (f.kind)}<div class="row"><span>{f.kind.replaceAll('_', ' ')}</span><span class="spacer"></span><span class="mono">{f.count}</span></div>{/each}</div>
				<h3>Plan mix</h3>
				<div class="list small">{#each m.plan_mix as p (p.plan)}<div class="row"><span>{p.plan}</span><span class="spacer"></span><span class="mono">{p.count}</span></div>{/each}</div>
			</section>
			<section class="card stack">
				<h2>Activation</h2>
				<div class="list small">
					<div class="row"><span>Jobs set up</span><span class="spacer"></span><span class="mono">{m.activation.jobs}</span></div>
					<div class="row"><span>Runs</span><span class="spacer"></span><span class="mono">{m.activation.runs}</span></div>
					<div class="row"><span>Runs per job</span><span class="spacer"></span><span class="mono">{m.activation.runs_per_job.toFixed(1)}</span></div>
					<div class="row"><span>Median time to first result</span><span class="spacer"></span><span class="mono">{m.activation.median_secs_to_first_result ? duration(m.activation.median_secs_to_first_result) : '—'}</span></div>
					<div class="row"><span>Scheduled run start delay p50 / p95</span><span class="spacer"></span><span class="mono">{m.schedule_start_delay.p50_secs?.toFixed(1) ?? '—'}s / {m.schedule_start_delay.p95_secs?.toFixed(1) ?? '—'}s</span></div>
				</div>
			</section>
			<section class="card stack">
				<h2>Usage by account</h2>
				<div class="list small">{#each m.usage_by_account as u, i (i)}<div class="row"><span>{u.email}</span><span class="chip">{u.bucket}</span><span class="spacer"></span><span class="mono">{money(u.micros)}</span></div>{/each}</div>
				<h3>Meters</h3>
				<div class="list small">{#each m.meters as x (x.meter)}<div class="row"><span>{x.meter}</span><span class="spacer"></span><span class="mono">{x.meter === 'compute' ? duration(x.quantity) : `${x.quantity.toFixed(4)} GB-h`} · {money(x.cost_micros)}</span></div>{/each}</div>
			</section>
			<section class="card stack">
				<h2>Assumptions to replace</h2>
				<table><thead><tr><th>What</th><th>Assumed</th><th>Measured by</th></tr></thead><tbody>{#each m.assumptions as a (a.what)}<tr><td>{a.what}</td><td class="mono">{a.assumed}</td><td class="low">{a.measure}</td></tr>{/each}</tbody></table>
				<h3>AI kill switch</h3>
				<label class="row"><input type="checkbox" checked={m.ai_killed.includes('mock-assistant')} onchange={(e) => kill('mock-assistant', e.currentTarget.checked)} /> Switch off mock-assistant</label>
			</section>
		</div>
	{/if}
</div>

{#if crediting}
	<Modal title="Give {crediting.email} a credit" onclose={() => (crediting = null)}>
		<Field label="Amount ($)"><input bind:value={creditDollars} inputmode="decimal" class="mono" /></Field>
		<Field label="Reason (they see it)"><input bind:value={creditReason} placeholder="Sorry about the outage on Tuesday" /></Field>
		<div class="row"><Button variant="primary" onclick={giveCredit} disabled={!creditReason.trim()}>Give credit</Button></div>
	</Modal>
{/if}

<style>
	.wide {
		max-width: 1300px;
	}
	.cell {
		width: 86px;
		padding: 4px 6px;
	}
	.json {
		min-height: 420px;
	}
	.small {
		font-size: 12.5px;
	}
	.hbar {
		height: 6px;
		background: var(--pane);
		border-radius: 3px;
	}
	.hbar span {
		display: block;
		height: 100%;
		background: var(--chart-2);
		border-radius: 3px;
	}
</style>
