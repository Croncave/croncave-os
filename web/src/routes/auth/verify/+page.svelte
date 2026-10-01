<script lang="ts">
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import AuthSplit from '$lib/auth/AuthSplit.svelte';
	import { post, message } from '$lib/api';
	import { refreshMe } from '$lib/session.svelte';

	let error = $state('');
	onMount(async () => {
		try {
			await post('/auth/verify', { token: page.url.searchParams.get('token') ?? '' });
			const me = await refreshMe();
			goto(me.signup_step !== 'done' ? '/signup' : me.computers.length ? '/home' : '/welcome', { replaceState: true });
		} catch (e) {
			error = message(e);
		}
	});
</script>

<AuthSplit footer={false}>
	{#if error}
		<div class="head">
			<h1>That link didn't work</h1>
			<p class="error">{error}</p>
		</div>
		<p class="mid">Links work once and expire after 15 minutes. <a href="/signin">Ask for a new link</a></p>
	{:else}
		<p class="mid">Signing you in…</p>
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
	}
</style>
