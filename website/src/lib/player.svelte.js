import { v4 as uuidv4 } from 'uuid';
import { browser } from '$app/environment';
import { alert_success, alert_appsync_error } from '$lib/alerts.svelte.js';
import { get_appsync_client } from '$lib/game.svelte';

export const player = $state({
	id: null,
	secret: null,
	name: null
});

if (browser) {
	const stored = localStorage.getItem('yak_mania_player');
	if (stored) {
		const parsed = JSON.parse(stored);
		player.id = parsed.id;
		player.secret = parsed.secret;
		player.name = parsed.name;
	}
}

if (browser) {
	$effect.root(() => {
		$effect(() => {
			if (player.id) {
				localStorage.setItem(
					'yak_mania_player',
					JSON.stringify({ id: player.id, secret: player.secret, name: player.name })
				);
			} else {
				localStorage.removeItem('yak_mania_player');
			}
		});
	});
}

export async function register_player(name) {
	const secret = uuidv4();
	try {
		const data = (
			await get_appsync_client().graphql({
				query: `
					mutation RegisterNewPlayer($name: String!, $secret: String!) {
						registerNewPlayer(name: $name, secret: $secret) {
				      sampled
							player {
								id
								name
								balance
        				yak_bred
        				yak_driven
        				yak_sheared
							}
						}
					}
				`,
				variables: { name, secret }
			})
		).data;

		player.secret = secret;
		player.id = data.registerNewPlayer.player.id;
		player.name = data.registerNewPlayer.player.name;

		alert_success('Player registered');
	} catch (e) {
		alert_appsync_error(e, 'Registration failed');
	}
}

export async function update_player_name(new_name) {
	try {
		const data = (
			await get_appsync_client().graphql({
				query: `
					mutation UpdatePlayerName($player_id: ID!, $secret: String!, $new_name: String!) {
						updatePlayerName(player_id: $player_id, secret: $secret, new_name: $new_name) {
						  sampled
							player {
								id
								name
								balance
        				yak_bred
        				yak_driven
        				yak_sheared
							}
						}
					}
				`,
				variables: { player_id: player.id, secret: player.secret, new_name }
			})
		).data;
		player.name = data.updatePlayerName.player.name;
		alert_success('Name updated');
	} catch (e) {
		alert_appsync_error(e, 'Could not update name');
	}
}

export function clear_player() {
	player.id = null;
	player.secret = null;
	player.name = null;
}
