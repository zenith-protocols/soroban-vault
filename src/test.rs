#![cfg(test)]

use soroban_sdk::{
    testutils::{Address as _, Ledger},
    token::StellarAssetClient,
    Address, Env, String, Vec,
};

use crate::{StrategyVaultContract, VaultContractClient};

const SCALAR_7: i128 = 10_000_000;
const LOCK_TIME: u64 = 300;

fn setup_test<'a>() -> (Env, VaultContractClient<'a>, Address, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let token = env.register_stellar_asset_contract_v2(admin.clone());
    let user = Address::generate(&env);
    let strategy = Address::generate(&env);

    // Fund user
    StellarAssetClient::new(&env, &token.address()).mint(&user, &(100_000 * SCALAR_7));

    // Deploy vault
    let strategies = Vec::from_array(&env, [strategy.clone()]);
    let vault_address = env.register(
        StrategyVaultContract,
        (
            String::from_str(&env, "Vault Shares"),
            String::from_str(&env, "vTKN"),
            token.address(),
            0u32,
            strategies,
            LOCK_TIME,
        ),
    );

    let vault = VaultContractClient::new(&env, &vault_address);
    (env, vault, token.address(), user, strategy)
}

// ==================== Lock Mechanism Tests ====================

#[test]
fn test_deposit_sets_lock() {
    let (_env, vault, _, user, _) = setup_test();

    vault.deposit(&(1000 * SCALAR_7), &user, &user, &user);

    assert!(vault.is_locked(&user));
    assert_eq!(vault.max_redeem(&user), 0);
}

#[test]
fn test_mint_sets_lock() {
    let (_env, vault, _, user, _) = setup_test();

    vault.mint(&(1000 * SCALAR_7), &user, &user, &user);

    assert!(vault.is_locked(&user));
}

#[test]
fn test_max_withdraw_returns_zero_when_locked() {
    let (_env, vault, _, user, _) = setup_test();

    vault.deposit(&(1000 * SCALAR_7), &user, &user, &user);

    assert_eq!(vault.max_withdraw(&user), 0);
}

#[test]
fn test_lock_time_returns_configured_value() {
    let (_env, vault, _, _, _) = setup_test();

    assert_eq!(vault.lock_time(), LOCK_TIME);
}

#[test]
fn test_unlock_after_lock_time() {
    let (env, vault, _, user, _) = setup_test();

    vault.deposit(&(1000 * SCALAR_7), &user, &user, &user);

    // Advance past lock time
    env.ledger().set_timestamp(env.ledger().timestamp() + LOCK_TIME + 1);

    assert!(!vault.is_locked(&user));
    assert!(vault.max_redeem(&user) > 0);
}

#[test]
fn test_new_deposit_resets_lock() {
    let (env, vault, _, user, _) = setup_test();

    vault.deposit(&(1000 * SCALAR_7), &user, &user, &user);

    // Advance halfway
    env.ledger().set_timestamp(env.ledger().timestamp() + LOCK_TIME / 2);
    assert!(vault.is_locked(&user));

    // New deposit resets lock
    vault.deposit(&(500 * SCALAR_7), &user, &user, &user);

    // Advance another half - still locked due to reset
    env.ledger().set_timestamp(env.ledger().timestamp() + LOCK_TIME / 2);
    assert!(vault.is_locked(&user));

    // Advance past new lock
    env.ledger().set_timestamp(env.ledger().timestamp() + LOCK_TIME / 2 + 1);
    assert!(!vault.is_locked(&user));
}

#[test]
#[should_panic(expected = "Error(Contract, #421)")] // SharesLocked
fn test_redeem_while_locked_fails() {
    let (_, vault, _, user, _) = setup_test();

    vault.deposit(&(1000 * SCALAR_7), &user, &user, &user);
    vault.redeem(&(500 * SCALAR_7), &user, &user, &user);
}

#[test]
#[should_panic(expected = "Error(Contract, #421)")] // SharesLocked
fn test_withdraw_while_locked_fails() {
    let (_, vault, _, user, _) = setup_test();

    vault.deposit(&(1000 * SCALAR_7), &user, &user, &user);
    vault.withdraw(&(500 * SCALAR_7), &user, &user, &user);
}

// ==================== Transfer Exploit Prevention ====================

#[test]
fn test_user_without_deposit_history_is_not_locked() {
    let (env, vault, _, _user, _) = setup_test();
    let recipient = Address::generate(&env);

    // User who never deposited is NOT locked (received shares via transfer)
    assert!(!vault.is_locked(&recipient));
}

#[test]
#[should_panic(expected = "Error(Contract, #421)")] // SharesLocked
fn test_transfer_while_locked_fails() {
    let (env, vault, _, user, _) = setup_test();
    let recipient = Address::generate(&env);

    vault.deposit(&(1000 * SCALAR_7), &user, &user, &user);
    assert!(vault.is_locked(&user));

    // Transfer should fail while locked
    vault.transfer(&user, &recipient, &(500 * SCALAR_7));
}

