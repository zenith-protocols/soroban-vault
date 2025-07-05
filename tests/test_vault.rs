mod common;
use common::*;

#[test]
fn test_first_deposit_one_to_one() {
    let test_env = setup_vault();
    let user = test_env.users.get(0).unwrap();

    // First deposit should get 1:1 ratio
    let deposit_amount = 1000 * SCALAR_7;
    let initial_balance = test_env.token_balance(&user);

    let shares = test_env.vault.deposit(&deposit_amount, &user, &user);

    assert_eq!(shares, deposit_amount, "First deposit should be 1:1");
    assert_eq!(test_env.vault.total_shares(), deposit_amount);
    assert_eq!(test_env.vault.total_tokens(), deposit_amount);
    assert_eq!(test_env.token_balance(&user), initial_balance - deposit_amount);
    assert_eq!(test_env.vault_balance(), deposit_amount);
}

#[test]
fn test_multiple_deposits_same_price() {
    let test_env = setup_vault();
    let user1 = test_env.users.get(0).unwrap();
    let user2 = test_env.users.get(1).unwrap();

    // First deposit
    let deposit1 = 2000 * SCALAR_7;
    let shares1 = test_env.vault.deposit(&deposit1, &user1, &user1);
    assert_eq!(shares1, deposit1);

    // Second deposit (no profit/loss yet, so still 1:1)
    let deposit2 = 1500 * SCALAR_7;
    let shares2 = test_env.vault.deposit(&deposit2, &user2, &user2);
    assert_eq!(shares2, deposit2);

    // Verify totals
    assert_eq!(test_env.vault.total_shares(), shares1 + shares2);
    assert_eq!(test_env.vault.total_tokens(), deposit1 + deposit2);
    assert_eq!(test_env.vault_balance(), deposit1 + deposit2);
}

#[test]
fn test_deposit_after_profit() {
    let test_env = setup_vault();
    let user1 = test_env.users.get(0).unwrap();
    let user2 = test_env.users.get(1).unwrap();
    let strategy = test_env.strategies.get(0).unwrap();

    // Initial deposit
    let initial_deposit = 1000 * SCALAR_7;
    test_env.vault.deposit(&initial_deposit, &user1, &user1);

    // Simulate 20% profit - mint tokens to strategy first
    let profit = 200 * SCALAR_7;
    test_env.mint_tokens(&strategy, profit);

    // Strategy transfers profit to vault
    test_env.vault.transfer_from(&strategy, &profit);

    // New deposit should get fewer shares due to appreciation
    let deposit2 = 600 * SCALAR_7;
    let shares2 = test_env.vault.deposit(&deposit2, &user2, &user2);

    // Share price is now 1.2, so 600 tokens = 500 shares
    assert_eq!(shares2, 500 * SCALAR_7);
    assert_eq!(test_env.vault.total_shares(), 1500 * SCALAR_7);
    assert_eq!(test_env.vault.total_tokens(), 1800 * SCALAR_7);
}

#[test]
fn test_deposit_after_loss() {
    let test_env = setup_vault();
    let user1 = test_env.users.get(0).unwrap();
    let user2 = test_env.users.get(1).unwrap();
    let strategy = test_env.strategies.get(0).unwrap();

    // Initial deposit
    let initial_deposit = 1000 * SCALAR_7;
    test_env.vault.deposit(&initial_deposit, &user1, &user1);

    // Simulate 20% loss via strategy
    let loss = 200 * SCALAR_7;
    test_env.vault.transfer_to(&strategy, &loss);

    // New deposit should get more shares due to depreciation
    let deposit2 = 400 * SCALAR_7;
    let shares2 = test_env.vault.deposit(&deposit2, &user2, &user2);

    // Share price is now 0.8, so 400 tokens = 500 shares
    assert_eq!(shares2, 500 * SCALAR_7);
    assert_eq!(test_env.vault.total_shares(), 1500 * SCALAR_7);
    assert_eq!(test_env.vault.total_tokens(), 1200 * SCALAR_7);
}

#[test]
fn test_mint_exact_shares() {
    let test_env = setup_vault();
    let user = test_env.users.get(0).unwrap();

    // First mint (1:1 ratio)
    let exact_shares = 1000 * SCALAR_7;
    let tokens_used = test_env.vault.mint(&exact_shares, &user, &user);

    assert_eq!(tokens_used, exact_shares, "First mint should be 1:1");
    assert_eq!(test_env.share_balance(&user), exact_shares);
    assert_eq!(test_env.vault.total_shares(), exact_shares);
}

#[test]
fn test_mint_after_appreciation() {
    let test_env = setup_vault();
    let user1 = test_env.users.get(0).unwrap();
    let user2 = test_env.users.get(1).unwrap();
    let strategy = test_env.strategies.get(0).unwrap();

    // Initial deposit
    test_env.vault.deposit(&(1000 * SCALAR_7), &user1, &user1);

    // Add profit
    let profit = 500 * SCALAR_7;
    test_env.mint_tokens(&strategy, profit);
    test_env.vault.transfer_from(&strategy, &profit);

    // Mint exact shares (share price is now 1.5)
    let exact_shares = 200 * SCALAR_7;
    let tokens_used = test_env.vault.mint(&exact_shares, &user2, &user2);

    assert_eq!(tokens_used, 300 * SCALAR_7); // 200 shares * 1.5
    assert_eq!(test_env.share_balance(&user2), exact_shares);
}

