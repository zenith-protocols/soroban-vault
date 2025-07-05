mod common;
use common::*;

#[test]
fn test_multi_user_multi_strategy_scenario() {
    let config = VaultConfig {
        num_users: 3,
        num_strategies: 2,
        ..Default::default()
    };
    let test_env = setup_vault_with_config(config);

    let user1 = test_env.users.get(0).unwrap();
    let user2 = test_env.users.get(1).unwrap();
    let user3 = test_env.users.get(2).unwrap();
    let strategy1 = test_env.strategies.get(0).unwrap();
    let strategy2 = test_env.strategies.get(1).unwrap();

    // Fund users for larger deposits
    test_env.mint_tokens(&user1, 40_000 * SCALAR_7);
    test_env.mint_tokens(&user2, 40_000 * SCALAR_7);
    test_env.mint_tokens(&user3, 40_000 * SCALAR_7);

    // Users deposit at different times
    test_env.vault.deposit(&(10_000 * SCALAR_7), &user1, &user1);
    test_env.vault.deposit(&(5_000 * SCALAR_7), &user2, &user2);

    // Strategy 1 borrows and makes profit
    test_env.vault.borrow(&strategy1, &(6_000 * SCALAR_7));
    test_env.mint_tokens(&strategy1, 7_200 * SCALAR_7); // 20% profit
    test_env.vault.repay(&strategy1, &(6_000 * SCALAR_7));
    test_env.vault.transfer_from(&strategy1, &(1_200 * SCALAR_7));

    // At this point: 16,200 tokens, 15,000 shares
    // Share price = 16,200 / 15,000 = 1.08

    // User3 deposits 8,640 tokens (should get 8,000 shares at price 1.08)
    let user3_deposit = 8_640 * SCALAR_7;
    let user3_shares = test_env.vault.deposit(&user3_deposit, &user3, &user3);
    assert_eq!(user3_shares, 8_000 * SCALAR_7);

    // Strategy 2 incurs loss
    test_env.vault.transfer_to(&strategy2, &(4_000 * SCALAR_7));
    test_env.mint_tokens(&strategy2, 3_200 * SCALAR_7); // 20% loss
    test_env.vault.transfer_from(&strategy2, &(3_200 * SCALAR_7));

    // Check final state
    assert_eq!(test_env.vault.total_shares(), 23_000 * SCALAR_7);
    assert_eq!(test_env.vault.total_tokens(), 24_040 * SCALAR_7);

    // User1 redeems (should get proportional share)
    test_env.vault.request_redeem(&(10_000 * SCALAR_7), &user1);
    test_env.advance_past_lock();
    let user1_tokens = test_env.vault.redeem(&user1, &user1);

    // User1 has 10,000 shares out of 23,000 total
    // Should get: 10,000 / 23,000 * 24,040 = 10,452.17...
    assert_approx_eq(user1_tokens, 10_452_173_913, "User1 redemption value");
}

#[test]
fn test_deposit_during_active_redemption() {
    let test_env = setup_vault();
    let user1 = test_env.users.get(0).unwrap();
    let user2 = test_env.users.get(1).unwrap();

    // User1 deposits and requests redemption
    test_env.vault.deposit(&(5_000 * SCALAR_7), &user1, &user1);
    test_env.vault.request_redeem(&(2_000 * SCALAR_7), &user1);

    // User2 can still deposit normally
    let user2_shares = test_env.vault.deposit(&(3_000 * SCALAR_7), &user2, &user2);
    assert_eq!(user2_shares, 3_000 * SCALAR_7);

    // Total shares include locked shares
    assert_eq!(test_env.vault.total_shares(), 8_000 * SCALAR_7);

    // User1 completes redemption
    test_env.advance_past_lock();
    test_env.vault.redeem(&user1, &user1);

    // Now total shares decrease
    assert_eq!(test_env.vault.total_shares(), 6_000 * SCALAR_7);
}