#[test]
fn test_transfer_after_unlock_succeeds() {
    let (env, vault, _, user, _) = setup_test();
    let recipient = Address::generate(&env);

    vault.deposit(&(1000 * SCALAR_7), &user, &user, &user);

    // Wait for lock to expire
    env.ledger().set_timestamp(env.ledger().timestamp() + LOCK_TIME + 1);
    assert!(!vault.is_locked(&user));

    // Transfer should succeed
    vault.transfer(&user, &recipient, &(500 * SCALAR_7));

    // Recipient can immediately redeem (no deposit history = not locked)
    assert!(!vault.is_locked(&recipient));
    assert!(vault.max_redeem(&recipient) > 0);
}

#[test]
#[should_panic(expected = "Error(Contract, #421)")] // SharesLocked
fn test_transfer_from_while_locked_fails() {
    let (env, vault, _, user, _) = setup_test();
    let spender = Address::generate(&env);
    let recipient = Address::generate(&env);

    vault.deposit(&(1000 * SCALAR_7), &user, &user, &user);
    vault.approve(&user, &spender, &(500 * SCALAR_7), &1000);

    // transfer_from should fail while owner is locked
    vault.transfer_from(&spender, &user, &recipient, &(500 * SCALAR_7));
}

#[test]
fn test_transfer_from_after_unlock_succeeds() {
    let (env, vault, _, user, _) = setup_test();
    let spender = Address::generate(&env);
    let recipient = Address::generate(&env);

    vault.deposit(&(1000 * SCALAR_7), &user, &user, &user);
    vault.approve(&user, &spender, &(500 * SCALAR_7), &1000);

    // Wait for lock to expire
    env.ledger().set_timestamp(env.ledger().timestamp() + LOCK_TIME + 1);

    // transfer_from should succeed
    vault.transfer_from(&spender, &user, &recipient, &(500 * SCALAR_7));

    // Recipient can immediately redeem
    assert!(!vault.is_locked(&recipient));
    assert!(vault.max_redeem(&recipient) > 0);
}

// ==================== Strategy Tests ====================

#[test]
fn test_strategy_withdraw_decreases_assets() {
    let (_env, vault, _token, user, strategy) = setup_test();

    vault.deposit(&(10_000 * SCALAR_7), &user, &user, &user);
    let initial_assets = vault.total_assets();

    // Strategy withdraws
    vault.strategy_withdraw(&strategy, &(2000 * SCALAR_7));

    assert_eq!(vault.total_assets(), initial_assets - 2000 * SCALAR_7);
    assert_eq!(vault.net_impact(&strategy), -(2000 * SCALAR_7));
}

#[test]
fn test_strategy_deposit_increases_assets() {
    let (env, vault, token, user, strategy) = setup_test();

    vault.deposit(&(10_000 * SCALAR_7), &user, &user, &user);

    // Fund strategy and deposit back with profit
    StellarAssetClient::new(&env, &token).mint(&strategy, &(3000 * SCALAR_7));
    vault.strategy_deposit(&strategy, &(3000 * SCALAR_7));

    assert_eq!(vault.total_assets(), 13_000 * SCALAR_7);
    assert_eq!(vault.net_impact(&strategy), 3000 * SCALAR_7);
}

#[test]
#[should_panic(expected = "Error(Contract, #422)")] // UnauthorizedStrategy
fn test_unauthorized_strategy_fails() {
    let (env, vault, _, user, _) = setup_test();
    let fake_strategy = Address::generate(&env);

    vault.deposit(&(10_000 * SCALAR_7), &user, &user, &user);
    vault.strategy_withdraw(&fake_strategy, &(1000 * SCALAR_7));
}

#[test]
#[should_panic(expected = "Error(Contract, #420)")] // InvalidAmount
fn test_zero_strategy_withdraw_fails() {
    let (_, vault, _, user, strategy) = setup_test();

    vault.deposit(&(10_000 * SCALAR_7), &user, &user, &user);
    vault.strategy_withdraw(&strategy, &0);
}

#[test]
#[should_panic(expected = "Error(Contract, #420)")] // InvalidAmount
fn test_zero_strategy_deposit_fails() {
    let (env, vault, token, user, strategy) = setup_test();

    vault.deposit(&(10_000 * SCALAR_7), &user, &user, &user);
    StellarAssetClient::new(&env, &token).mint(&strategy, &(1000 * SCALAR_7));
    vault.strategy_deposit(&strategy, &0);
}

#[test]
#[should_panic(expected = "Error(Contract, #422)")] // UnauthorizedStrategy
fn test_unauthorized_strategy_deposit_fails() {
    let (env, vault, token, user, _) = setup_test();
    let fake_strategy = Address::generate(&env);

    vault.deposit(&(10_000 * SCALAR_7), &user, &user, &user);
    StellarAssetClient::new(&env, &token).mint(&fake_strategy, &(1000 * SCALAR_7));
    vault.strategy_deposit(&fake_strategy, &(1000 * SCALAR_7));
}
