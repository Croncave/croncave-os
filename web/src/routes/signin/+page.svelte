<script lang="ts">
	import Narrow from '../Narrow.svelte';
	import Button from '$lib/ui/Button.svelte';
	import { post, message } from '$lib/api';

	let email = $state('');
	let sent = $state(false);
	let busy = $state(false);
	let error = $state('');

	async function send(e: SubmitEvent) {
		e.preventDefault();
		busy = true;
		error = '';
		try {
			await post('/auth/email', { email });
			sent = true;
		} catch (err) {
			error = message(err);
		} finally {
			busy = false;
		}
	}
</script>

<Narrow>
	{#if sent}
		<h1>Check your email</h1>
		<p class="mid">We sent a sign-in link to <strong>{email}</strong>. It works once, for 30 minutes.</p>
		<p class="mid">In this prototype, email is simulated: <a href="/dev" data-testid="outbox-link">open the outbox</a> to find the link.</p>
		<Button onclick={() => (sent = false)}>Use another email</Button>
	{:else}
		<h1>Sign in or sign up</h1>
		<p class="mid">A private computer in the cloud that keeps working while you're away. No password: we'll email you a link.</p>
		<form class="stack" onsubmit={send}>
			<input type="email" required bind:value={email} placeholder="you@example.com" aria-label="Email" autocomplete="email" />
			{#if error}<div class="error">{error}</div>{/if}
			<Button variant="primary" type="submit" {busy}>Email me a sign-in link</Button>
		</form>
	{/if}
</Narrow>
