<script>
	import { onMount, onDestroy } from 'svelte';
	import {
		game,
		get_appsync_admin_client,
		load_game_state,
		subscribe_admin_game_updates,
		unsubscribe_all
	} from '$lib/game.svelte.js';
	import babyYakSprite from '$lib/assets/baby-yak-sprite.webp';
	import hairyYakSprite from '$lib/assets/hairy-yak-sprite.webp';
	import nakedYakSprite from '$lib/assets/naked-yak-sprite.webp';
	import haySprite from '$lib/assets/hay-tool-sprite.webp';
	import truckSprite from '$lib/assets/truck-loaded-tool-sprite.webp';
	import trimmerSprite from '$lib/assets/trimmer-tool-sprite.webp';
	import yakIcon from '$lib/assets/yak-mania-icon.webp';

	onMount(() => {
		load_game_state(get_appsync_admin_client());
		subscribe_admin_game_updates();
	});

	onDestroy(() => {
		unsubscribe_all();
	});

	let sorted_players = $derived.by(() => {
		const sorted = [...game.players.values()].sort((a, b) => {
			if (b.balance !== a.balance) return b.balance - a.balance;
			return a.name.localeCompare(b.name);
		});

		let current_rank = 1;
		return sorted.map((p, i) => {
			if (i > 0 && sorted[i].balance < sorted[i - 1].balance) {
				current_rank = i + 1;
			}
			return { ...p, rank: current_rank };
		});
	});

	const rank_styles = [
		'text-2xl',
		'text-3xl font-medium',
		'text-4xl font-semibold',
		'text-5xl font-bold'
	];
	let rank_3_style_index = $derived(sorted_players[3]?.rank != sorted_players[2]?.rank ? 1 : 0);
	let rank_2_style_index = $derived(
		sorted_players[2]?.rank != sorted_players[1]?.rank ? rank_3_style_index + 1 : rank_3_style_index
	);
	let rank_1_style_index = $derived(
		sorted_players[1]?.rank != sorted_players[0]?.rank ? rank_2_style_index + 1 : rank_2_style_index
	);
</script>

<div class="flex h-dvh flex-col overflow-auto">
	<div class="flex items-center justify-center gap-3 bg-base-200 p-4">
		<img src={yakIcon} alt="Yak Mania" class="h-24 w-24 rounded-lg" />
		<div class="text-left text-5xl leading-tight font-bold">Yak mania</div>
	</div>

	<div class="flex flex-col gap-2 p-8">
		<div class="flex justify-around">
			<div class="flex flex-col items-center">
				<img src={babyYakSprite} alt="Baby Yak" class="h-28 w-28 object-contain" />
				<div class="text-4xl font-bold" class:text-error={!game.yak_counts?.in_nursery}>
					{game.yak_counts?.in_nursery ?? 0}
				</div>
				<div class="text-xl opacity-70">In Nursery</div>
			</div>
			<div class="flex flex-col items-center">
				<img src={hairyYakSprite} alt="Hairy Yak" class="h-28 w-28 object-contain" />
				<div class="text-4xl font-bold" class:text-error={!game.yak_counts?.in_warehouse}>
					{game.yak_counts?.in_warehouse ?? 0}
				</div>
				<div class="text-xl opacity-70">In Warehouse</div>
			</div>
			<div class="flex flex-col items-center">
				<img src={hairyYakSprite} alt="Hairy Yak" class="h-28 w-28 object-contain" />
				<div class="text-4xl font-bold" class:text-error={!game.yak_counts?.in_shearingshed}>
					{game.yak_counts?.in_shearingshed ?? 0}
				</div>
				<div class="text-xl opacity-70">In Shearing Shed</div>
			</div>
			<div class="flex flex-col items-center">
				<img src={nakedYakSprite} alt="Naked Yak" class="h-28 w-28 object-contain" />
				<div class="text-4xl font-bold">{game.yak_counts?.total_sheared ?? 0}</div>
				<div class="text-xl opacity-70">Sheared</div>
			</div>
		</div>

		<div class="flex justify-evenly">
			<div class="flex aspect-square flex-col items-center rounded-2xl border p-2">
				<img src={haySprite} alt="Hay" class="h-40 w-40 object-contain" />
				<div class="text-6xl font-bold">{game.yak_counts?.with_breeders ?? 0}</div>
				<div class="text-2xl opacity-70">being Bred</div>
			</div>
			<div class="flex aspect-square flex-col items-center rounded-2xl border p-2">
				<img src={truckSprite} alt="Truck" class="h-40 w-40 object-contain" />
				<div class="text-6xl font-bold">{game.yak_counts?.with_drivers ?? 0}</div>
				<div class="text-2xl opacity-70">being Driven</div>
			</div>
			<div class="flex aspect-square flex-col items-center rounded-2xl border p-2">
				<img src={trimmerSprite} alt="Trimmer" class="h-40 w-40 object-contain" />
				<div class="text-6xl font-bold">{game.yak_counts?.with_shearers ?? 0}</div>
				<div class="text-2xl opacity-70">being Sheared</div>
			</div>
		</div>
	</div>

	<div class="p-8">
		<h2 class="mb-2 text-center text-5xl font-bold">Leaderboard</h2>
		<table class="table w-full table-zebra">
			<thead class="text-2xl">
				<tr>
					<th>Rank</th>
					<th>Player</th>
					<th>Balance</th>
					<th>Bred</th>
					<th>Driven</th>
					<th>Sheared</th>
				</tr>
			</thead>
			<tbody>
				{#each sorted_players as p, i (p.id)}
					<tr
						class={i === 0
							? rank_styles[rank_1_style_index]
							: i === 1
								? rank_styles[rank_2_style_index]
								: i === 2
									? rank_styles[rank_3_style_index]
									: rank_styles[0]}
					>
						<td>#{p.rank}</td>
						<td>{p.name}</td>
						<td>💵 ${p.balance.toFixed(0)}</td>
						<td>{p.yak_bred}</td>
						<td>{p.yak_driven}</td>
						<td>{p.yak_sheared}</td>
					</tr>
				{/each}
			</tbody>
		</table>
	</div>
</div>
