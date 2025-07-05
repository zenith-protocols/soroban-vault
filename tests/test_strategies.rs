mod common;
use common::*;
use soroban_sdk::{Address, testutils::Address as _};

#[test]
fn test_strategy_authorization() {
    let test_env = setup_vault();
    let user = test_env.users.get(0).unwrap();
    let strategy = test_env.strategies.get(0).unwrap();

    // Fund vault
    test_env.vault.deposit(&(10_000 * SCALAR_7), &user, &user);

    // Authorized strategy can borrow
    test_env.vault.borrow(&strategy, &(1000 * SCALAR_7));

    // Check strategy data
    let strategy_data = test_env.vault.get_strategy(&strategy);
    assert_eq!(strategy_data.borrowed, 1000 * SCALAR_7);
    assert_eq!(strategy_data.net_impact, 0); // Borrow doesn't affect net_impact
}

#[test]
#[should_panic(expected = "Error(Contract, #4045)")] // UnauthorizedStrategy
fn test_unauthorized_strategy_fails() {
    let test_env = setup_vault();
    let user = test_env.users.get(0).unwrap();
    let fake_strategy = Address::generate(&test_env.env);

    // Fund vault
    test_env.vault.deposit(&(10_000 * SCALAR_7), &user, &user);

    // Unauthorized strategy cannot borrow
    test_env.vault.borrow(&fake_strategy, &(1000 * SCALAR_7));
}

#[test]
fn test_borrow_and_repay_cycle() {
    let test_env = setup_vault();
    let user = test_env.users.get(0).unwrap();
    let strategy = test_env.strategies.get(0).unwrap();

    // Fund vault
    test_env.vault.deposit(&(5000 * SCALAR_7), &user, &user);

    let initial_vault_balance = test_env.vault_balance();
    let initial_strategy_balance = test_env.token_balance(&strategy);

    // Borrow
    let borrow_amount = 2000 * SCALAR_7;
    test_env.vault.borrow(&strategy, &borrow_amount);

    // Check balances after borrow
    assert_eq!(test_env.vault_balance(), initial_vault_balance - borrow_amount);
    assert_eq!(test_env.token_balance(&strategy), initial_strategy_balance + borrow_amount);

    let strategy_data = test_env.vault.get_strategy(&strategy);
    assert_eq!(strategy_data.borrowed, borrow_amount);

    // Fund strategy for repayment
    test_env.mint_tokens(&strategy, borrow_amount);

    // Repay
    test_env.vault.repay(&strategy, &borrow_amount);

    // Check final state
    assert_eq!(test_env.vault_balance(), initial_vault_balance);

    let strategy_data = test_env.vault.get_strategy(&strategy);
    assert_eq!(strategy_data.borrowed, 0);
    assert_eq!(strategy_data.net_impact, 0);
}

#[test]
fn test_partial_repayments() {
    let test_env = setup_vault();
    let user = test_env.users.get(0).unwrap();
    let strategy = test_env.strategies.get(0).unwrap();

    // Setup
    test_env.vault.deposit(&(10_000 * SCALAR_7), &user, &user);
    test_env.vault.borrow(&strategy, &(3000 * SCALAR_7));

    // Repay in parts
    test_env.mint_tokens(&strategy, 3000 * SCALAR_7);

    test_env.vault.repay(&strategy, &(1000 * SCALAR_7));
    let data = test_env.vault.get_strategy(&strategy);
    assert_eq!(data.borrowed, 2000 * SCALAR_7);

    test_env.vault.repay(&strategy, &(1500 * SCALAR_7));
    let data = test_env.vault.get_strategy(&strategy);
    assert_eq!(data.borrowed, 500 * SCALAR_7);

    test_env.vault.repay(&strategy, &(500 * SCALAR_7));
    let data = test_env.vault.get_strategy(&strategy);
    assert_eq!(data.borrowed, 0);
}

#[test]
fn test_transfer_to_strategy() {
    let test_env = setup_vault();
    let user = test_env.users.get(0).unwrap();
    let strategy = test_env.strategies.get(0).unwrap();

    // Fund vault
    test_env.vault.deposit(&(5000 * SCALAR_7), &user, &user);

    // Transfer to strategy
    let transfer_amount = 1500 * SCALAR_7;
    test_env.vault.transfer_to(&strategy, &transfer_amount);

    // Check balances
    assert_eq!(test_env.token_balance(&strategy), transfer_amount);
    assert_eq!(test_env.vault_balance(), 3500 * SCALAR_7);

    // Check accounting
    let strategy_data = test_env.vault.get_strategy(&strategy);
    assert_eq!(strategy_data.borrowed, 0); // Not a borrow
    assert_eq!(strategy_data.net_impact, -transfer_amount); // Negative impact

    // Total tokens should decrease
    assert_eq!(test_env.vault.total_tokens(), 3500 * SCALAR_7);
}

