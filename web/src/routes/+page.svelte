<script lang="ts">
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import { refreshMe } from '$lib/session.svelte';

	onMount(async () => {
		try {
			const me = await refreshMe();
			goto(me.signup_step !== 'done' ? '/signup' : me.computers.length ? '/home' : '/computers/new', { replaceState: true });
		} catch {
			goto('/signin', { replaceState: true });
		}
	});
</script>
