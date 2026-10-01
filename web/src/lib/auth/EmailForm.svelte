<script lang="ts">
	import { onDestroy, onMount } from 'svelte';
	import AuthSplit from './AuthSplit.svelte';
	import Icon from '$lib/ui/Icon.svelte';
	import { get, post, message } from '$lib/api';

	// Signing in and creating an account are the same step: we email a one-time link.
	let { mode }: { mode: 'signin' | 'join' } = $props();

	let email = $state('');
	let sent = $state(false);
	let busy = $state(false);
	let error = $state('');
	let wait = $state(0);
	let devTools = $state(false);
	let timer: ReturnType<typeof setInterval> | undefined;

	onMount(() => {
		get('/config')
			.then((c) => (devTools = c.dev_tools))
			.catch(() => {});
	});
	onDestroy(() => clearInterval(timer));

	async function send(e?: SubmitEvent) {
		e?.preventDefault();
		busy = true;
		error = '';
		try {
			await post('/auth/email', { email });
			sent = true;
			wait = 60;
			clearInterval(timer);
			timer = setInterval(() => {
				wait = Math.max(0, wait - 1);
				if (!wait) clearInterval(timer);
			}, 1000);
		} catch (err) {
			error = message(err);
		} finally {
			busy = false;
		}
	}
	const clock = (s: number) => `${Math.floor(s / 60)}:${String(s % 60).padStart(2, '0')}`;
</script>

<AuthSplit footer={!sent}>
	{#if sent}
		<span class="icon-tile"><Icon name="mail" size={24} /></span>
		<div class="head">
			<h1>Check your email</h1>
			<p class="mid">We sent a sign-in link to <span class="mono strong">{email}</span>. It works once and expires in 15 minutes.</p>
		</div>
		<div class="hint">
			<span class="hint-t">Can’t find it?</span>
			<p>Check your spam folder, or wait a minute and send another. Open the link on any device; this page signs you in when you do.</p>
			{#if devTools}<p>This prototype doesn’t send email: <a href="/dev" data-testid="outbox-link">open the outbox</a> to find the link.</p>{/if}
		</div>
		<div class="again">
			{#if wait > 0}<span>Send another in <span class="mono strong">{clock(wait)}</span></span>
			{:else}<button class="linkish" onclick={() => send()} disabled={busy}>Send another</button>{/if}
			<button class="linkish strong-link" onclick={() => (sent = false)}>Use a different email</button>
		</div>
	{:else}
		<div class="head">
			<h1>{mode === 'signin' ? 'Sign in to Croncave' : 'Create your account'}</h1>
			<p class="mid">{mode === 'signin' ? 'Welcome back. Pick up where your computers left off.' : 'Start free or pick a plan. Every plan comes with free usage.'}</p>
		</div>
		<form class="fields" onsubmit={send}>
			<label class="field">
				<span class="name">Email</span>
				<input type="email" required bind:value={email} placeholder="you@example.com" autocomplete="email" />
			</label>
			{#if error}<div class="error">{error}</div>{/if}
			<button class="primary" type="submit" disabled={busy}>{mode === 'signin' ? 'Send sign-in link' : 'Create account'}</button>
			<p class="note">We’ll email you a sign-in link. No password to remember.</p>
		</form>
		<p class="switch">
			{#if mode === 'signin'}New to Croncave? <a href="/join">Create an account</a>{:else}Already have an account? <a href="/signin">Sign in</a>{/if}
		</p>
	{/if}
</AuthSplit>

<style>
	.head {
		display: flex;
		flex-direction: column;
		gap: 6px;
	}
	h1 {
		font-size: 26px;
		line-height: 34px;
		font-weight: 600;
		letter-spacing: -0.02em;
	}
	.mid {
		font-size: 14px;
		line-height: 20px;
	}
	.strong {
		color: var(--ink);
	}
	.fields {
		display: flex;
		flex-direction: column;
		gap: 12px;
	}
	.field {
		display: flex;
		flex-direction: column;
		gap: 6px;
	}
	.name {
		font-size: 13px;
		line-height: 18px;
		font-weight: 500;
	}
	input {
		height: 42px;
		background: var(--surface);
	}
	.primary {
		display: flex;
		align-items: center;
		justify-content: center;
		height: 42px;
		border-radius: 6px;
		border: 0;
		background: var(--accent);
		color: var(--on-accent);
		font: 600 14px Inter, system-ui, sans-serif;
		cursor: pointer;
	}
	.primary:disabled {
		opacity: 0.6;
	}
	.note {
		font-size: 13px;
		line-height: 19px;
		color: var(--mid);
	}
	.switch {
		font-size: 14px;
		line-height: 20px;
		color: var(--mid);
	}
	.switch a,
	.strong-link {
		color: var(--ink);
		font-weight: 500;
		text-decoration: underline;
		text-underline-offset: 3px;
		text-decoration-color: var(--line-strong);
	}
	.icon-tile {
		width: 48px;
		height: 48px;
		border-radius: 12px;
		background: var(--accent-soft);
		color: var(--accent-ink);
		display: flex;
		align-items: center;
		justify-content: center;
	}
	.hint {
		display: flex;
		flex-direction: column;
		gap: 10px;
		padding: 14px 16px;
		border-radius: 8px;
		border: 1px solid var(--line);
		background: var(--surface);
		font-size: 13px;
		line-height: 19px;
		color: var(--mid);
	}
	.hint-t {
		font-weight: 500;
		color: var(--ink);
	}
	.again {
		display: flex;
		justify-content: space-between;
		align-items: center;
		font-size: 13px;
		color: var(--mid);
	}
	.linkish {
		background: none;
		border: 0;
		padding: 0;
		font: inherit;
		color: var(--accent-ink);
		cursor: pointer;
	}
</style>
