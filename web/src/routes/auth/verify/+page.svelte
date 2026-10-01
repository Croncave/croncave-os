<script lang="ts">
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import Narrow from '../../Narrow.svelte';
	import { post, message } from '$lib/api';
	import { refreshMe } from '$lib/session.svelte';

	let error = $state('');
	onMount(async () => {
		try {
			await post('/auth/verify', { token: page.url.searchParams.get('token') ?? '' });
			const me = await refreshMe();
			goto(me.signup_step !== 'done' ? '/signup' : '/home', { replaceState: true });
		} catch (e) {
			error = message(e);
		}
	});
</script>

<Narrow>
	{#if error}
		<h1>That link didn't work</h1>
		<p class="error">{error}</p>
		<a href="/signin">Ask for a new link</a>
	{:else}
		<p class="mid">Signing you in…</p>
	{/if}
</Narrow>
