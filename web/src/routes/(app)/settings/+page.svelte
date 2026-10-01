<script lang="ts">
	import { get, post, message } from '$lib/api';
	import { refreshMe, session } from '$lib/session.svelte';
	import { ago } from '$lib/format';
	import Button from '$lib/ui/Button.svelte';
	import Field from '$lib/ui/Field.svelte';
	import { toast } from '$lib/ui/toast.svelte';

	const me = $derived(session.me);
	let name = $state(session.me?.user.name ?? '');
	let mode = $state(session.me?.user.prefs.mode ?? 'dark');
	let channels = $state({ email: true, sms: false, push: false, ...(session.me?.user.prefs.channels ?? {}) });
	let quiet = $state({ enabled: false, start_hour: 22, end_hour: 7, ...(session.me?.user.prefs.quiet_hours ?? {}) });
	// eslint-disable-next-line @typescript-eslint/no-explicit-any
	let audit = $state<any[]>([]);
	$effect(() => {
		get('/me/audit').then((r) => (audit = r.entries));
	});

	async function save(body: Record<string, unknown>, done = 'Saved') {
		try {
			await post('/me/prefs', body);
			toast(done);
			refreshMe();
		} catch (e) {
			toast(message(e), true);
		}
	}
	function setMode(m: string) {
		mode = m;
		document.documentElement.dataset.mode = m;
		document.cookie = `cc_mode=${m}; path=/; max-age=31536000; samesite=lax`;
		save({ mode: m }, 'Appearance saved');
	}
	const saveQuiet = () => save({ quiet_hours: { ...quiet, utc_offset_minutes: -new Date().getTimezoneOffset() } });
</script>

<div class="page">
	<h1>Settings</h1>
	<section class="card stack">
		<h2>Profile</h2>
		<Field label="Name"><div class="row" style="flex-wrap: nowrap"><input bind:value={name} aria-label="Name" /><Button onclick={() => save({ name })}>Save</Button></div></Field>
		<div class="low">{me?.user.email} · {me?.user.phone}</div>
	</section>
	<section class="card stack">
		<h2>Appearance</h2>
		<div class="row" role="radiogroup" aria-label="Mode">
			{#each [['dark', 'Dark'], ['light', 'Light'], ['system', 'Match my system']] as [id, label] (id)}
				<label class="row"><input type="radio" name="mode" checked={mode === id} onchange={() => setMode(id)} /> {label}</label>
			{/each}
		</div>
		<Field label="Color scheme" help="More schemes are on the way; every screen already takes them."><select disabled><option>Croncave</option></select></Field>
	</section>
	<section class="card stack">
		<h2>Notifications</h2>
		<p class="mid">Activity always shows everything. Choose where else you're told when something finishes, finds something or fails.</p>
		<label class="row"><input type="checkbox" bind:checked={channels.email} /> Email ({me?.user.email})</label>
		<label class="row"><input type="checkbox" bind:checked={channels.sms} /> Text message ({me?.user.phone})</label>
		<label class="row"><input type="checkbox" bind:checked={channels.push} /> Push notifications in this browser</label>
		<div><Button onclick={() => save({ channels })}>Save channels</Button></div>
		<label class="row"><input type="checkbox" bind:checked={quiet.enabled} /> Quiet hours from
			<select bind:value={quiet.start_hour} class="hour">{#each Array.from({ length: 24 }, (_, h) => h) as h (h)}<option value={h}>{h}:00</option>{/each}</select> to
			<select bind:value={quiet.end_hour} class="hour">{#each Array.from({ length: 24 }, (_, h) => h) as h (h)}<option value={h}>{h}:00</option>{/each}</select>
			<span class="low">(urgent ones still come through)</span></label>
		<div><Button onclick={saveQuiet}>Save quiet hours</Button></div>
	</section>
	<section class="card stack">
		<h2>Assistant</h2>
		<p class="mid">Off unless you turn it on. It fills in the same forms you would and nothing runs until you apply it. Its AI is billed at exactly what the provider charges and stops at your cap.</p>
		<label class="row"><input type="checkbox" checked={me?.account.ai_enabled} onchange={(e) => save({ ai_enabled: e.currentTarget.checked }, e.currentTarget.checked ? 'Assistant on' : 'Assistant off')} data-testid="ai-toggle" /> Turn on the assistant</label>
	</section>
	<section class="card stack">
		<h2>Connections</h2>
		<div class="list">
			<div class="row"><span>Phone</span><span class="spacer"></span><span class="mono">{me?.user.phone}</span></div>
			<div class="row"><span>GitHub</span><span class="spacer"></span><Button size="sm" disabled>Connect (coming soon)</Button></div>
			<div class="row"><span>Coding agent sign-in (Claude, OpenAI)</span><span class="spacer"></span><span class="low">Mock agent in the prototype</span></div>
		</div>
	</section>
	<section class="card stack">
		<h2>Who changed what</h2>
		<div class="list small">
			{#each audit as a, i (i)}<div class="row"><span class="chip">{a.actor}</span><span>{a.action.replace('.', ' ')} {a.target}</span><span class="spacer"></span><span class="low mono">{ago(a.at)}</span></div>{:else}<div class="low">Nothing yet.</div>{/each}
		</div>
	</section>
</div>

<style>
	.hour {
		width: auto;
	}
	.small {
		font-size: 13px;
	}
</style>
