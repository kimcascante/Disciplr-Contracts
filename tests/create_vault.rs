#![cfg(test)]

extern crate std;

use soroban_sdk::{
    testutils::{Address as _, Ledger},
    token::StellarAssetClient,
    Address, BytesN, Env,
};

use disciplr_vault::{
    DisciplrVault, DisciplrVaultClient, VaultStatus, MAX_AMOUNT, MAX_VAULT_DURATION, MIN_AMOUNT,
};

fn setup() -> (
    Env,
    DisciplrVaultClient<'static>,
    Address,
    StellarAssetClient<'static>,
) {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(DisciplrVault, ());
    let client = DisciplrVaultClient::new(&env, &contract_id);

    let usdc_admin = Address::generate(&env);
    let usdc_token = env.register_stellar_asset_contract_v2(usdc_admin.clone());
    let usdc_addr = usdc_token.address();
    let usdc_asset = StellarAssetClient::new(&env, &usdc_addr);

    (env, client, usdc_addr, usdc_asset)
}

#[test]
fn test_create_vault_valid_boundary_values() {
    let (env, client, usdc, usdc_asset) = setup();

    let creator = Address::generate(&env);
    let now = 1_725_000_000u64;
    env.ledger().set_timestamp(now);

    usdc_asset.mint(&creator, &MIN_AMOUNT);

    let success = Address::generate(&env);
    let failure = Address::generate(&env);
    let milestone = BytesN::from_array(&env, &[0u8; 32]);

    let vault_id = client.create_vault(
        &usdc,
        &creator,
        &MIN_AMOUNT,
        &now,
        &(now + MAX_VAULT_DURATION),
        &milestone,
        &None,
        &success,
        &failure,
    );

    assert_eq!(vault_id, 0u32);
}

#[test]
#[should_panic(expected = "Error(Contract, #7)")]
fn test_amount_below_minimum() {
    let (env, client, usdc, _usdc_asset) = setup();

    let creator = Address::generate(&env);
    let now = env.ledger().timestamp();

    client.create_vault(
        &usdc,
        &creator,
        &(MIN_AMOUNT - 1),
        &now,
        &(now + 86_400),
        &BytesN::from_array(&env, &[0u8; 32]),
        &None,
        &Address::generate(&env),
        &Address::generate(&env),
    );
}

#[test]
#[should_panic(expected = "Error(Contract, #7)")]
fn test_amount_above_maximum() {
    let (env, client, usdc, _usdc_asset) = setup();

    let creator = Address::generate(&env);
    let now = env.ledger().timestamp();

    client.create_vault(
        &usdc,
        &creator,
        &(MAX_AMOUNT + 1),
        &now,
        &(now + 86_400),
        &BytesN::from_array(&env, &[0u8; 32]),
        &None,
        &Address::generate(&env),
        &Address::generate(&env),
    );
}

#[test]
#[should_panic(expected = "Error(Contract, #9)")]
fn test_duration_exceeds_max() {
    let (env, client, usdc, _usdc_asset) = setup();

    let creator = Address::generate(&env);
    let now = env.ledger().timestamp();

    client.create_vault(
        &usdc,
        &creator,
        &MIN_AMOUNT,
        &now,
        &(now + MAX_VAULT_DURATION + 1),
        &BytesN::from_array(&env, &[0u8; 32]),
        &None,
        &Address::generate(&env),
        &Address::generate(&env),
    );
}

#[test]
#[should_panic(expected = "Error(Contract, #4)")]
fn test_start_timestamp_in_past() {
    let (env, client, usdc, _usdc_asset) = setup();

    let creator = Address::generate(&env);
    let now = 1_725_000_000u64;
    env.ledger().set_timestamp(now);

    client.create_vault(
        &usdc,
        &creator,
        &MIN_AMOUNT,
        &(now - 3_600),
        &(now + 86_400),
        &BytesN::from_array(&env, &[0u8; 32]),
        &None,
        &Address::generate(&env),
        &Address::generate(&env),
    );
}

