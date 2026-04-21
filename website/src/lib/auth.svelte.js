import { fetchAuthSession, signInWithRedirect, signOut } from 'aws-amplify/auth';
import { Hub } from 'aws-amplify/utils';
import { browser } from '$app/environment';
import { goto } from '$app/navigation';
import { resolve } from '$app/paths';

export const signed_user = $state({
	loading: true,
	id: null,
	email: null,
	groups: null,
	is_admin: null
});

export async function verify_user_signed_in() {
	signed_user.loading = true;
	try {
		const {
			idToken: { payload }
		} = (await fetchAuthSession()).tokens ?? {};
		const groups = payload['cognito:groups'];
		signed_user.id = payload.sub;
		signed_user.email = payload.email;
		signed_user.groups = groups;
		signed_user.is_admin = groups != null && groups.indexOf('Admins') >= 0;
	} catch {
		signed_user.id = null;
		signed_user.email = null;
		signed_user.groups = null;
		signed_user.is_admin = null;
	} finally {
		signed_user.loading = false;
	}
}
export const check_admin_and_redirect = () => {
	if (!signed_user.loading) {
		if (!signed_user.id) {
			signInWithRedirect();
		} else if (!signed_user.is_admin) {
			goto(resolve('/'));
		}
	}
};

export async function sign_out() {
	await goto(resolve('/'));
	await signOut();
}

if (browser) {
	Hub.listen('auth', ({ payload }) => {
		switch (payload.event) {
			case 'signedIn':
				verify_user_signed_in();
				break;
			case 'signedOut':
				verify_user_signed_in().then(() => {
					goto(resolve('/'));
				});
				break;
			case 'tokenRefresh':
				verify_user_signed_in();
				break;
		}
	});

	verify_user_signed_in();
}
