<script lang="ts">
	import { post, message } from '$lib/api';
	import { refreshMe, session } from '$lib/session.svelte';
	import Icon from '$lib/ui/Icon.svelte';
	import { toast } from '$lib/ui/toast.svelte';

	// Shown to everyone, AI on or off (with it off, the panel explains and offers to turn it
	// on). Anyone can put it away; Settings › Assistant brings it back.
	async function hide() {
		try {
			await post('/me/prefs', { ask_hidden: true });
			await refreshMe();
			session.assistantOpen = false;
			toast('Ask is hidden. Bring it back in Settings › Assistant.');
		} catch (e) {
			toast(message(e), true);
		}
	}
</script>

{#if !session.me?.user.prefs.ask_hidden}
	<div class="ask" class:beside={session.activityOpen}>
		<button class="open" onclick={() => (session.assistantOpen = !session.assistantOpen)} aria-label="Ask the assistant" aria-expanded={session.assistantOpen}>
			<Icon name="sparkle" size={20} />Ask
		</button>
		<button class="hide" onclick={hide} aria-label="Hide the Ask button" title="Hide"><Icon name="x" size={12} stroke={2} /></button>
	</div>
{/if}

<style>
	.ask {
		position: fixed;
		right: 32px;
		bottom: 32px;
		z-index: 25;
	}
	.beside {
		right: 444px;
	}
	.open {
		display: flex;
		align-items: center;
		gap: 8px;
		height: 48px;
		padding: 0 18px 0 14px;
		border-radius: 9999px;
		border: none;
		background: var(--accent);
		color: var(--on-accent);
		font: 600 14px Inter, system-ui, sans-serif;
		cursor: pointer;
		box-shadow: 0 8px 24px var(--shadow);
	}
	.hide {
		position: absolute;
		top: -6px;
		right: -6px;
		width: 22px;
		height: 22px;
		border-radius: 9999px;
		border: 1px solid var(--line);
		background: var(--surface);
		color: var(--mid);
		display: flex;
		align-items: center;
		justify-content: center;
		cursor: pointer;
		opacity: 0;
		transition: opacity 0.12s;
	}
	.ask:hover .hide,
	.ask:focus-within .hide {
		opacity: 1;
	}
	.hide:hover {
		color: var(--ink);
	}
	@media (hover: none) {
		.hide {
			opacity: 1;
		}
	}
</style>
