<script>
	import { signed_user, check_admin_and_redirect } from '$lib/auth.svelte.js';

	let { children } = $props();

	$effect(check_admin_and_redirect);
</script>

{#if signed_user.loading}
	<div class="flex h-dvh items-center justify-center">
		<span class="loading loading-lg loading-spinner"></span>
	</div>
{:else if signed_user.id && signed_user.is_admin}
	{@render children()}
{/if}
