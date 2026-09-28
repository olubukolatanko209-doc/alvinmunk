#![cfg(test)]
use super::*;
use soroban_sdk::{symbol_short, testutils::Address as _, vec, Address, Env};

fn setup() -> (Env, RegistryContractClient<'static>, Address) {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let id = env.register(RegistryContract, ());
    let client = RegistryContractClient::new(&env, &id);
    client.init(&admin);
    (env, client, admin)
}

#[test]
fn claim_sets_forward_and_reverse() {
    let (env, client, _admin) = setup();
    let alice = Address::generate(&env);
    client.claim(&alice, &symbol_short!("alice"));
    assert_eq!(client.resolve(&symbol_short!("alice")), Some(alice.clone()));
    assert_eq!(client.reverse(&alice), Some(symbol_short!("alice")));
}

#[test]
fn unknown_handle_resolves_none() {
    let (env, client, _admin) = setup();
    assert_eq!(client.resolve(&symbol_short!("nobody")), None);
    let ghost = Address::generate(&env);
    assert_eq!(client.reverse(&ghost), None);
}

#[test]
#[should_panic]
fn claim_taken_by_other_reverts() {
    let (env, client, _admin) = setup();
    let alice = Address::generate(&env);
    let bob = Address::generate(&env);
    client.claim(&alice, &symbol_short!("star"));
    client.claim(&bob, &symbol_short!("star")); // panics: HandleTaken
}

#[test]
fn reclaim_same_handle_is_idempotent() {
    let (env, client, _admin) = setup();
    let alice = Address::generate(&env);
    client.claim(&alice, &symbol_short!("alice"));
    client.claim(&alice, &symbol_short!("alice")); // no-op, no panic
    assert_eq!(client.resolve(&symbol_short!("alice")), Some(alice));
}

#[test]
fn rename_frees_the_old_handle() {
    let (env, client, _admin) = setup();
    let alice = Address::generate(&env);
    client.claim(&alice, &symbol_short!("old"));
    client.claim(&alice, &symbol_short!("new"));
    // old handle is freed; new one points to alice; reverse reflects the new one.
    assert_eq!(client.resolve(&symbol_short!("old")), None);
    assert_eq!(client.resolve(&symbol_short!("new")), Some(alice.clone()));
    assert_eq!(client.reverse(&alice), Some(symbol_short!("new")));
}

#[test]
fn release_frees_both_directions() {
    let (env, client, _admin) = setup();
    let alice = Address::generate(&env);
    client.claim(&alice, &symbol_short!("alice"));
    client.release(&alice);
    assert_eq!(client.resolve(&symbol_short!("alice")), None);
    assert_eq!(client.reverse(&alice), None);
}

#[test]
#[should_panic]
fn release_without_handle_reverts() {
    let (env, client, _admin) = setup();
    let alice = Address::generate(&env);
    client.release(&alice); // panics: NoHandle
}

#[test]
fn freed_handle_is_reclaimable_by_another() {
    let (env, client, _admin) = setup();
    let alice = Address::generate(&env);
    let bob = Address::generate(&env);
    client.claim(&alice, &symbol_short!("star"));
    client.release(&alice);
    client.claim(&bob, &symbol_short!("star"));
    assert_eq!(client.resolve(&symbol_short!("star")), Some(bob));
}

#[test]
fn admin_release_clears_a_squatted_handle() {
    let (env, client, _admin) = setup();
    let squatter = Address::generate(&env);
    let real = Address::generate(&env);
    client.claim(&squatter, &symbol_short!("brand"));
    client.admin_release(&symbol_short!("brand"));
    assert_eq!(client.resolve(&symbol_short!("brand")), None);
    assert_eq!(client.reverse(&squatter), None);
    // now the rightful owner can claim it
    client.claim(&real, &symbol_short!("brand"));
    assert_eq!(client.resolve(&symbol_short!("brand")), Some(real));
}

/// Release build of this contract, committed so the upgrade path can be tested without a
/// wasm build step in CI. Refresh with `make upgrade-fixtures` after changing the contract.
const REGISTRY_WASM: &[u8] = include_bytes!("../testdata/alvinmunk_registry.wasm");

// ── reverse_many tests ────────────────────────────────────────────────────────

#[test]
fn reverse_many_returns_handles_in_input_order() {
    let (env, client, _admin) = setup();
    let alice = Address::generate(&env);
    let bob = Address::generate(&env);
    let carol = Address::generate(&env);
    client.claim(&alice, &symbol_short!("alice"));
    client.claim(&bob, &symbol_short!("bob"));
    // carol has no handle
    let result = client.reverse_many(&vec![&env, alice.clone(), carol.clone(), bob.clone()]);
    assert_eq!(result.len(), 3);
    assert_eq!(result.get(0).unwrap(), Some(symbol_short!("alice")));
    assert_eq!(result.get(1).unwrap(), None);
    assert_eq!(result.get(2).unwrap(), Some(symbol_short!("bob")));
}

#[test]
fn reverse_many_returns_none_for_unclaimed_addresses() {
    let (env, client, _admin) = setup();
    let ghost = Address::generate(&env);
    let result = client.reverse_many(&vec![&env, ghost]);
    assert_eq!(result.len(), 1);
    assert_eq!(result.get(0).unwrap(), None);
}

#[test]
fn reverse_many_empty_input_returns_empty_vec() {
    let (env, client, _admin) = setup();
    let result = client.reverse_many(&vec![&env]);
    assert_eq!(result.len(), 0);
}

#[test]
#[should_panic]
fn reverse_many_over_cap_reverts() {
    let (env, client, _admin) = setup();
    // Build a vec of REVERSE_MANY_CAP + 1 addresses to trigger TooMany
    let mut addrs = vec![&env];
    for _ in 0..=REVERSE_MANY_CAP {
        addrs.push_back(Address::generate(&env));
    }
    client.reverse_many(&addrs); // panics: TooMany
}

#[test]
fn reverse_many_at_cap_does_not_revert() {
    let (env, client, _admin) = setup();
    let mut addrs = vec![&env];
    for _ in 0..REVERSE_MANY_CAP {
        addrs.push_back(Address::generate(&env));
    }
    // Exactly at cap — must not panic
    let result = client.reverse_many(&addrs);
    assert_eq!(result.len(), REVERSE_MANY_CAP);
}
#[test]
fn upgrade_to_identical_wasm_preserves_handles() {
    let (env, client, _admin) = setup();
    let alice = Address::generate(&env);
    client.claim(&alice, &symbol_short!("alice"));

    let hash = env.deployer().upload_contract_wasm(REGISTRY_WASM);
    client.upgrade(&hash);

    assert_eq!(client.resolve(&symbol_short!("alice")), Some(alice.clone()));
    assert_eq!(client.reverse(&alice), Some(symbol_short!("alice")));
}

#[test]
#[should_panic(expected = "HostError: Error(Auth, InvalidAction)")]
fn non_admin_upgrade_reverts() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let id = env.register(RegistryContract, ());
    let client = RegistryContractClient::new(&env, &id);
    client.init(&admin);
    let hash = soroban_sdk::BytesN::from_array(&env, &[1; 32]);
    client.upgrade(&hash);
}