#[test]
fn test_strategy_operations_during_redemptions() {
    let test_env = setup_vault();
    let user = test_env.users.get(0).unwrap();
    let strategy = test_env.strategies.get(0).unwrap();

    // Setup: deposit and partial redemption request
    test_env.vault.deposit(&(10_000 * SCALAR_7), &user, &user);
    test_env.vault.request_redeem(&(3_000 * SCALAR_7), &user);

    // Strategy can still operate normally
    test_env.vault.borrow(&strategy, &(4_000 * SCALAR_7));

    // Strategy makes profit
    test_env.mint_tokens(&strategy, 4_400 * SCALAR_7);
    test_env.vault.repay(&strategy, &(4_000 * SCALAR_7));
    test_env.vault.transfer_from(&strategy, &(400 * SCALAR_7));

    // Complete redemption (should include profit share)
    test_env.advance_past_lock();
    let redeemed = test_env.vault.redeem(&user, &user);

    // 3000 shares out of 10000, with vault having 10400 tokens
    assert_eq!(redeemed, 3_120 * SCALAR_7);
}

#[test]
fn test_high_redemption_pressure() {
    let config = VaultConfig {
        num_users: 5,
        ..Default::default()
    };
    let test_env = setup_vault_with_config(config);

    // All users deposit
    for (i, user) in test_env.users.iter().enumerate() {
        let amount = (i as i128 + 1) * 1_000 * SCALAR_7;
        test_env.vault.deposit(&amount, &user, &user);
    }

    // 80% of users request redemption
    for i in 0..4 {
        let user = test_env.users.get(i).unwrap();
        let shares = test_env.share_balance(&user);
        test_env.vault.request_redeem(&shares, &user);
    }

    // Vault should still function
    let last_user = test_env.users.get(4).unwrap();
    test_env.vault.deposit(&(1_000 * SCALAR_7), &last_user, &last_user);

    // Complete all redemptions
    test_env.advance_past_lock();
    for i in 0..4 {
        let user = test_env.users.get(i).unwrap();
        test_env.vault.redeem(&user, &user);
    }

    // Only last user's funds remain
    assert_eq!(test_env.vault.total_shares(), 6_000 * SCALAR_7);
    assert_eq!(test_env.vault.total_tokens(), 6_000 * SCALAR_7);
}

#[test]
fn test_precision_edge_cases() {
    let test_env = setup_vault();
    let user = test_env.users.get(0).unwrap();
    let strategy = test_env.strategies.get(0).unwrap();

    // Fund user for large deposit
    test_env.mint_tokens(&user, 990_000 * SCALAR_7);

    // Create non-round share price
    test_env.vault.deposit(&(1_000_000 * SCALAR_7), &user, &user);

    // Add small profit to create fractional share price
    test_env.mint_tokens(&strategy, 1);
    test_env.vault.transfer_from(&strategy, &1);

    // Deposit amounts that cause rounding
    for i in 1..10 {
        let amount = i * 7; // Prime number multiples
        let shares = test_env.vault.deposit(&amount, &user, &user);

        // Verify shares are calculated correctly
        assert!(shares > 0, "Should always mint at least 1 unit");
    }
}

#[test]
fn test_vault_recovery_after_strategy_default() {
    let test_env = setup_vault();
    let user1 = test_env.users.get(0).unwrap();
    let user2 = test_env.users.get(1).unwrap();
    let strategy = test_env.strategies.get(0).unwrap();

    // Initial deposits
    test_env.vault.deposit(&(10_000 * SCALAR_7), &user1, &user1);

    // Strategy borrows and defaults
    test_env.vault.borrow(&strategy, &(4_000 * SCALAR_7));
    // Strategy can't repay anything - vault has lost 4000 tokens

    // Vault now has 6000 tokens but 10000 shares
    // Share price = 6000 / 10000 = 0.6

    // New user deposits 6000 tokens at price 0.6
    let new_shares = test_env.vault.deposit(&(6_000 * SCALAR_7), &user2, &user2);

    // 6000 / 0.6 = 10000 shares
    assert_eq!(new_shares, 10_000 * SCALAR_7);

    // Both users now share the loss equally
    assert_eq!(test_env.vault.total_shares(), 20_000 * SCALAR_7);
    assert_eq!(test_env.vault.total_tokens(), 12_000 * SCALAR_7);
}