#[test]
#[should_panic(expected = "Error(Contract, #8)")]
fn test_end_before_or_equal_start() {
    let (env, client, usdc, _usdc_asset) = setup();

    let creator = Address::generate(&env);
    let now = 1_725_000_000u64;
    env.ledger().set_timestamp(now);

    client.create_vault(
        &usdc,
        &creator,
        &MIN_AMOUNT,
        &(now + 200),
        &(now + 100),
        &BytesN::from_array(&env, &[0u8; 32]),
        &None,
        &Address::generate(&env),
        &Address::generate(&env),
    );
}

#[test]
fn test_amount_exactly_max_allowed() {
    let (env, client, usdc, usdc_asset) = setup();

    let creator = Address::generate(&env);
    let now = 1_725_000_000u64;
    env.ledger().set_timestamp(now);

    usdc_asset.mint(&creator, &MAX_AMOUNT);

    let vault_id = client.create_vault(
        &usdc,
        &creator,
        &MAX_AMOUNT,
        &now,
        &(now + 86_400),
        &BytesN::from_array(&env, &[0u8; 32]),
        &None,
        &Address::generate(&env),
        &Address::generate(&env),
    );

    assert_eq!(vault_id, 0u32);
}

#[test]
#[should_panic(expected = "Error(Contract, #7)")]
fn test_amount_zero() {
    let (env, client, usdc, _usdc_asset) = setup();

    let creator = Address::generate(&env);
    let now = env.ledger().timestamp();

    client.create_vault(
        &usdc,
        &creator,
        &0_i128,
        &now,
        &(now + 86_400),
        &BytesN::from_array(&env, &[0u8; 32]),
        &None,
        &Address::generate(&env),
        &Address::generate(&env),
    );
}

#[test]
fn test_minimum_valid_duration() {
    let (env, client, usdc, usdc_asset) = setup();

    let creator = Address::generate(&env);
    let now = 1_725_000_000u64;
    env.ledger().set_timestamp(now);

    usdc_asset.mint(&creator, &MIN_AMOUNT);

    let vault_id = client.create_vault(
        &usdc,
        &creator,
        &MIN_AMOUNT,
        &now,
        &(now + 1),
        &BytesN::from_array(&env, &[0u8; 32]),
        &None,
        &Address::generate(&env),
        &Address::generate(&env),
    );

    assert_eq!(vault_id, 0u32);
}

#[test]
fn test_valid_zero_verifier_and_normal_duration() {
    let (env, client, usdc, usdc_asset) = setup();

    let creator = Address::generate(&env);
    let now = env.ledger().timestamp();

    usdc_asset.mint(&creator, &5_000_000_000_i128);

    client.create_vault(
        &usdc,
        &creator,
        &5_000_000_000_i128,
        &now,
        &(now + 7 * 24 * 60 * 60),
        &BytesN::from_array(&env, &[1u8; 32]),
        &None,
        &Address::generate(&env),
        &Address::generate(&env),
    );
}

#[test]
fn test_get_vault_state_never_created_id_returns_none() {
    let (_env, client, _usdc, _usdc_asset) = setup();

    assert_eq!(client.vault_count(), 0u32);
    assert!(client.get_vault_state(&0u32).is_none());
    assert!(client.get_vault_state(&42u32).is_none());
}

#[test]
fn test_get_vault_state_cancelled_vault_remains_readable() {
    let (env, client, usdc, usdc_asset) = setup();

    let creator = Address::generate(&env);
    let now = 1_725_000_000u64;
    env.ledger().set_timestamp(now);
    usdc_asset.mint(&creator, &(MIN_AMOUNT * 2));

    let vault_id = client.create_vault(
        &usdc,
        &creator,
        &MIN_AMOUNT,
        &now,
        &(now + 86_400),
        &BytesN::from_array(&env, &[9u8; 32]),
        &None,
        &Address::generate(&env),
        &Address::generate(&env),
    );

    assert_eq!(client.vault_count(), 1u32);
    client.cancel_vault(&vault_id, &usdc);

    let vault = client.get_vault_state(&vault_id).unwrap();
    assert_eq!(vault.status, VaultStatus::Cancelled);
    assert!(client.get_vault_state(&1u32).is_none());
}