#[test]
fn test_request_redeem_basic() {
    let test_env = setup_vault();
    let user = test_env.users.get(0).unwrap();

    // Deposit first
    let shares = test_env.vault.deposit(&(1000 * SCALAR_7), &user, &user);

    // Request redemption for half
    let redeem_shares = shares / 2;
    test_env.vault.request_redeem(&redeem_shares, &user);

    // Check shares are locked in vault
    assert_eq!(test_env.share_balance(&user), shares - redeem_shares);
    assert_eq!(test_env.share_token_client().balance(&test_env.vault.address), redeem_shares);
}

#[test]
fn test_redeem_after_lock_period() {
    let test_env = setup_vault();
    let user = test_env.users.get(0).unwrap();

    // Deposit and request redemption
    let deposit = 1000 * SCALAR_7;
    let shares = test_env.vault.deposit(&deposit, &user, &user);
    test_env.vault.request_redeem(&shares, &user);

    // Advance past lock time
    test_env.advance_past_lock();

    // Execute redemption
    let tokens_received = test_env.vault.redeem(&user, &user);

    assert_eq!(tokens_received, deposit);
    assert_eq!(test_env.share_balance(&user), 0);
    assert_eq!(test_env.vault.total_shares(), 0);
    assert_eq!(test_env.vault.total_tokens(), 0);
}

#[test]
fn test_redeem_at_exact_unlock_time() {
    let test_env = setup_vault();
    let user = test_env.users.get(0).unwrap();

    // Deposit and request redemption
    test_env.vault.deposit(&(1000 * SCALAR_7), &user, &user);
    test_env.vault.request_redeem(&(1000 * SCALAR_7), &user);

    // Advance to exact unlock time
    test_env.advance_to_unlock();

    // Should work at exact time
    let tokens = test_env.vault.redeem(&user, &user);
    assert_eq!(tokens, 1000 * SCALAR_7);
}

#[test]
fn test_emergency_redeem_with_penalty() {
    let test_env = setup_vault();
    let user = test_env.users.get(0).unwrap();

    // Deposit and request redemption
    let deposit = 1000 * SCALAR_7;
    test_env.vault.deposit(&deposit, &user, &user);
    test_env.vault.request_redeem(&(1000 * SCALAR_7), &user);

    // Emergency redeem immediately (full penalty)
    let tokens_received = test_env.vault.emergency_redeem(&user, &user);

    // Should receive 90% (10% penalty)
    assert_eq!(tokens_received, 900 * SCALAR_7);
    assert_eq!(test_env.vault.total_shares(), 0);
    // Penalty stays in vault
    assert_eq!(test_env.vault.total_tokens(), 100 * SCALAR_7);
}

#[test]
fn test_emergency_redeem_partial_penalty() {
    let test_env = setup_vault();
    let user = test_env.users.get(0).unwrap();

    // Deposit and request redemption
    test_env.vault.deposit(&(1000 * SCALAR_7), &user, &user);
    test_env.vault.request_redeem(&(1000 * SCALAR_7), &user);

    // Advance halfway through lock period
    test_env.advance_time(DEFAULT_LOCK_TIME / 2);

    // Emergency redeem (half penalty = 5%)
    let tokens_received = test_env.vault.emergency_redeem(&user, &user);

    assert_eq!(tokens_received, 950 * SCALAR_7);
}

#[test]
fn test_cancel_redeem() {
    let test_env = setup_vault();
    let user = test_env.users.get(0).unwrap();

    // Deposit and request redemption
    let shares = test_env.vault.deposit(&(2000 * SCALAR_7), &user, &user);
    let redeem_shares = shares / 3;
    test_env.vault.request_redeem(&redeem_shares, &user);

    // Cancel redemption
    test_env.vault.cancel_redeem(&user);

    // All shares should be returned
    assert_eq!(test_env.share_balance(&user), shares);
    assert_eq!(test_env.share_token_client().balance(&test_env.vault.address), 0);
}

#[test]
fn test_multiple_users_deposits_and_redemptions() {
    let test_env = setup_vault();
    let user1 = test_env.users.get(0).unwrap();
    let user2 = test_env.users.get(1).unwrap();

    // Both users deposit
    test_env.vault.deposit(&(3000 * SCALAR_7), &user1, &user1);
    test_env.vault.deposit(&(2000 * SCALAR_7), &user2, &user2);

    // User1 requests redemption
    test_env.vault.request_redeem(&(1500 * SCALAR_7), &user1);

    // User2 can still deposit
    test_env.vault.deposit(&(1000 * SCALAR_7), &user2, &user2);

    // Advance time and user1 redeems
    test_env.advance_past_lock();
    let user1_tokens = test_env.vault.redeem(&user1, &user1);

    assert_eq!(user1_tokens, 1500 * SCALAR_7);
    assert_eq!(test_env.vault.total_shares(), 4500 * SCALAR_7);
    assert_eq!(test_env.vault.total_tokens(), 4500 * SCALAR_7);
}