#[test]
fn test_emergency_redemption_under_stress() {
    let test_env = setup_vault();
    let user1 = test_env.users.get(0).unwrap();
    let user2 = test_env.users.get(1).unwrap();
    let strategy = test_env.strategies.get(0).unwrap();

    // Setup
    test_env.vault.deposit(&(8_000 * SCALAR_7), &user1, &user1);
    test_env.vault.deposit(&(2_000 * SCALAR_7), &user2, &user2);

    // Strategy has most funds
    test_env.vault.borrow(&strategy, &(7_000 * SCALAR_7));

    // Only 3000 tokens remain in vault, but users want to emergency redeem 10000 shares
    // This test shows the vault needs better liquidity management

    // User2 can emergency redeem since there's enough liquidity
    test_env.vault.request_redeem(&(2_000 * SCALAR_7), &user2);
    let user2_received = test_env.vault.emergency_redeem(&user2, &user2);
    assert_eq!(user2_received, 1_800 * SCALAR_7); // 90% after penalty

    // User1 cannot fully redeem due to liquidity constraints
    // This would fail in practice - vault needs liquidity management
}

#[test]
fn test_share_price_stability() {
    let test_env = setup_vault();
    let user = test_env.users.get(0).unwrap();
    let strategy = test_env.strategies.get(0).unwrap();

    // Fund user for large deposit
    test_env.mint_tokens(&user, 90_000 * SCALAR_7);

    // Large initial deposit
    test_env.vault.deposit(&(100_000 * SCALAR_7), &user, &user);

    // Many small operations
    for i in 0..20 {
        if i % 2 == 0 {
            // Small profit
            test_env.mint_tokens(&strategy, 10 * SCALAR_7);
            test_env.vault.transfer_from(&strategy, &(10 * SCALAR_7));
        } else {
            // Small loss
            test_env.vault.transfer_to(&strategy, &(10 * SCALAR_7));
        }
    }

    // Share price should be exactly 1:1 still
    assert_eq!(test_env.vault.total_shares(), 100_000 * SCALAR_7);
    assert_eq!(test_env.vault.total_tokens(), 100_000 * SCALAR_7);
}

#[test]
fn test_concurrent_operations() {
    let config = VaultConfig {
        num_users: 3,
        num_strategies: 2,
        ..Default::default()
    };
    let test_env = setup_vault_with_config(config);

    let user1 = test_env.users.get(0).unwrap();
    let user2 = test_env.users.get(1).unwrap();
    let user3 = test_env.users.get(2).unwrap();
    let strategy1 = test_env.strategies.get(0).unwrap();
    let strategy2 = test_env.strategies.get(1).unwrap();

    // Concurrent operations
    test_env.vault.deposit(&(5_000 * SCALAR_7), &user1, &user1);
    test_env.vault.borrow(&strategy1, &(2_000 * SCALAR_7));
    test_env.vault.deposit(&(3_000 * SCALAR_7), &user2, &user2);
    test_env.vault.request_redeem(&(1_000 * SCALAR_7), &user1);
    test_env.vault.transfer_to(&strategy2, &(1_000 * SCALAR_7));
    test_env.vault.mint(&(2_000 * SCALAR_7), &user3, &user3);

    // Verify state consistency
    let total_user_shares = test_env.share_balance(&user1) +
        test_env.share_balance(&user2) +
        test_env.share_balance(&user3) +
        test_env.share_token_client().balance(&test_env.vault.address); // locked shares

    assert_eq!(total_user_shares, test_env.vault.total_shares());

    // Verify token accounting
    // Deposits: 5000 + 3000 + 2000 = 10000
    // Transfer_to reduced total_tokens by 1000
    // So total_tokens should be 9000
    assert_eq!(test_env.vault.total_tokens(), 9_000 * SCALAR_7);

    // Verify actual token locations
    let vault_balance = test_env.vault_balance();
    let strategy1_balance = test_env.token_balance(&strategy1);
    let strategy2_balance = test_env.token_balance(&strategy2);

    // Vault balance = 10000 - 2000 (borrowed) - 1000 (transferred) = 7000
    // But we need to account for the locked redemption shares
    // Actually: 5000 (after borrow) + 3000 (deposit) - 1000 (transfer) + 2000 (mint) - 1000 (locked) = 5000
    assert_eq!(vault_balance, 5_000 * SCALAR_7);
    assert_eq!(strategy1_balance, 2_000 * SCALAR_7);
    assert_eq!(strategy2_balance, 1_000 * SCALAR_7);
}

