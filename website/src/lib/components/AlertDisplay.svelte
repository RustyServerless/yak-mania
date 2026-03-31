<script>
	import { alerts } from '$lib/alerts.svelte.js';
</script>

{#if alerts.size > 0}
	<div class="fixed top-0 left-[5%] z-50 w-[90%] md:left-[20%] md:w-[60%] lg:left-1/4 lg:w-1/2">
		{#each alerts as [id, alert] (id)}
			<div role="alert" class="mb-1 alert gap-1 rounded-xl p-1 md:gap-4 {alert.alert_class}">
				<div class="flex text-sm">
					<svg
						xmlns="http://www.w3.org/2000/svg"
						class="h-5 w-5 shrink-0 fill-current"
						viewBox="0 0 24 24"
					>
						<path
							d="M13,13H11V7H13M13,17H11V15H13M12,2A10,10 0 0,0 2,12A10,10 0 0,0 12,22A10,10 0 0,0 22,12A10,10 0 0,0 12,2Z"
						/>
					</svg>
					<b>{alert.title}</b>
				</div>
				<div class="mx-auto text-xs">{alert.message}</div>
				<button
					class="btn btn-circle text-current btn-outline btn-xs"
					title="Close alert"
					aria-label="Close alert"
					onclick={() => alert.close()}
				>
					<svg xmlns="http://www.w3.org/2000/svg" class="h-5 w-5" viewBox="0 0 24 24">
						<path
							d="M19,6.41L17.59,5L12,10.59L6.41,5L5,6.41L10.59,12L5,17.59L6.41,19L12,13.41L17.59,19L19,17.59L13.41,12L19,6.41Z"
						/>
					</svg>
				</button>
			</div>
		{/each}
	</div>
{/if}