// -----------------------------------------------------------------------
// Issue #140: Integration – full lifecycle success path
// -----------------------------------------------------------------------

/// e2e: create → validate → release funds to success_destination
#[test]
fn test_e2e_full_lifecycle_success() {
    let (env, client, usdc, usdc_asset) = setup();

    let creator = Address::generate(&env);
    let verifier = Address::generate(&env);
    let success_dest = Address::generate(&env);
    let failure_dest = Address::generate(&env);
    let now = 1_725_000_000u64;
    env.ledger().set_timestamp(now);

    usdc_asset.mint(&creator, &MIN_AMOUNT);

    // Step 1: create
    let vault_id = client.create_vault(
        &usdc,
        &creator,
        &MIN_AMOUNT,
        &now,
        &(now + 86_400),
        &BytesN::from_array(&env, &[7u8; 32]),
        &Some(verifier.clone()),
        &success_dest,
        &failure_dest,
    );
    let vault = client.get_vault_state(&vault_id).unwrap();
    assert_eq!(vault.status, VaultStatus::Active);
    assert!(!vault.milestone_validated);

    // Step 2: validate (before deadline)
    env.ledger().set_timestamp(now + 43_200);
    let validated = client.validate_milestone(&vault_id);
    assert!(validated);
    let vault = client.get_vault_state(&vault_id).unwrap();
    assert!(vault.milestone_validated);
    assert_eq!(vault.status, VaultStatus::Active);

    // Step 3: release
    let result = client.release_funds(&vault_id, &usdc);
    assert!(result);
    let vault = client.get_vault_state(&vault_id).unwrap();
    assert_eq!(vault.status, VaultStatus::Completed);
}

/// e2e: create → deadline reached → release without explicit validation
#[test]
fn test_e2e_lifecycle_release_after_deadline() {
    let (env, client, usdc, usdc_asset) = setup();

    let creator = Address::generate(&env);
    let success_dest = Address::generate(&env);
    let failure_dest = Address::generate(&env);
    let now = 1_725_000_000u64;
    env.ledger().set_timestamp(now);

    usdc_asset.mint(&creator, &MIN_AMOUNT);

    let vault_id = client.create_vault(
        &usdc,
        &creator,
        &MIN_AMOUNT,
        &now,
        &(now + 86_400),
        &BytesN::from_array(&env, &[8u8; 32]),
        &None,
        &success_dest,
        &failure_dest,
    );

    env.ledger().set_timestamp(now + 86_401);
    let result = client.release_funds(&vault_id, &usdc);
    assert!(result);
    assert_eq!(client.get_vault_state(&vault_id).unwrap().status, VaultStatus::Completed);
}

/// e2e: create → deadline reached without validation → redirect to failure_destination
#[test]
fn test_e2e_lifecycle_redirect_after_deadline() {
    let (env, client, usdc, usdc_asset) = setup();

    let creator = Address::generate(&env);
    let success_dest = Address::generate(&env);
    let failure_dest = Address::generate(&env);
    let now = 1_725_000_000u64;
    env.ledger().set_timestamp(now);

    usdc_asset.mint(&creator, &MIN_AMOUNT);

    let vault_id = client.create_vault(
        &usdc,
        &creator,
        &MIN_AMOUNT,
        &now,
        &(now + 86_400),
        &BytesN::from_array(&env, &[9u8; 32]),
        &None,
        &success_dest,
        &failure_dest,
    );

    env.ledger().set_timestamp(now + 86_401);
    let result = client.redirect_funds(&vault_id, &usdc);
    assert!(result);
    assert_eq!(client.get_vault_state(&vault_id).unwrap().status, VaultStatus::Failed);
}