#[test]
fn test_transfer_from_strategy() {
    let test_env = setup_vault();
    let user = test_env.users.get(0).unwrap();
    let strategy = test_env.strategies.get(0).unwrap();

    // Fund vault and strategy
    test_env.vault.deposit(&(5000 * SCALAR_7), &user, &user);
    test_env.mint_tokens(&strategy, 2000 * SCALAR_7);

    // Transfer from strategy
    let transfer_amount = 2000 * SCALAR_7;
    test_env.vault.transfer_from(&strategy, &transfer_amount);

    // Check balances
    assert_eq!(test_env.token_balance(&strategy), 0);
    assert_eq!(test_env.vault_balance(), 7000 * SCALAR_7);

    // Check accounting
    let strategy_data = test_env.vault.get_strategy(&strategy);
    assert_eq!(strategy_data.borrowed, 0);
    assert_eq!(strategy_data.net_impact, transfer_amount); // Positive impact

    // Total tokens should increase
    assert_eq!(test_env.vault.total_tokens(), 7000 * SCALAR_7);
}

#[test]
fn test_strategy_profit_scenario() {
    let test_env = setup_vault();
    let user = test_env.users.get(0).unwrap();
    let strategy = test_env.strategies.get(0).unwrap();

    // Initial state
    test_env.vault.deposit(&(10_000 * SCALAR_7), &user, &user);

    // Strategy receives funds
    test_env.vault.transfer_to(&strategy, &(3000 * SCALAR_7));

    // Strategy makes 20% profit
    let profit = 600 * SCALAR_7;
    test_env.mint_tokens(&strategy, 3000 * SCALAR_7 + profit);

    // Return funds with profit
    test_env.vault.transfer_from(&strategy, &(3600 * SCALAR_7));

    // Check final state
    let strategy_data = test_env.vault.get_strategy(&strategy);
    assert_eq!(strategy_data.net_impact, 600 * SCALAR_7); // Net profit
    assert_eq!(test_env.vault.total_tokens(), 10_600 * SCALAR_7);

    // Share price should reflect profit
    let shares_for_1000 = test_env.vault.deposit(&(1060 * SCALAR_7), &user, &user);
    assert_eq!(shares_for_1000, 1000 * SCALAR_7); // Price is 1.06
}

#[test]
fn test_strategy_loss_scenario() {
    let test_env = setup_vault();
    let user = test_env.users.get(0).unwrap();
    let strategy = test_env.strategies.get(0).unwrap();

    // Initial state
    test_env.vault.deposit(&(10_000 * SCALAR_7), &user, &user);

    // Strategy borrows
    test_env.vault.borrow(&strategy, &(4000 * SCALAR_7));

    // Strategy incurs 25% loss
    let loss = 1000 * SCALAR_7;
    test_env.mint_tokens(&strategy, 4000 * SCALAR_7 - loss);

    // Repay what's left
    test_env.vault.repay(&strategy, &(3000 * SCALAR_7));

    // Strategy still owes 1000 but can't pay
    let strategy_data = test_env.vault.get_strategy(&strategy);
    assert_eq!(strategy_data.borrowed, 1000 * SCALAR_7);

    // This represents a real loss to the vault
    // In practice, vault would need to write off the debt
}

#[test]
fn test_multiple_strategies() {
    let config = VaultConfig {
        num_strategies: 3,
        ..Default::default()
    };
    let test_env = setup_vault_with_config(config);
    let user = test_env.users.get(0).unwrap();

    // Fund vault
    test_env.vault.deposit(&(10_000 * SCALAR_7), &user, &user);

    // Each strategy borrows different amounts
    let strategy1 = test_env.strategies.get(0).unwrap();
    let strategy2 = test_env.strategies.get(1).unwrap();
    let strategy3 = test_env.strategies.get(2).unwrap();

    test_env.vault.borrow(&strategy1, &(3000 * SCALAR_7));
    test_env.vault.borrow(&strategy2, &(2000 * SCALAR_7));
    test_env.vault.borrow(&strategy3, &(1000 * SCALAR_7));

    // Check individual tracking
    assert_eq!(test_env.vault.get_strategy(&strategy1).borrowed, 3000 * SCALAR_7);
    assert_eq!(test_env.vault.get_strategy(&strategy2).borrowed, 2000 * SCALAR_7);
    assert_eq!(test_env.vault.get_strategy(&strategy3).borrowed, 1000 * SCALAR_7);

    // Vault balance should reflect all borrows
    assert_eq!(test_env.vault_balance(), 4000 * SCALAR_7);
}

