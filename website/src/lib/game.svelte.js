import { generateClient } from 'aws-amplify/api';
import { alert_appsync_error } from '$lib/alerts.svelte.js';
import { SvelteMap } from 'svelte/reactivity';
import { player, clear_player } from '$lib/player.svelte';

// AppSync clients (lazy-initialized to avoid "Amplify has not been configured" warning during SSR)
let _appsync_client;
let _appsync_admin_client;

export function get_appsync_client() {
	if (!_appsync_client) _appsync_client = generateClient();
	return _appsync_client;
}

export function get_appsync_admin_client() {
	if (!_appsync_admin_client) _appsync_admin_client = generateClient({ authMode: 'userPool' });
	return _appsync_admin_client;
}

// Game state
export const game = $state({
	status: null,
	players: new SvelteMap(),
	yak_counts: null,
	job_fees: null
});

const changes_buffer = {
	players: new Map(),
	yak_counts: null,
	job_fees: null,
	flush_interval: null
};

// Subscriptions
const subscriptions = [];
let game_updated_sub = null;

// Shared query string for gameUpdated
const ADMIN_GAME_UPDATED_SUBSCRIPTION_QUERY = `
	subscription GameUpdated {
		gameUpdated {
		  sampled
			game_status
			player {
				id
				name
				assignment {
					job
					yak {
						id
					}
					fee
				}
				balance
				yak_bred
				yak_driven
				yak_sheared
			}
			yak_counts {
				in_nursery
				with_breeders
				in_warehouse
				with_drivers
				in_shearingshed
				with_shearers
				total_sheared
			}
			job_fees {
				breeder
				driver
				shearer
			}
		}
	}
`;
const PLAYER_GAME_UPDATED_SUBSCRIPTION_QUERY = `
	subscription GameUpdated {
		gameUpdated {
		  sampled
			game_status
			player {
				id
				name
				balance
				yak_bred
				yak_driven
				yak_sheared
			}
			yak_counts {
				in_nursery
				with_breeders
				in_warehouse
				with_drivers
				in_shearingshed
				with_shearers
				total_sheared
			}
			job_fees {
				breeder
				driver
				shearer
			}
		}
	}
`;

// Shared handler for gameUpdated
function handle_game_updated({ data }) {
	console.log('handle_game_updated', data);
	const update = data.gameUpdated;
	if (!update) return;

	if (update.player) {
		changes_buffer.players.set(update.player.id, update.player);
	}

	if (update.game_status) {
		game.status = update.game_status;
		if (update.game_status == 'RESET') {
			changes_buffer.players.clear();
			game.players.forEach((p) => {
				p.balance = 0;
				p.assignment = null;
				p.yak_bred = 0;
				p.yak_driven = 0;
				p.yak_sheared = 0;
			});
		}
	}

	if (update.yak_counts) {
		changes_buffer.yak_counts = update.yak_counts;
	}

	if (update.job_fees) {
		changes_buffer.job_fees = update.job_fees;
	}

	if (update.player || update.yak_counts || update.job_fees) {
		start_flush_interval();
	}
}

// Flush function
function commit_buffers() {
	const has_changes =
		changes_buffer.players.size > 0 ||
		changes_buffer.yak_counts !== null ||
		changes_buffer.job_fees !== null;

	if (!has_changes) {
		stop_flush_interval();
		return;
	}

	if (changes_buffer.players.size > 0) {
		changes_buffer.players.forEach((p) => {
			const player = $state(p);
			game.players.set(p.id, player);
		});
		changes_buffer.players.clear();
	}

	if (changes_buffer.yak_counts !== null) {
		game.yak_counts = changes_buffer.yak_counts;
		changes_buffer.yak_counts = null;
	}

	if (changes_buffer.job_fees !== null) {
		game.job_fees = changes_buffer.job_fees;
		changes_buffer.job_fees = null;
	}
}

// Interval management
function start_flush_interval() {
	if (changes_buffer.flush_interval === null) {
		commit_buffers();
		changes_buffer.flush_interval = setInterval(commit_buffers, 250);
	}
}

function stop_flush_interval() {
	if (changes_buffer.flush_interval !== null) {
		clearInterval(changes_buffer.flush_interval);
		changes_buffer.flush_interval = null;
	}
}

// Load game state
export async function load_game_state(client) {
	try {
		const data = (
			await client.graphql({
				query: `
				query GameState {
					gameState {
						game_status
						players {
							id
							name
							assignment {
								job
								yak {
									id
								}
								fee
							}
							balance
							yak_bred
							yak_driven
							yak_sheared
						}
						yak_counts {
					    in_nursery
							with_breeders
							in_warehouse
							with_drivers
							in_shearingshed
							with_shearers
							total_sheared
						}
						job_fees {
							breeder
							driver
							shearer
						}
					}
				}
			`
			})
		).data;
		game.status = data.gameState.game_status;

		const player_map = new SvelteMap();
		data.gameState.players.map((p) => {
			const player = $state(p);
			player_map.set(p.id, player);
		});
		game.players = player_map;
		game.yak_counts = data.gameState.yak_counts;
		game.job_fees = data.gameState.job_fees;
		return data.gameState;
	} catch (e) {
		alert_appsync_error(e, 'Could not load game state');
		return null;
	}
}

// Subscribe to game updates
export function subscribe_player_game_updates() {
	subscribe_game_updates(get_appsync_client(), PLAYER_GAME_UPDATED_SUBSCRIPTION_QUERY);
}
// Subscribe to game updates
export function subscribe_admin_game_updates() {
	subscribe_game_updates(get_appsync_admin_client(), ADMIN_GAME_UPDATED_SUBSCRIPTION_QUERY);
}

// Subscribe to game updates
function subscribe_game_updates(client, query) {
	// 1. gameUpdated subscription (stored separately for pause/resume)
	game_updated_sub = client.graphql({ query }).subscribe({
		next: handle_game_updated,
		error: (e) => console.error('gameUpdated subscription error', e)
	});

	// 2. removedPlayer subscription
	const removed_sub = client
		.graphql({
			query: `
				subscription RemovedPlayer {
					removedPlayer {
						id
					}
				}
			`
		})
		.subscribe({
			next: ({ data }) => {
				if (data.removedPlayer) {
					const id = data.removedPlayer.id;
					game.players.delete(id);
					if (player.id && player.id === id) {
						clear_player();
					}
				}
			},
			error: (e) => console.error('removedPlayer subscription error', e)
		});
	subscriptions.push(removed_sub);
}

// Unsubscribe all
export function unsubscribe_all() {
	if (game_updated_sub) {
		game_updated_sub.unsubscribe();
		game_updated_sub = null;
	}
	subscriptions.forEach((sub) => sub.unsubscribe());
	subscriptions.splice(0);
	stop_flush_interval();
	changes_buffer.players.clear();
	changes_buffer.yak_counts = null;
	changes_buffer.job_fees = null;
}