/// e2e: create → cancel → funds returned to creator
#[test]
fn test_e2e_lifecycle_cancel() {
    let (env, client, usdc, usdc_asset) = setup();

    let creator = Address::generate(&env);
    let now = 1_725_000_000u64;
    env.ledger().set_timestamp(now);

    usdc_asset.mint(&creator, &MIN_AMOUNT);

    let vault_id = client.create_vault(
        &usdc,
        &creator,
        &MIN_AMOUNT,
        &now,
        &(now + 86_400),
        &BytesN::from_array(&env, &[5u8; 32]),
        &None,
        &Address::generate(&env),
        &Address::generate(&env),
    );

    let result = client.cancel_vault(&vault_id, &usdc);
    assert!(result);
    assert_eq!(client.get_vault_state(&vault_id).unwrap().status, VaultStatus::Cancelled);
}

// -----------------------------------------------------------------------
// Issue #144: Gas / budget – large milestone_hash stress
// -----------------------------------------------------------------------

/// Stress: 10 vaults with max-size milestone hashes stay within Soroban budget
#[test]
fn test_budget_large_milestone_hash_stress() {
    let (env, client, usdc, usdc_asset) = setup();

    let now = 1_725_000_000u64;
    env.ledger().set_timestamp(now);

    for i in 0u8..10 {
        let creator = Address::generate(&env);
        usdc_asset.mint(&creator, &MIN_AMOUNT);

        let hash = BytesN::from_array(&env, &[i; 32]);

        let vault_id = client.create_vault(
            &usdc,
            &creator,
            &MIN_AMOUNT,
            &now,
            &(now + 86_400),
            &hash,
            &None,
            &Address::generate(&env),
            &Address::generate(&env),
        );

        let vault = client.get_vault_state(&vault_id).unwrap();
        assert_eq!(vault.milestone_hash, hash);
    }
}

/// Stress: all-zeros and all-0xFF hashes are stored correctly
#[test]
fn test_budget_extreme_milestone_hash_values() {
    let (env, client, usdc, usdc_asset) = setup();

    let now = 1_725_000_000u64;
    env.ledger().set_timestamp(now);

    let hashes: [([u8; 32], u8); 2] = [([0x00; 32], 0), ([0xFF; 32], 1)];

    for (raw, _) in &hashes {
        let creator = Address::generate(&env);
        usdc_asset.mint(&creator, &MIN_AMOUNT);

        let hash = BytesN::from_array(&env, raw);
        let vault_id = client.create_vault(
            &usdc,
            &creator,
            &MIN_AMOUNT,
            &now,
            &(now + 86_400),
            &hash,
            &None,
            &Address::generate(&env),
            &Address::generate(&env),
        );

        assert_eq!(client.get_vault_state(&vault_id).unwrap().milestone_hash, hash);
    }
}

// -----------------------------------------------------------------------
// Issue #130: Event indexing – stable topic names and ordering
// -----------------------------------------------------------------------

use soroban_sdk::{Symbol, TryIntoVal};
use soroban_sdk::testutils::Events;

/// Verify vault_created event topic name is stable
#[test]
fn test_event_vault_created_topic() {
    let (env, client, usdc, usdc_asset) = setup();

    let creator = Address::generate(&env);
    let now = 1_725_000_000u64;
    env.ledger().set_timestamp(now);
    usdc_asset.mint(&creator, &MIN_AMOUNT);

    let vault_id = client.create_vault(
        &usdc,
        &creator,
        &MIN_AMOUNT,
        &now,
        &(now + 86_400),
        &BytesN::from_array(&env, &[1u8; 32]),
        &None,
        &Address::generate(&env),
        &Address::generate(&env),
    );

    let all_events = env.events().all();
    let found = all_events.iter().any(|(_, topics, _)| {
        if let Ok(name) = topics.get(0).unwrap().try_into_val::<Symbol>(&env) {
            name == Symbol::new(&env, "vault_created")
        } else {
            false
        }
    });
    assert!(found, "vault_created event must be emitted");
    let _ = vault_id;
}