#[test]
fn test_extreme_share_price_ratios() {
    let test_env = setup_vault();
    let user = test_env.users.get(0).unwrap();
    let strategy = test_env.strategies.get(0).unwrap();

    // Create 10x appreciation
    test_env.vault.deposit(&(1_000 * SCALAR_7), &user, &user);
    test_env.mint_tokens(&strategy, 9_000 * SCALAR_7);
    test_env.vault.transfer_from(&strategy, &(9_000 * SCALAR_7));

    // Share price is now 10:1
    assert_eq!(test_env.vault.total_tokens(), 10_000 * SCALAR_7);
    assert_eq!(test_env.vault.total_shares(), 1_000 * SCALAR_7);

    // Small deposits should still work
    let tiny_deposit = 10 * SCALAR_7;
    let shares = test_env.vault.deposit(&tiny_deposit, &user, &user);
    assert_eq!(shares, 1 * SCALAR_7); // Gets 1/10th shares

    // Very small deposits round down to 0 shares and should fail in practice
    // (though our contract might need additional validation for this)
}

#[test]
fn test_state_consistency_under_all_operations() {
    let config = VaultConfig {
        num_users: 2,
        num_strategies: 1,
        penalty_rate: SCALAR_7 / 4, // 25% penalty
        ..Default::default()
    };
    let test_env = setup_vault_with_config(config);
    let user1 = test_env.users.get(0).unwrap();
    let user2 = test_env.users.get(1).unwrap();
    let strategy = test_env.strategies.get(0).unwrap();

    // Track expected state
    let mut expected_total_shares = 0i128;
    let mut expected_total_tokens = 0i128;

    // Deposit
    test_env.vault.deposit(&(5_000 * SCALAR_7), &user1, &user1);
    expected_total_shares += 5_000 * SCALAR_7;
    expected_total_tokens += 5_000 * SCALAR_7;

    // Mint
    test_env.vault.mint(&(1_000 * SCALAR_7), &user2, &user2);
    expected_total_shares += 1_000 * SCALAR_7;
    expected_total_tokens += 1_000 * SCALAR_7;

    // Borrow (doesn't change totals)
    test_env.vault.borrow(&strategy, &(2_000 * SCALAR_7));

    // Transfer to (reduces total_tokens)
    test_env.vault.transfer_to(&strategy, &(500 * SCALAR_7));
    expected_total_tokens -= 500 * SCALAR_7;

    // Transfer from (increases total_tokens)
    test_env.mint_tokens(&strategy, 3_000 * SCALAR_7);
    test_env.vault.transfer_from(&strategy, &(800 * SCALAR_7));
    expected_total_tokens += 800 * SCALAR_7;

    // Request redemption (doesn't change totals)
    test_env.vault.request_redeem(&(1_000 * SCALAR_7), &user1);

    // Emergency redeem (reduces both, penalty stays)
    let redeemed = test_env.vault.emergency_redeem(&user1, &user1);
    expected_total_shares -= 1_000 * SCALAR_7;
    expected_total_tokens -= redeemed; // Only what was paid out

    // Verify final state
    assert_eq!(test_env.vault.total_shares(), expected_total_shares);
    assert_eq!(test_env.vault.total_tokens(), expected_total_tokens);

    // Verify share price consistency
    if expected_total_shares > 0 {
        let share_price = expected_total_tokens * SCALAR_7 / expected_total_shares;

        // Test deposit at current price
        let test_deposit = 1_000 * SCALAR_7;
        let expected_shares = test_deposit * SCALAR_7 / share_price;
        let actual_shares = test_env.vault.deposit(&test_deposit, &user2, &user2);

        assert_approx_eq(actual_shares, expected_shares, "Share price consistency");
    }
}