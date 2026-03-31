<script>
	import { game } from '$lib/game.svelte.js';
	import { player } from '$lib/player.svelte.js';
	import haySprite from '$lib/assets/hay-tool-sprite.webp';
	import truckEmptySprite from '$lib/assets/truck-empty-tool-sprite.webp';
	import trimmerSprite from '$lib/assets/trimmer-tool-sprite.webp';

	let { selected_job, locked = false, onselect } = $props();

	let base_disabled = $derived(game.status === 'RESET' || !player.id);

	let breeder_disabled = $derived(
		locked || base_disabled || !game.yak_counts || game.yak_counts.in_nursery < 1
	);
	let driver_disabled = $derived(
		locked || base_disabled || !game.yak_counts || game.yak_counts.in_warehouse < 1
	);
	let shearer_disabled = $derived(
		locked || base_disabled || !game.yak_counts || game.yak_counts.in_shearingshed < 1
	);

	let fees_array = $derived(
		[
			breeder_disabled ? null : game.job_fees?.breeder,
			driver_disabled ? null : game.job_fees?.driver,
			shearer_disabled ? null : game.job_fees?.shearer
		].filter((fee) => fee != null)
	);

	let max_fee = $derived(Math.max(...fees_array));
	let min_fee = $derived(Math.min(...fees_array));
	let all_equals = $derived(max_fee === min_fee);
	let only_one_min = $derived(
		!all_equals && fees_array.filter((fee) => fee === min_fee).length === 1
	);
</script>

<div class="dock static h-24 bg-base-200 px-0 pt-2">
	<button
		class="text-center"
		class:active={selected_job === 'BREEDER'}
		disabled={breeder_disabled}
		onclick={() => onselect?.('BREEDER')}
	>
		<img
			src={haySprite}
			alt="Hay"
			class="h-10 w-10 object-contain"
			class:opacity-10={breeder_disabled}
		/>
		<div>
			<span class="text-xs font-black"> Breed: </span>
			<span
				class="text-sm"
				class:text-success={game.job_fees?.breeder === max_fee && !breeder_disabled && !all_equals}
				class:text-error={game.job_fees?.breeder === min_fee && !breeder_disabled && only_one_min}
			>
				{#if game.job_fees}${game.job_fees?.breeder.toFixed(0)}{/if}
			</span>
		</div>
		<span class="text-xs opacity-70">
			{#if game.yak_counts}{game.yak_counts.in_nursery}
				{#if game.yak_counts.in_nursery > 1}yaks{:else}yak{/if} available{/if}
		</span>
	</button>

	<button
		class="text-center"
		class:active={selected_job === 'DRIVER'}
		disabled={driver_disabled}
		onclick={() => onselect?.('DRIVER')}
	>
		<img
			src={truckEmptySprite}
			alt="Truck"
			class="h-10 w-10 object-contain"
			class:opacity-10={driver_disabled}
		/>
		<div>
			<span class="text-xs font-black"> Drive: </span>
			<span
				class="text-sm"
				class:text-success={game.job_fees?.driver === max_fee && !driver_disabled && !all_equals}
				class:text-error={game.job_fees?.driver === min_fee && !driver_disabled && only_one_min}
			>
				{#if game.job_fees}${game.job_fees?.driver.toFixed(0)}{/if}
			</span>
		</div>
		<span class="text-xs opacity-70">
			{#if game.yak_counts}{game.yak_counts.in_warehouse}
				{#if game.yak_counts.in_warehouse > 1}yaks{:else}yak{/if} available{/if}
		</span>
	</button>

	<button
		class="text-center"
		class:active={selected_job === 'SHEARER'}
		disabled={shearer_disabled}
		onclick={() => onselect?.('SHEARER')}
	>
		<img
			src={trimmerSprite}
			alt="Trimmer"
			class="h-10 w-10 object-contain"
			class:opacity-10={shearer_disabled}
		/>
		<div>
			<span class="text-xs font-black"> Shear: </span>
			<span
				class="text-sm"
				class:text-success={game.job_fees?.shearer === max_fee && !shearer_disabled && !all_equals}
				class:text-error={game.job_fees?.shearer === min_fee && !shearer_disabled && only_one_min}
			>
				{#if game.job_fees}${game.job_fees?.shearer.toFixed(0)}{/if}
			</span>
		</div>
		<span class="text-xs opacity-70">
			{#if game.yak_counts}{game.yak_counts.in_shearingshed}
				{#if game.yak_counts.in_shearingshed > 1}yaks{:else}yak{/if} available{/if}
		</span>
	</button>
</div>
