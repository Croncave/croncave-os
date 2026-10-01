<script lang="ts">
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import TopBar from '$lib/shell/TopBar.svelte';
	import Dock from '$lib/shell/Dock.svelte';
	import ActivityPanel from '$lib/shell/ActivityPanel.svelte';
	import AssistantPanel from '$lib/shell/AssistantPanel.svelte';
	import AskButton from '$lib/shell/AskButton.svelte';
	import { refreshMe, session } from '$lib/session.svelte';
	import { connectLive, onChange, throttle } from '$lib/live';

	let { children } = $props();
	let ready = $state(false);

	onMount(() => {
		refreshMe()
			.then((me) => {
				if (me.signup_step !== 'done') return goto('/signup');
				const free = ['/computers/new', '/plans', '/settings', '/usage', '/admin'];
				if (!me.computers.length && !free.some((p) => page.url.pathname.startsWith(p))) return goto('/computers/new');
				ready = true;
				connectLive();
			})
			.catch(() => goto('/signin'));
		// The top bar (computer state, usage left, unread count) follows what happens.
		return onChange(
			throttle(() => {
				refreshMe().catch(() => {});
			}, 500)
		);
	});
</script>

{#if ready && session.me}
	<div class="shell">
		<TopBar />
		<div class="body">
			<Dock />
			<main class="main">
				{#if session.me.account.paused_at && !page.url.pathname.startsWith('/plans')}
					<div class="banner paused" data-testid="paused-banner">
						<strong>Work is paused at your spending cap.</strong>
						<span class="mid">Computers finish what they're doing and sleep. Nothing is deleted.</span>
						<span class="spacer"></span><a href="/plans">Raise the cap</a>
					</div>
				{/if}
				{@render children()}
			</main>
		</div>
		{#if session.activityOpen}<ActivityPanel />{/if}
		{#if session.assistantOpen}<AssistantPanel />{/if}
		<AskButton />
	</div>
{/if}

<style>
	.shell {
		min-height: 100vh;
		background: var(--bg);
	}
	.body {
		display: flex;
		min-height: calc(100vh - 52px);
	}
	.main {
		flex: 1;
		min-width: 0;
		display: flex;
		flex-direction: column;
	}
	.paused {
		margin: 0 12px 12px 0;
	}
</style>