#[test]
#[should_panic(expected = "Error(Contract, #4042)")] // InsufficientVaultBalance
fn test_minimum_liquidity_constraint() {
    let config = VaultConfig {
        min_liquidity_rate: SCALAR_7 / 2, // 50% minimum
        ..Default::default()
    };
    let test_env = setup_vault_with_config(config);
    let user = test_env.users.get(0).unwrap();
    let strategy = test_env.strategies.get(0).unwrap();

    // Deposit
    test_env.vault.deposit(&(10_000 * SCALAR_7), &user, &user);

    // Can borrow up to 50%
    test_env.vault.borrow(&strategy, &(5000 * SCALAR_7));

    // Should fail to borrow more (exceeds liquidity limit)
    test_env.vault.borrow(&strategy, &(1 * SCALAR_7));
}

#[test]
#[should_panic(expected = "Error(Contract, #4042)")] // InsufficientVaultBalance
fn test_transfer_operations_affect_liquidity() {
    let test_env = setup_vault();
    let user = test_env.users.get(0).unwrap();
    let strategy = test_env.strategies.get(0).unwrap();

    // Setup
    test_env.vault.deposit(&(10_000 * SCALAR_7), &user, &user);

    // Transfer reduces available liquidity
    test_env.vault.transfer_to(&strategy, &(3000 * SCALAR_7));

    // Try to borrow more than remaining - this should fail
    test_env.vault.borrow(&strategy, &(6000 * SCALAR_7));
}

#[test]
fn test_transfer_operations_liquidity_recovery() {
    let test_env = setup_vault();
    let user = test_env.users.get(0).unwrap();
    let strategy = test_env.strategies.get(0).unwrap();

    // Setup
    test_env.vault.deposit(&(10_000 * SCALAR_7), &user, &user);

    // Transfer reduces available liquidity
    test_env.vault.transfer_to(&strategy, &(3000 * SCALAR_7));

    // Return funds
    test_env.mint_tokens(&strategy, 3000 * SCALAR_7);
    test_env.vault.transfer_from(&strategy, &(3000 * SCALAR_7));

    // Now can borrow again
    test_env.vault.borrow(&strategy, &(6000 * SCALAR_7));
}

#[test]
#[should_panic(expected = "Error(Contract, #4041)")] // InvalidAmount
fn test_over_repay_fails() {
    let test_env = setup_vault();
    let user = test_env.users.get(0).unwrap();
    let strategy = test_env.strategies.get(0).unwrap();

    // Setup
    test_env.vault.deposit(&(5000 * SCALAR_7), &user, &user);
    test_env.vault.borrow(&strategy, &(1000 * SCALAR_7));

    // Try to repay more than borrowed
    test_env.mint_tokens(&strategy, 2000 * SCALAR_7);
    test_env.vault.repay(&strategy, &(2000 * SCALAR_7));
}

#[test]
#[should_panic(expected = "Error(Contract, #4042)")] // InsufficientVaultBalance
fn test_insufficient_balance_for_borrow() {
    let test_env = setup_vault();
    let user = test_env.users.get(0).unwrap();
    let strategy = test_env.strategies.get(0).unwrap();

    // Small deposit
    test_env.vault.deposit(&(1000 * SCALAR_7), &user, &user);

    // Try to borrow more than available
    test_env.vault.borrow(&strategy, &(2000 * SCALAR_7));
}

#[test]
#[should_panic(expected = "Error(Contract, #4041)")] // InvalidAmount
fn test_zero_strategy_operations_fail() {
    let test_env = setup_vault();
    let strategy = test_env.strategies.get(0).unwrap();

    // All should fail
    test_env.vault.borrow(&strategy, &0);
}

#[test]
fn test_net_impact_tracking() {
    let test_env = setup_vault();
    let user = test_env.users.get(0).unwrap();
    let strategy = test_env.strategies.get(0).unwrap();

    test_env.vault.deposit(&(10_000 * SCALAR_7), &user, &user);
    test_env.mint_tokens(&strategy, 10_000 * SCALAR_7);

    // Series of operations
    test_env.vault.transfer_to(&strategy, &(1000 * SCALAR_7));     // -1000
    test_env.vault.transfer_from(&strategy, &(1500 * SCALAR_7));   // +500
    test_env.vault.transfer_to(&strategy, &(2000 * SCALAR_7));     // -1500
    test_env.vault.transfer_from(&strategy, &(3000 * SCALAR_7));   // +1500

    let strategy_data = test_env.vault.get_strategy(&strategy);
    assert_eq!(strategy_data.net_impact, 1500 * SCALAR_7);

    // Borrows don't affect net impact
    test_env.vault.borrow(&strategy, &(1000 * SCALAR_7));
    let strategy_data = test_env.vault.get_strategy(&strategy);
    assert_eq!(strategy_data.net_impact, 1500 * SCALAR_7);
}