#[test]
fn test_share_price_with_strategy_profits() {
    let test_env = setup_vault();
    let user1 = test_env.users.get(0).unwrap();
    let user2 = test_env.users.get(1).unwrap();
    let strategy = test_env.strategies.get(0).unwrap();

    // Initial deposit
    test_env.vault.deposit(&(5000 * SCALAR_7), &user1, &user1);

    // Strategy borrows and makes profit
    test_env.vault.transfer_to(&strategy, &(2000 * SCALAR_7));

    // Strategy returns with 25% profit
    test_env.mint_tokens(&strategy, 2500 * SCALAR_7);
    test_env.vault.transfer_from(&strategy, &(2500 * SCALAR_7));

    // Vault now has 5500 tokens for 5000 shares (1.1 price)
    assert_eq!(test_env.vault.total_tokens(), 5500 * SCALAR_7);

    // New deposit should reflect new price
    let deposit2 = 1100 * SCALAR_7;
    let shares2 = test_env.vault.deposit(&deposit2, &user2, &user2);

    assert_eq!(shares2, 1000 * SCALAR_7); // 1100 / 1.1 = 1000
}

#[test]
fn test_deposit_withdraw_full_cycle() {
    let test_env = setup_vault();
    let user = test_env.users.get(0).unwrap();

    let initial_balance = test_env.token_balance(&user);

    // Deposit
    let deposit = 1500 * SCALAR_7;
    let shares = test_env.vault.deposit(&deposit, &user, &user);

    // Request full redemption
    test_env.vault.request_redeem(&shares, &user);

    // Wait and redeem
    test_env.advance_past_lock();
    let withdrawn = test_env.vault.redeem(&user, &user);

    // Should get back what was put in
    assert_eq!(withdrawn, deposit);
    assert_eq!(test_env.token_balance(&user), initial_balance);
    assert_eq!(test_env.vault.total_shares(), 0);
    assert_eq!(test_env.vault.total_tokens(), 0);
}

#[test]
fn test_small_deposit_amounts() {
    let test_env = setup_vault();
    let user = test_env.users.get(0).unwrap();

    // Test with 1 stroop
    let tiny_deposit = 1;
    let shares = test_env.vault.deposit(&tiny_deposit, &user, &user);

    assert_eq!(shares, tiny_deposit);
    assert_eq!(test_env.vault.total_shares(), tiny_deposit);
}

#[test]
fn test_large_deposit_amounts() {
    let test_env = setup_vault();
    let user = test_env.users.get(0).unwrap();

    // Fund user with more tokens
    test_env.mint_tokens(&user, 900_000 * SCALAR_7);

    // Large deposit
    let large_deposit = 900_000 * SCALAR_7;
    let shares = test_env.vault.deposit(&large_deposit, &user, &user);

    assert_eq!(shares, large_deposit);
    assert_eq!(test_env.vault.total_shares(), large_deposit);
}

#[test]
#[should_panic(expected = "Error(Contract, #4041)")] // InvalidAmount
fn test_zero_deposit_fails() {
    let test_env = setup_vault();
    let user = test_env.users.get(0).unwrap();

    test_env.vault.deposit(&0, &user, &user);
}

#[test]
#[should_panic(expected = "Error(Contract, #4041)")] // InvalidAmount
fn test_zero_mint_fails() {
    let test_env = setup_vault();
    let user = test_env.users.get(0).unwrap();

    test_env.vault.mint(&0, &user, &user);
}

#[test]
#[should_panic(expected = "Error(Contract, #4041)")] // InvalidAmount
fn test_zero_redeem_request_fails() {
    let test_env = setup_vault();
    let user = test_env.users.get(0).unwrap();

    test_env.vault.request_redeem(&0, &user);
}

#[test]
#[should_panic(expected = "Error(Contract, #4043)")] // RedemptionInProgress
fn test_double_redeem_request_fails() {
    let test_env = setup_vault();
    let user = test_env.users.get(0).unwrap();

    test_env.vault.deposit(&(1000 * SCALAR_7), &user, &user);
    test_env.vault.request_redeem(&(500 * SCALAR_7), &user);

    // Second request should fail
    test_env.vault.request_redeem(&(300 * SCALAR_7), &user);
}

#[test]
#[should_panic(expected = "Error(Contract, #4044)")] // RedemptionLocked
fn test_early_redeem_fails() {
    let test_env = setup_vault();
    let user = test_env.users.get(0).unwrap();

    test_env.vault.deposit(&(1000 * SCALAR_7), &user, &user);
    test_env.vault.request_redeem(&(1000 * SCALAR_7), &user);

    // Try to redeem immediately
    test_env.vault.redeem(&user, &user);
}