/// Verify milestone_validated event topic name is stable
#[test]
fn test_event_milestone_validated_topic() {
    let (env, client, usdc, usdc_asset) = setup();

    let creator = Address::generate(&env);
    let now = 1_725_000_000u64;
    env.ledger().set_timestamp(now);
    usdc_asset.mint(&creator, &MIN_AMOUNT);

    let vault_id = client.create_vault(
        &usdc,
        &creator,
        &MIN_AMOUNT,
        &now,
        &(now + 86_400),
        &BytesN::from_array(&env, &[2u8; 32]),
        &None,
        &Address::generate(&env),
        &Address::generate(&env),
    );

    client.validate_milestone(&vault_id);

    let all_events = env.events().all();
    let found = all_events.iter().any(|(_, topics, _)| {
        if let Ok(name) = topics.get(0).unwrap().try_into_val::<Symbol>(&env) {
            name == Symbol::new(&env, "milestone_validated")
        } else {
            false
        }
    });
    assert!(found, "milestone_validated event must be emitted");
}

/// Verify funds_released event topic name is stable
#[test]
fn test_event_funds_released_topic() {
    let (env, client, usdc, usdc_asset) = setup();

    let creator = Address::generate(&env);
    let now = 1_725_000_000u64;
    env.ledger().set_timestamp(now);
    usdc_asset.mint(&creator, &MIN_AMOUNT);

    let vault_id = client.create_vault(
        &usdc,
        &creator,
        &MIN_AMOUNT,
        &now,
        &(now + 86_400),
        &BytesN::from_array(&env, &[3u8; 32]),
        &None,
        &Address::generate(&env),
        &Address::generate(&env),
    );

    env.ledger().set_timestamp(now + 86_401);
    client.release_funds(&vault_id, &usdc);

    let all_events = env.events().all();
    let found = all_events.iter().any(|(_, topics, _)| {
        if let Ok(name) = topics.get(0).unwrap().try_into_val::<Symbol>(&env) {
            name == Symbol::new(&env, "funds_released")
        } else {
            false
        }
    });
    assert!(found, "funds_released event must be emitted");
}

/// Verify funds_redirected event topic name is stable
#[test]
fn test_event_funds_redirected_topic() {
    let (env, client, usdc, usdc_asset) = setup();

    let creator = Address::generate(&env);
    let now = 1_725_000_000u64;
    env.ledger().set_timestamp(now);
    usdc_asset.mint(&creator, &MIN_AMOUNT);

    let vault_id = client.create_vault(
        &usdc,
        &creator,
        &MIN_AMOUNT,
        &now,
        &(now + 86_400),
        &BytesN::from_array(&env, &[4u8; 32]),
        &None,
        &Address::generate(&env),
        &Address::generate(&env),
    );

    env.ledger().set_timestamp(now + 86_401);
    client.redirect_funds(&vault_id, &usdc);

    let all_events = env.events().all();
    let found = all_events.iter().any(|(_, topics, _)| {
        if let Ok(name) = topics.get(0).unwrap().try_into_val::<Symbol>(&env) {
            name == Symbol::new(&env, "funds_redirected")
        } else {
            false
        }
    });
    assert!(found, "funds_redirected event must be emitted");
}

/// Verify vault_cancelled event topic name is stable
#[test]
fn test_event_vault_cancelled_topic() {
    let (env, client, usdc, usdc_asset) = setup();

    let creator = Address::generate(&env);
    let now = 1_725_000_000u64;
    env.ledger().set_timestamp(now);
    usdc_asset.mint(&creator, &MIN_AMOUNT);

    let vault_id = client.create_vault(
        &usdc,
        &creator,
        &MIN_AMOUNT,
        &now,
        &(now + 86_400),
        &BytesN::from_array(&env, &[6u8; 32]),
        &None,
        &Address::generate(&env),
        &Address::generate(&env),
    );

    client.cancel_vault(&vault_id, &usdc);

    let all_events = env.events().all();
    let found = all_events.iter().any(|(_, topics, _)| {
        if let Ok(name) = topics.get(0).unwrap().try_into_val::<Symbol>(&env) {
            name == Symbol::new(&env, "vault_cancelled")
        } else {
            false
        }
    });
    assert!(found, "vault_cancelled event must be emitted");
}
