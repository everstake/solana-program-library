//! Instruction types

#![allow(clippy::too_many_arguments)]
use {
    crate::{
        find_deposit_authority_program_address, find_stake_program_address,
        find_transient_stake_program_address, find_withdraw_authority_program_address,
        state::{Fee, FeeType, StakePool, ValidatorList},
        MAX_VALIDATORS_TO_UPDATE,
    },
    borsh::{BorshDeserialize, BorshSchema, BorshSerialize},
    solana_program::{
        instruction::{AccountMeta, Instruction},
        pubkey::Pubkey,
        stake, system_program, sysvar,
    },
};

/// Defines which validator vote account is set during the
/// `SetPreferredValidator` instruction
#[repr(C)]
#[derive(Clone, Debug, PartialEq, BorshSerialize, BorshDeserialize, BorshSchema)]
pub enum PreferredValidatorType {
    /// Set preferred validator for deposits
    Deposit,
    /// Set preferred validator for withdraws
    Withdraw,
}

/// Defines which authority to update in the `SetFundingAuthority`
/// instruction
#[repr(C)]
#[derive(Clone, Debug, PartialEq, BorshSerialize, BorshDeserialize, BorshSchema)]
pub enum FundingType {
    /// Sets the stake deposit authority
    StakeDeposit,
    /// Sets the SOL deposit authority
    SolDeposit,
    /// Sets the SOL withdraw authority
    SolWithdraw,
}

/// Instructions supported by the StakePool program.
#[repr(C)]
#[derive(Clone, Debug, PartialEq, BorshSerialize, BorshDeserialize, BorshSchema)]
pub enum StakePoolInstruction {
    ///   Initializes a new StakePool.
    ///
    ///   0. `[w]` New StakePool to create.
    ///   1. `[s]` Manager
    ///   2. `[]` Staker
    ///   3. `[]` Stake pool withdraw authority
    ///   4. `[w]` Uninitialized validator stake list storage account
    ///   5. `[]` Reserve stake account must be initialized, have zero balance,
    ///       and staker / withdrawer authority set to pool withdraw authority.
    ///   6. `[]` Pool token mint. Must have zero supply, owned by withdraw authority.
    ///   7. `[]` Pool account to deposit the generated fee for manager.
    ///   8. `[]` Account for Treasury
    ///   9. `[]` Account for validator fee
    ///   10. `[]` Token program id
    ///   11. `[]` (Optional) Deposit authority that must sign all deposits.
    ///      Defaults to the program address generated using
    ///      `find_deposit_authority_program_address`, making deposits permissionless.
    Initialize {
        /// Fee assessed as percentage of perceived rewards
        #[allow(dead_code)] // but it's not
        fee: Fee,
        /// Fee charged per withdrawal as percentage of withdrawal
        #[allow(dead_code)] // but it's not
        withdrawal_fee: Fee,
        /// Fee charged per deposit as percentage of deposit
        #[allow(dead_code)] // but it's not
        deposit_fee: Fee,
        /// Fee assessed on taking rewards for treasury
        #[allow(dead_code)] // but it's not
        treasury_fee: Fee,
        /// Percentage [0-100] of deposit_fee that goes to referrer
        #[allow(dead_code)] // but it's not
        referral_fee: u8,
        /// Maximum expected number of validators
        #[allow(dead_code)] // but it's not
        max_validators: u32,
        /// No fee deposit threshold
        #[allow(dead_code)] // but it's not
        no_fee_deposit_threshold: u16,
    },

    ///   (Staker only) Adds stake account delegated to validator to the pool's
    ///   list of managed validators.
    ///
    ///   The stake account will have the rent-exempt amount plus
    ///   `crate::MINIMUM_ACTIVE_STAKE` (currently 0.001 SOL).
    ///
    ///   0. `[w]` Stake pool
    ///   1. `[s]` Staker
    ///   2. `[ws]` Funding account (must be a system account)
    ///   3. `[]` Stake pool withdraw authority
    ///   4. `[w]` Validator stake list storage account
    ///   5. `[w]` Stake account to add to the pool
    ///   6. `[]` Validator this stake account will be delegated to
    ///   7. `[]` Rent sysvar
    ///   8. `[]` Clock sysvar
    ///   9. '[]' Stake history sysvar
    ///  10. '[]' Stake config sysvar
    ///  11. `[]` System program
    ///  12. `[]` Stake program
    AddValidatorToPool,

    ///   (Staker only) Removes validator from the pool
    ///
    ///   Only succeeds if the validator stake account has the minimum of
    ///   `crate::MINIMUM_ACTIVE_STAKE` (currently 0.001 SOL) plus the rent-exempt
    ///   amount.
    ///
    ///   0. `[w]` Stake pool
    ///   1. `[s]` Staker
    ///   2. `[]` Stake pool withdraw authority
    ///   3. `[]` New withdraw/staker authority to set in the stake account
    ///   4. `[w]` Validator stake list storage account
    ///   5. `[w]` Stake account to remove from the pool
    ///   6. `[]` Transient stake account, to check that that we're not trying to activate
    ///   7. `[w]` Destination stake account, to receive the minimum SOL from the validator stake account
    ///   8. `[]` Sysvar clock
    ///   9. `[]` Stake program id,
    RemoveValidatorFromPool,

    /// (Staker only) Decrease active stake on a validator, eventually moving it to the reserve
    ///
    /// Internally, this instruction splits a validator stake account into its
    /// corresponding transient stake account and deactivates it.
    ///
    /// In order to rebalance the pool without taking custody, the staker needs
    /// a way of reducing the stake on a stake account. This instruction splits
    /// some amount of stake, up to the total activated stake, from the canonical
    /// validator stake account, into its "transient" stake account.
    ///
    /// The instruction only succeeds if the transient stake account does not
    /// exist. The amount of lamports to move must be at least rent-exemption
    /// plus 1 lamport.
    ///
    ///  0. `[]` Stake pool
    ///  1. `[s]` Stake pool staker
    ///  2. `[]` Stake pool withdraw authority
    ///  3. `[w]` Validator list
    ///  4. `[w]` Canonical stake account to split from
    ///  5. `[w]` Transient stake account to receive split
    ///  6. `[]` Clock sysvar
    ///  7. `[]` Rent sysvar
    ///  8. `[]` System program
    ///  9. `[]` Stake program
    DecreaseValidatorStake {
        /// amount of lamports to split into the transient stake account
        #[allow(dead_code)] // but it's not
        lamports: u64,
        /// seed used to create transient stake account
        #[allow(dead_code)] // but it's not
        transient_stake_seed: u64,
    },

    /// (Staker only) Increase stake on a validator from the reserve account
    ///
    /// Internally, this instruction splits reserve stake into a transient stake
    /// account and delegate to the appropriate validator. `UpdateValidatorListBalance`
    /// will do the work of merging once it's ready.
    ///
    /// This instruction only succeeds if the transient stake account does not exist.
    /// The minimum amount to move is rent-exemption plus `crate::MINIMUM_ACTIVE_STAKE`
    /// (currently 0.001 SOL) in order to avoid issues on credits observed when
    /// merging active stakes later.
    ///
    ///  0. `[]` Stake pool
    ///  1. `[s]` Stake pool staker
    ///  2. `[]` Stake pool withdraw authority
    ///  3. `[w]` Validator list
    ///  4. `[w]` Stake pool reserve stake
    ///  5. `[w]` Transient stake account
    ///  6. `[]` Validator vote account to delegate to
    ///  7. '[]' Clock sysvar
    ///  8. '[]' Rent sysvar
    ///  9. `[]` Stake History sysvar
    /// 10. `[]` Stake Config sysvar
    /// 11. `[]` System program
    /// 12. `[]` Stake program
    ///  userdata: amount of lamports to increase on the given validator.
    ///  The actual amount split into the transient stake account is:
    ///  `lamports + stake_rent_exemption`
    ///  The rent-exemption of the stake account is withdrawn back to the reserve
    ///  after it is merged.
    IncreaseValidatorStake {
        /// amount of lamports to increase on the given validator
        #[allow(dead_code)] // but it's not
        lamports: u64,
        /// seed used to create transient stake account
        #[allow(dead_code)] // but it's not
        transient_stake_seed: u64,
    },

    /// (Staker only) Set the preferred deposit or withdraw stake account for the
    /// stake pool
    ///
    /// In order to avoid users abusing the stake pool as a free conversion
    /// between SOL staked on different validators, the staker can force all
    /// deposits and/or withdraws to go to one chosen account, or unset that account.
    ///
    /// 0. `[w]` Stake pool
    /// 1. `[s]` Stake pool staker
    /// 2. `[]` Validator list
    ///
    /// Fails if the validator is not part of the stake pool.
    SetPreferredValidator {
        /// Affected operation (deposit or withdraw)
        #[allow(dead_code)] // but it's not
        validator_type: PreferredValidatorType,
        /// Validator vote account that deposits or withdraws must go through,
        /// unset with None
        #[allow(dead_code)] // but it's not
        validator_vote_address: Option<Pubkey>,
    },

    ///  Updates balances of validator and transient stake accounts in the pool
    ///
    ///  While going through the pairs of validator and transient stake accounts,
    ///  if the transient stake is inactive, it is merged into the reserve stake
    ///  account. If the transient stake is active and has matching credits
    ///  observed, it is merged into the canonical validator stake account. In
    ///  all other states, nothing is done, and the balance is simply added to
    ///  the canonical stake account balance.
    ///
    ///  0. `[]` Stake pool
    ///  1  `[s]` Manager
    ///  2. `[]` Stake pool withdraw authority
    ///  3. `[w]` Validator stake list storage account
    ///  4. `[w]` Reserve stake account
    ///  5. `[]` Sysvar clock
    ///  6. `[]` Sysvar stake history
    ///  7. `[]` Stake program
    ///  8. ..8+N ` [] N pairs of validator and transient stake accounts
    UpdateValidatorListBalance {
        /// Index to start updating on the validator list
        #[allow(dead_code)] // but it's not
        start_index: u32,
        /// If true, don't try merging transient stake accounts into the reserve or
        /// validator stake account.  Useful for testing or if a particular stake
        /// account is in a bad state, but we still want to update
        #[allow(dead_code)] // but it's not
        no_merge: bool,
    },

    ///   Updates total pool balance based on balances in the reserve and validator list
    ///
    ///   0. `[w]` Stake pool
    ///   1  `[s]` Manager
    ///   2. `[]` Stake pool withdraw authority
    ///   3. `[w]` Validator stake list storage account
    ///   4. `[]` Reserve stake account
    ///   5. `[w]` Account to receive pool Manager fee
    ///   6. `[w]` Pool mint account
    ///   7. `[w]` Account to receive treasury fee tokens
    ///   8. `[]` Pool token program
    /// 
    ///   userdata: max_validator_yield_per_epoch_numerator
    UpdateStakePoolBalance(u32),

    ///   Cleans up validator stake account entries marked as `ReadyForRemoval`
    ///
    ///   0. `[]` Stake pool
    ///   1. `[s]` Manager
    ///   2. `[w]` Validator stake list storage account
    CleanupRemovedValidatorEntries,

    ///   Deposit some stake into the pool.  The output is a "pool" token representing ownership
    ///   into the pool. Inputs are converted to the current ratio.
    ///
    ///   0. `[w]` Stake pool
    ///   1. `[w]` Validator stake list storage account
    ///   2. `[s]/[]` Stake pool deposit authority
    ///   3. `[]` Stake pool withdraw authority
    ///   4. `[w]` Stake account to join the pool (withdraw authority for the stake account should be first set to the stake pool deposit authority)
    ///   5. `[w]` Validator stake account for the stake account to be merged with
    ///   6. `[w]` Reserve stake account, to withdraw rent exempt reserve
    ///   7. `[w]` User account to receive pool tokens
    ///   8. `[w]` Account to receive pool fee tokens
    ///   9. `[w]` Account to receive a portion of pool fee tokens as referral fees
    ///   10. `[w]` Pool token mint account
    ///   11. '[]' Sysvar clock account
    ///   12. '[]' Sysvar stake history account
    ///   13. `[]` Pool token program id,
    ///   14. `[]` Stake program id,
    DepositStake,

    ///   Withdraw the token from the pool at the current ratio.
    ///
    ///   Succeeds if the stake account has enough SOL to cover the desired amount
    ///   of pool tokens, and if the withdrawal keeps the total staked amount
    ///   above the minimum of rent-exempt amount + 0.001 SOL.
    ///
    ///   When allowing withdrawals, the order of priority goes:
    ///
    ///   * preferred withdraw validator stake account (if set)
    ///   * validator stake accounts
    ///   * transient stake accounts
    ///   * reserve stake account
    ///
    ///   A user can freely withdraw from a validator stake account, and if they
    ///   are all at the minimum, then they can withdraw from transient stake
    ///   accounts, and if they are all at minimum, then they can withdraw from
    ///   the reserve.
    ///
    ///   0. `[w]` Stake pool
    ///   1. `[w]` Validator stake list storage account
    ///   2. `[]` Stake pool withdraw authority
    ///   3. `[w]` Validator or reserve stake account to split
    ///   4. `[w]` Unitialized stake account to receive withdrawal
    ///   5. `[]` User account to set as a new withdraw authority
    ///   6. `[s]` User transfer authority, for pool token account
    ///   7. `[w]` User account with pool tokens to burn from
    ///   8. `[w]` Account to receive pool fee tokens
    ///   9. `[w]` Pool token mint account
    ///  10. `[]` Sysvar clock account (required)
    ///  11. `[]` Pool token program id
    ///  12. `[]` Stake program id,
    ///  userdata: amount of pool tokens to withdraw
    WithdrawStake(u64),

    ///  (Manager only) Update manager
    ///
    ///  0. `[w]` StakePool
    ///  1. `[s]` Manager
    ///  2. `[s]` New manager
    ///  3. `[]` New manager fee account
    SetManager,

    ///  (Manager only) Update fee
    ///
    ///  0. `[w]` StakePool
    ///  1. `[s]` Manager
    SetFee {
        /// Type of fee to update and value to update it to
        #[allow(dead_code)] // but it's not
        fee: FeeType,
    },

    ///  (Manager or staker only) Update staker
    ///
    ///  0. `[w]` StakePool
    ///  1. `[s]` Manager or current staker
    ///  2. '[]` New staker pubkey
    SetStaker,

    ///   Deposit SOL directly into the pool's reserve account. The output is a "pool" token
    ///   representing ownership into the pool. Inputs are converted to the current ratio.
    ///
    ///   0. `[w]` Stake pool
    ///   1. `[]` Stake pool withdraw authority
    ///   2. `[w]` Reserve stake account, to deposit SOL
    ///   3. `[s]` Account providing the lamports to be deposited into the pool
    ///   4. `[w]` User account to receive pool tokens
    ///   5. `[w]` Account to receive fee tokens
    ///   6. `[w]` Account to receive a portion of fee as referral fees
    ///   7. `[w]` Pool token mint account
    ///   8. `[]` System program account
    ///   9. `[]` Token program id
    ///  10. `[s]` (Optional) Stake pool sol deposit authority.
    DepositSol(u64),

    ///  (Manager only) Update SOL deposit authority
    ///
    ///  0. `[w]` StakePool
    ///  1. `[s]` Manager
    ///  2. '[]` New authority pubkey or none
    SetFundingAuthority(FundingType),

    ///   Withdraw SOL directly from the pool's reserve account. Fails if the
    ///   reserve does not have enough SOL.
    ///
    ///   0. `[w]` Stake pool
    ///   1. `[]` Stake pool withdraw authority
    ///   2. `[s]` User transfer authority, for pool token account
    ///   3. `[w]` User account to burn pool tokens
    ///   4. `[w]` Reserve stake account, to withdraw SOL
    ///   5. `[w]` Account receiving the lamports from the reserve, must be a system account
    ///   6. `[w]` Account to receive pool fee tokens
    ///   7. `[w]` Pool token mint account
    ///   8. '[]' Clock sysvar
    ///   9. '[]' Stake history sysvar
    ///  10. `[]` Stake program account
    ///  11. `[]` Token program id
    ///  12. `[s]` (Optional) Stake pool sol withdraw authority
    WithdrawSol(u64),
    
    ///   Deposit SOL directly into the pool's reserve account to increase liquidity
    ///
    ///   0. `[w]` Stake pool
    ///   1. `[s]` Manager
    ///   2. `[]` Stake pool withdraw authority
    ///   3. `[w]` Reserve stake account, to deposit SOL
    ///   4. `[ws]` Account providing the lamports to be deposited into the pool
    ///   5. `[]` System program account
    ///   6. `[s]` (Optional) Stake pool sol deposit authority.
    DepositLiquiditySol(u64),

    ///   Withdraw SOL directly from the pool's reserve account to decrease liquidity
    ///
    ///   0. `[w]` Stake pool
    ///   1. `[s]` Manager
    ///   2. `[]` Stake pool withdraw authority
    ///   3. `[w]` Reserve stake account, to withdraw  SOL
    ///   4. `[w]` Account receiving the lamports from the reserve
    ///   5. '[]' Clock sysvar
    ///   6. '[]' Stake history sysvar
    ///   7. `[]` Stake program account
    ///   8. `[s]` (Optional) Stake pool sol withdraw authority
    WithdrawLiquiditySol(u64),

    ///   Create account for storing DAO`s Community token`s mint
    ///   0. `[]` Stake pool
    ///   1. `[s]` Manager
    ///   2. `[w]` Account storing community token dto
    ///   3. `[w]` Account storing dao state dto
    ///   4.  `[]` Rent sysvar
    ///   5.  `[]` System program account
    CreateCommunityToken {
        ///   Community token`s mint adress
        #[allow(dead_code)] // but it's not
        token_mint: Pubkey,
    },

    ///   Create account for storing DAO`s state
    ///   0. `[]` Stake pool
    ///   1. `[s]` Manager
    ///   2. `[w]` Account storing dao state dto
    ///   3. `[]` Rent sysvar
    ///   4. `[]` System program account
    CreateDaoState {
        /// Is DAO enabled for StakePool
        #[allow(dead_code)] // but it's not
        is_enabled: bool,
    },

    ///   Create account for storing information for DAO`s community tokens destribution strategy
    ///   0. `[]` Stake pool
    ///   1. `[s]` Owner wallet
    ///   2. `[w]` Account storing community token staking rewards dto
    ///   3. `[w]` account for storing counter for community token staking rewards accounts
    ///   4. `[]` Rent sysvar
    ///   5. `[]` System program account
    CreateCommunityTokenStakingRewards,

    ///   Deposit SOL directly into the pool's reserve account with existing DAO`s community tokens strategy. The output is a "pool" token
    ///   representing ownership into the pool. Inputs are converted to the current ratio.
    ///
    ///   0. `[w]` Stake pool
    ///   1. `[]` Stake pool withdraw authority
    ///   2. `[w]` Reserve stake account, to deposit SOL
    ///   3. `[s]` Account providing the lamports to be deposited into the pool
    ///   4. `[w]` User account to receive pool tokens
    ///   5  `[]` User account to hold DAO`s community tokens
    ///   6. `[w]` Account to receive fee tokens
    ///   7. `[w]` Account to receive a portion of fee as referral fees
    ///   8. `[w]` Pool token mint account
    ///   9. `[]` System program account
    ///  10. `[]` Token program id
    ///  11. `[w]` Account for storing community token staking rewards dto
    ///  12. `[s]` Owner wallet
    ///  13. `[]` Account for storing community token dto
    ///  14. `[s]` (Optional) Stake pool sol deposit authority.
    DaoStrategyDepositSol(u64),

    ///   Withdraw SOL directly from the pool's reserve account with existing DAO`s community tokens strategy. Fails if the
    ///   reserve does not have enough SOL.
    ///
    ///   0. `[w]` Stake pool
    ///   1. `[]` Stake pool withdraw authority
    ///   2. `[s]` User transfer authority, for pool token account
    ///   3. `[w]` User account to burn pool tokens
    ///   4  `[]` User account to hold DAO`s community tokens
    ///   5. `[w]` Reserve stake account, to withdraw SOL
    ///   6. `[w]` Account receiving the lamports from the reserve, must be a system account
    ///   7. `[w]` Account to receive pool fee tokens
    ///   8. `[w]` Pool token mint account
    ///   9. '[]' Clock sysvar
    ///  10. '[]' Stake history sysvar
    ///  11. `[]` Stake program account
    ///  12. `[]` Token program id
    ///  13. `[w]` Account for storing community token staking rewards dto
    ///  14. `[s]` Owner wallet 
    ///  15. `[]` Account for storing community token
    ///  16. `[s]` (Optional) Stake pool sol withdraw authority
    DaoStrategyWithdrawSol(u64),

    ///   Withdraw the token from the pool at the current ratio  with existing DAO`s community tokens strategy.
    ///
    ///   Succeeds if the stake account has enough SOL to cover the desired amount
    ///   of pool tokens, and if the withdrawal keeps the total staked amount
    ///   above the minimum of rent-exempt amount + 0.001 SOL.
    ///
    ///   When allowing withdrawals, the order of priority goes:
    ///
    ///   * preferred withdraw validator stake account (if set)
    ///   * validator stake accounts
    ///   * transient stake accounts
    ///   * reserve stake account
    ///
    ///   A user can freely withdraw from a validator stake account, and if they
    ///   are all at the minimum, then they can withdraw from transient stake
    ///   accounts, and if they are all at minimum, then they can withdraw from
    ///   the reserve.
    ///
    ///   0. `[w]` Stake pool
    ///   1. `[w]` Validator stake list storage account
    ///   2. `[]` Stake pool withdraw authority
    ///   3. `[w]` Validator or reserve stake account to split
    ///   4. `[w]` Unitialized stake account to receive withdrawal
    ///   5. `[]` User account to set as a new withdraw authority
    ///   6. `[s]` User transfer authority, for pool token account
    ///   7. `[w]` User account with pool tokens to burn from
    ///   8. `[w]` Account to receive pool fee tokens
    ///   9. `[w]` Pool token mint account
    ///  10. `[]` Sysvar clock account (required)
    ///  11. `[]` Pool token program id
    ///  12. `[]` Stake program id,
    ///  13. `[]` User account to hold DAO`s community tokens
    ///  14. `[w]` Account for storing community token staking rewards dto
    ///  15. `[s]` Owner wallet
    ///  16. `[]` Account for storing community token dto
    /// 
    ///  userdata: amount of pool tokens to withdraw
    DaoStrategyWithdrawStake(u64),

    ///   Deposit some stake into the pool with existing DAO`s community tokens strategy. The output is a "pool" token representing ownership
    ///   into the pool. Inputs are converted to the current ratio.
    ///
    ///   0. `[w]` Stake pool
    ///   1. `[w]` Validator stake list storage account
    ///   2. `[]` Stake pool withdraw authority
    ///   3. `[w]` Stake account to join the pool (withdraw authority for the stake account should be first set to the stake pool deposit authority)
    ///   4. `[w]` Destination stake account for the stake account to be merged with. It can be Validator stake account or Reserve stake account.
    ///   5. `[w]` Reserve stake account, to withdraw rent exempt reserve
    ///   6. `[w]` User account to receive pool tokens
    ///   7. `[w]` Account to receive pool fee tokens
    ///   8. `[w]` Account to receive a portion of pool fee tokens as referral fees
    ///  9. `[w]` Pool token mint account
    ///  10. '[]' Sysvar clock account
    ///  11. '[]' Sysvar stake history account
    ///  12. `[]` Pool token program id,
    ///  13. `[]` Stake program id,
    ///  14. `[]` User account to hold DAO`s community tokens
    ///  15. `[w]` Account for storing community token staking rewards dto
    ///  16. `[s]` Owner wallet
    ///  17. `[]` Account for storing community token dto
    ///  18. `[s]` (Optional) Stake pool deposit authority
    DaoStrategyDepositStake,

    ///   Create account for storing counter for community token staking rewards accounts
    ///   0. `[]` Stake pool
    ///   1. `[s]` Manager
    ///   2. `[w]` Account for storing counter for community token staking rewards accounts
    ///   3. `[]`  Account for storing community token dto
    ///   4. `[]` Rent sysvar
    ///   5. `[]` System program account
    CreateCommunityTokenStakingRewardsCounter,

    ///   Mints community tokens
    ///   Currenty, we mint tokens for EVS DAO Reserve, which contains up to 75% of community tokens max supply
    ///   The strategy for EVS Strategic Reserve, which contains up to 25% of community tokens max supply, is not implemented yet
    ///   The community tokens counter and its limits are implemented using CommunityTokensCounter structure
    /// 
    ///   0. `[]` Stake pool
    ///   1. `[s]` Manager
    ///   2. `[]` User wallet
    ///   3. `[]` Stake pool withdraw authority
    ///   4. `[w]` User account to receive pool tokens
    ///   5. `[w]` Community token mint account
    ///   6. `[]`  Account for storing community token dto
    ///   7. `[w]` Account for storing community tokens counter
    ///   8. `[w]` Account for storing community token staking rewards dto
    ///   9. `[]` System program account
    ///  10. `[]` Token program id
    ///  11. `[]` Sysvar clock account (required)
    MintCommunityToken {
        /// Community tokens amount
        #[allow(dead_code)] // but it's not
        amount: u64,
        /// Current epoch
        #[allow(dead_code)] // but it's not
        current_epoch: u64
    },

    ///   Create account for community tokens counter
    ///   It comprises of two separate counters: EVS DAO Reserve and EVS Strategic Reserve
    ///   EVS DAO reserve can hold up to 75% of max tokens supply
    ///   EVS Strategic reserve can hold up to 25% of max tokens supply
    /// 
    ///   0. `[]` Stake pool
    ///   1. `[s]` Manager
    ///   2. `[w]` Account for community tokens counter
    ///   3. `[]` Community token dto account
    ///   4. `[]` Rent sysvar
    ///   5. `[]` System program account   
    CreateCommunityTokensCounter,

    ///   Delete account for storing information for DAO`s community tokens destribution strategy
    ///   0. `[]` Stake pool
    ///   1. `[s]` Manager 
    ///   2. '[]' User wallet
    ///   3. `[w]` Account storing community token staking rewards dto
    ///   4. '[]' Pool mint
    ///   5  '[]' User account for storing pool tokens
    ///   6. `[]` System program account
    DeleteCommunityTokenStakingRewards,

    ///   Merge inactive stake accounts with the pool's reserve account
    ///   0. `[w]` Stake pool
    ///   1. `[s]` Manager
    ///   2. `[]` Stake pool withdraw authority
    ///   3. `[w]` Pool's inactive stake account
    ///   4. `[w]` Reserve stake account
    ///   5. '[]' Sysvar clock account
    ///   6. '[]' Sysvar stake history account
    ///   7. `[]` Stake program id,
    MergeInactiveStake,

    ///   Create account for pool's referrer list
    /// 
    ///   0. `[]` Stake pool
    ///   1. `[s]` Manager
    ///   2. `[w]` Account for pool's referrer list
    ///   3. `[]` Rent sysvar
    ///   4. `[]` System program account 
    /// 
    ///   userdata: maximum number of referrers in the list  
    CreateReferrerList(u32),

    ///   Add a referrer to the pool's referrer list
    /// 
    ///   0. `[]` Stake pool
    ///   1. `[s]` Manager
    ///   2. `[w]` Account for pool's referrer list
    ///   3. `[]` Referrer's account
    AddReferrer,

    ///   Deposit SOL directly into the pool's reserve account with existing DAO`s community tokens strategy. The output is a "pool" token
    ///   representing ownership into the pool. Inputs are converted to the current ratio.
    ///   This instructions is a part of our Referral program and must include a whitelisted referral.
    ///
    ///   0. `[w]` Stake pool
    ///   1. `[]` Stake pool withdraw authority
    ///   2. `[w]` Reserve stake account, to deposit SOL
    ///   3. `[s]` Account providing the lamports to be deposited into the pool
    ///   4. `[w]` User account to receive pool tokens
    ///   5  `[]` User account to hold DAO`s community tokens
    ///   6. `[w]` Account to receive fee tokens
    ///   7. `[w]` Account to receive a portion of fee in SOL as referral fees
    ///   8. `[]` Referrer list dto account
    ///   9  `[w]` Account for storing metrics of deposit sol with refferer transaction
    ///  10. `[w]` Metrics counter for deposit sol transactions (dto account)
    ///  11. `[w]` Pool token mint account
    ///  12. `[]` System program account
    ///  13. `[]` Rent sysvar   
    ///  14. `[]` Token program id
    ///  15. `[w]` Account for storing community token staking rewards dto
    ///  16. `[s]` Wallet owner
    ///  17. `[]` Account for storing community token dto
    ///  18. `[s]` (Optional) Stake pool sol deposit authority.
    DaoStrategyDepositSolWithReferrer(u64),

    ///   Remove a referrer from the pool's referrer list
    /// 
    ///   0. `[]` Stake pool
    ///   1. `[s]` Manager
    ///   2. `[w]` Account for pool's referrer list
    ///   3. `[]` Referrer's account
    RemoveReferrer,

    ///   Create account for storing counter for metrics of deposit sol transactions with referrer
    ///   0. `[]` Stake pool
    ///   1. `[s]` Manager
    ///   2. `[w]` Account for storing counter for metrics of deposit sol transactions with referrer
    ///   3. `[]` Rent sysvar
    ///   4. `[]` System program account
    CreateMetricsDepositReferrerCounter,

    ///   Remove metrics accounts of deposit sol transactions with referrer
    ///   We don't need metrics flushed to DB
    /// 
    ///   0. `[]` Stake pool
    ///   1. `[s]` Manager
    ///   2. `[w]` Account for storing counter for metrics of deposit sol transactions with referrer
    ///   3. `[]` System program account
    ///   4. `[w]` Metrics account
    ///   5.. `[w]` (Optional) Metrics accounts
    /// 
    RemoveMetricsDepositReferrer,

    ///   Update or Create token metadata
    /// 
    ///   0. `[]` Stake pool
    ///   1. `[s]` Manager
    ///   2. `[]` Mint (withdraw) authority (Community token or Pool token mint authority)
    ///   3. `[]` Token mint (Community token or Pool token mint)
    ///   4. `[]` Rent sysvar
    ///   5. `[]` System program account
    /// 
    UpdateTokenMetadata {
        #[allow(dead_code)] // but it's not
        /// token name
        name: String,
        #[allow(dead_code)] // but it's not
        /// token symbol
        symbol: String,
        /// token metadata uri
        #[allow(dead_code)] // but it's not
        uri: String,
    },
    

    /// (Manager only) No fee deposit threshold
    ///
    ///  0. `[w]` StakePool
    ///  1. `[s]` Manager
    /// 
    /// userdata: threshold
    SetNoFeeDepositThreshold(u16),

    ///   Deposit SOL directly into the pool's reserve account with existing DAO`s community tokens strategy. The output is a "pool" token
    ///   representing ownership into the pool. Inputs are converted to the current ratio.
    ///   This instructions is a part of our Referral program v2, where referrer fee is paid from the manager/epoch fee, and must include a whitelisted referral.
    ///
    ///   0. `[w]` Stake pool
    ///   1. `[]` Stake pool withdraw authority
    ///   2. `[w]` Reserve stake account, to deposit SOL
    ///   3. `[s]` Account providing the lamports to be deposited into the pool
    ///   4. `[w]` User account to receive pool tokens
    ///   5  `[]` User account to hold DAO`s community tokens
    ///   6. `[w]` Account to receive fee tokens
    ///   7. `[]` Referrer id (their SOL account, fee may be paid in sol or esol later on)
    ///   8. `[]` Referrer list dto account
    ///   9. `[w]` Pool token mint account
    ///  10. `[]` System program account  
    ///  11. `[]` Token program id
    ///  12. `[w]` Account for storing community token staking rewards dto
    ///  13. `[s]` Wallet owner
    ///  14. `[]` Account for storing community token dto
    ///  15. `[s]` (Optional) Stake pool sol deposit authority.
    //
    // Index 39 
    DaoStrategyDepositSolWithReferrer2(u64),
}

/// Creates an 'initialize' instruction.
pub fn initialize(
    program_id: &Pubkey,
    stake_pool: &Pubkey,
    manager: &Pubkey,
    staker: &Pubkey,
    stake_pool_withdraw_authority: &Pubkey,
    validator_list: &Pubkey,
    reserve_stake: &Pubkey,
    pool_mint: &Pubkey,
    manager_pool_account: &Pubkey,
    treasury_fee_account: &Pubkey,
    token_program_id: &Pubkey,
    deposit_authority: Option<Pubkey>,
    fee: Fee,
    withdrawal_fee: Fee,
    deposit_fee: Fee,
    referral_fee: u8,
    treasury_fee: Fee,
    max_validators: u32,
    no_fee_deposit_threshold: u16,
) -> Instruction {
    let init_data = StakePoolInstruction::Initialize {
        fee,
        withdrawal_fee,
        deposit_fee,
        treasury_fee,
        referral_fee,
        max_validators,
        no_fee_deposit_threshold,
    };
    let data = init_data.try_to_vec().unwrap();
    let mut accounts = vec![
        AccountMeta::new(*stake_pool, false),
        AccountMeta::new_readonly(*manager, true),
        AccountMeta::new_readonly(*staker, false),
        AccountMeta::new_readonly(*stake_pool_withdraw_authority, false),
        AccountMeta::new(*validator_list, false),
        AccountMeta::new_readonly(*reserve_stake, false),
        AccountMeta::new(*pool_mint, false),
        AccountMeta::new(*manager_pool_account, false),
        AccountMeta::new_readonly(*treasury_fee_account, false),
        AccountMeta::new_readonly(*token_program_id, false),
    ];
    if let Some(deposit_authority) = deposit_authority {
        accounts.push(AccountMeta::new_readonly(deposit_authority, true));
    }
    Instruction {
        program_id: *program_id,
        accounts,
        data,
    }
}

/// Creates `AddValidatorToPool` instruction (add new validator stake account to the pool)
pub fn add_validator_to_pool(
    program_id: &Pubkey,
    stake_pool: &Pubkey,
    staker: &Pubkey,
    funder: &Pubkey,
    stake_pool_withdraw: &Pubkey,
    validator_list: &Pubkey,
    stake: &Pubkey,
    validator: &Pubkey,
) -> Instruction {
    let accounts = vec![
        AccountMeta::new(*stake_pool, false),
        AccountMeta::new_readonly(*staker, true),
        AccountMeta::new(*funder, true),
        AccountMeta::new_readonly(*stake_pool_withdraw, false),
        AccountMeta::new(*validator_list, false),
        AccountMeta::new(*stake, false),
        AccountMeta::new_readonly(*validator, false),
        AccountMeta::new_readonly(sysvar::rent::id(), false),
        AccountMeta::new_readonly(sysvar::clock::id(), false),
        AccountMeta::new_readonly(sysvar::stake_history::id(), false),
        AccountMeta::new_readonly(stake::config::id(), false),
        AccountMeta::new_readonly(system_program::id(), false),
        AccountMeta::new_readonly(stake::program::id(), false),
    ];
    Instruction {
        program_id: *program_id,
        accounts,
        data: StakePoolInstruction::AddValidatorToPool
            .try_to_vec()
            .unwrap(),
    }
}

/// Creates `RemoveValidatorFromPool` instruction (remove validator stake account from the pool)
pub fn remove_validator_from_pool(
    program_id: &Pubkey,
    stake_pool: &Pubkey,
    staker: &Pubkey,
    stake_pool_withdraw: &Pubkey,
    new_stake_authority: &Pubkey,
    validator_list: &Pubkey,
    stake_account: &Pubkey,
    transient_stake_account: &Pubkey,
    destination_stake_account: &Pubkey,
) -> Instruction {
    let accounts = vec![
        AccountMeta::new(*stake_pool, false),
        AccountMeta::new_readonly(*staker, true),
        AccountMeta::new_readonly(*stake_pool_withdraw, false),
        AccountMeta::new_readonly(*new_stake_authority, false),
        AccountMeta::new(*validator_list, false),
        AccountMeta::new(*stake_account, false),
        AccountMeta::new_readonly(*transient_stake_account, false),
        AccountMeta::new(*destination_stake_account, false),
        AccountMeta::new_readonly(sysvar::clock::id(), false),
        AccountMeta::new_readonly(stake::program::id(), false),
    ];
    Instruction {
        program_id: *program_id,
        accounts,
        data: StakePoolInstruction::RemoveValidatorFromPool
            .try_to_vec()
            .unwrap(),
    }
}

/// Creates `DecreaseValidatorStake` instruction (rebalance from validator account to
/// transient account)
pub fn decrease_validator_stake(
    program_id: &Pubkey,
    stake_pool: &Pubkey,
    staker: &Pubkey,
    stake_pool_withdraw_authority: &Pubkey,
    validator_list: &Pubkey,
    validator_stake: &Pubkey,
    transient_stake: &Pubkey,
    lamports: u64,
    transient_stake_seed: u64,
) -> Instruction {
    let accounts = vec![
        AccountMeta::new_readonly(*stake_pool, false),
        AccountMeta::new_readonly(*staker, true),
        AccountMeta::new_readonly(*stake_pool_withdraw_authority, false),
        AccountMeta::new(*validator_list, false),
        AccountMeta::new(*validator_stake, false),
        AccountMeta::new(*transient_stake, false),
        AccountMeta::new_readonly(sysvar::clock::id(), false),
        AccountMeta::new_readonly(sysvar::rent::id(), false),
        AccountMeta::new_readonly(system_program::id(), false),
        AccountMeta::new_readonly(stake::program::id(), false),
    ];
    Instruction {
        program_id: *program_id,
        accounts,
        data: StakePoolInstruction::DecreaseValidatorStake {
            lamports,
            transient_stake_seed,
        }
        .try_to_vec()
        .unwrap(),
    }
}

/// Creates `IncreaseValidatorStake` instruction (rebalance from reserve account to
/// transient account)
pub fn increase_validator_stake(
    program_id: &Pubkey,
    stake_pool: &Pubkey,
    staker: &Pubkey,
    stake_pool_withdraw_authority: &Pubkey,
    validator_list: &Pubkey,
    reserve_stake: &Pubkey,
    transient_stake: &Pubkey,
    validator: &Pubkey,
    lamports: u64,
    transient_stake_seed: u64,
) -> Instruction {
    let accounts = vec![
        AccountMeta::new_readonly(*stake_pool, false),
        AccountMeta::new_readonly(*staker, true),
        AccountMeta::new_readonly(*stake_pool_withdraw_authority, false),
        AccountMeta::new(*validator_list, false),
        AccountMeta::new(*reserve_stake, false),
        AccountMeta::new(*transient_stake, false),
        AccountMeta::new_readonly(*validator, false),
        AccountMeta::new_readonly(sysvar::clock::id(), false),
        AccountMeta::new_readonly(sysvar::rent::id(), false),
        AccountMeta::new_readonly(sysvar::stake_history::id(), false),
        AccountMeta::new_readonly(stake::config::id(), false),
        AccountMeta::new_readonly(system_program::id(), false),
        AccountMeta::new_readonly(stake::program::id(), false),
    ];
    Instruction {
        program_id: *program_id,
        accounts,
        data: StakePoolInstruction::IncreaseValidatorStake {
            lamports,
            transient_stake_seed,
        }
        .try_to_vec()
        .unwrap(),
    }
}

/// Creates `SetPreferredDepositValidator` instruction
pub fn set_preferred_validator(
    program_id: &Pubkey,
    stake_pool_address: &Pubkey,
    staker: &Pubkey,
    validator_list_address: &Pubkey,
    validator_type: PreferredValidatorType,
    validator_vote_address: Option<Pubkey>,
) -> Instruction {
    Instruction {
        program_id: *program_id,
        accounts: vec![
            AccountMeta::new(*stake_pool_address, false),
            AccountMeta::new_readonly(*staker, true),
            AccountMeta::new_readonly(*validator_list_address, false),
        ],
        data: StakePoolInstruction::SetPreferredValidator {
            validator_type,
            validator_vote_address,
        }
        .try_to_vec()
        .unwrap(),
    }
}

/// Create an `AddValidatorToPool` instruction given an existing stake pool and
/// vote account
pub fn add_validator_to_pool_with_vote(
    program_id: &Pubkey,
    stake_pool: &StakePool,
    stake_pool_address: &Pubkey,
    funder: &Pubkey,
    vote_account_address: &Pubkey,
) -> Instruction {
    let pool_withdraw_authority =
        find_withdraw_authority_program_address(program_id, stake_pool_address).0;
    let (stake_account_address, _) =
        find_stake_program_address(program_id, vote_account_address, stake_pool_address);
    add_validator_to_pool(
        program_id,
        stake_pool_address,
        &stake_pool.staker,
        funder,
        &pool_withdraw_authority,
        &stake_pool.validator_list,
        &stake_account_address,
        vote_account_address,
    )
}

/// Create an `RemoveValidatorFromPool` instruction given an existing stake pool and
/// vote account
pub fn remove_validator_from_pool_with_vote(
    program_id: &Pubkey,
    stake_pool: &StakePool,
    stake_pool_address: &Pubkey,
    vote_account_address: &Pubkey,
    new_stake_account_authority: &Pubkey,
    transient_stake_seed: u64,
    destination_stake_address: &Pubkey,
) -> Instruction {
    let pool_withdraw_authority =
        find_withdraw_authority_program_address(program_id, stake_pool_address).0;
    let (stake_account_address, _) =
        find_stake_program_address(program_id, vote_account_address, stake_pool_address);
    let (transient_stake_account, _) = find_transient_stake_program_address(
        program_id,
        vote_account_address,
        stake_pool_address,
        transient_stake_seed,
    );
    remove_validator_from_pool(
        program_id,
        stake_pool_address,
        &stake_pool.staker,
        &pool_withdraw_authority,
        new_stake_account_authority,
        &stake_pool.validator_list,
        &stake_account_address,
        &transient_stake_account,
        destination_stake_address,
    )
}

/// Create an `IncreaseValidatorStake` instruction given an existing stake pool and
/// vote account
pub fn increase_validator_stake_with_vote(
    program_id: &Pubkey,
    stake_pool: &StakePool,
    stake_pool_address: &Pubkey,
    vote_account_address: &Pubkey,
    lamports: u64,
    transient_stake_seed: u64,
) -> Instruction {
    let pool_withdraw_authority =
        find_withdraw_authority_program_address(program_id, stake_pool_address).0;
    let (transient_stake_address, _) = find_transient_stake_program_address(
        program_id,
        vote_account_address,
        stake_pool_address,
        transient_stake_seed,
    );

    increase_validator_stake(
        program_id,
        stake_pool_address,
        &stake_pool.staker,
        &pool_withdraw_authority,
        &stake_pool.validator_list,
        &stake_pool.reserve_stake,
        &transient_stake_address,
        vote_account_address,
        lamports,
        transient_stake_seed,
    )
}

/// Create a `DecreaseValidatorStake` instruction given an existing stake pool and
/// vote account
pub fn decrease_validator_stake_with_vote(
    program_id: &Pubkey,
    stake_pool: &StakePool,
    stake_pool_address: &Pubkey,
    vote_account_address: &Pubkey,
    lamports: u64,
    transient_stake_seed: u64,
) -> Instruction {
    let pool_withdraw_authority =
        find_withdraw_authority_program_address(program_id, stake_pool_address).0;
    let (validator_stake_address, _) =
        find_stake_program_address(program_id, vote_account_address, stake_pool_address);
    let (transient_stake_address, _) = find_transient_stake_program_address(
        program_id,
        vote_account_address,
        stake_pool_address,
        transient_stake_seed,
    );
    decrease_validator_stake(
        program_id,
        stake_pool_address,
        &stake_pool.staker,
        &pool_withdraw_authority,
        &stake_pool.validator_list,
        &validator_stake_address,
        &transient_stake_address,
        lamports,
        transient_stake_seed,
    )
}

/// Creates `UpdateValidatorListBalance` instruction (update validator stake account balances)
pub fn update_validator_list_balance(
    program_id: &Pubkey,
    stake_pool: &Pubkey,
    manager: &Pubkey,
    stake_pool_withdraw_authority: &Pubkey,
    validator_list_address: &Pubkey,
    reserve_stake: &Pubkey,
    validator_list: &ValidatorList,
    validator_vote_accounts: &[Pubkey],
    start_index: u32,
    no_merge: bool,
) -> Instruction {
    let mut accounts = vec![
        AccountMeta::new_readonly(*stake_pool, false),
        AccountMeta::new_readonly(*manager, true),
        AccountMeta::new_readonly(*stake_pool_withdraw_authority, false),
        AccountMeta::new(*validator_list_address, false),
        AccountMeta::new(*reserve_stake, false),
        AccountMeta::new_readonly(sysvar::clock::id(), false),
        AccountMeta::new_readonly(sysvar::stake_history::id(), false),
        AccountMeta::new_readonly(stake::program::id(), false),
    ];
    accounts.append(
        &mut validator_vote_accounts
            .iter()
            .flat_map(|vote_account_address| {
                let validator_stake_info = validator_list.find(vote_account_address);
                if let Some(validator_stake_info) = validator_stake_info {
                    let (validator_stake_account, _) =
                        find_stake_program_address(program_id, vote_account_address, stake_pool);
                    let (transient_stake_account, _) = find_transient_stake_program_address(
                        program_id,
                        vote_account_address,
                        stake_pool,
                        validator_stake_info.transient_seed_suffix_start,
                    );
                    vec![
                        AccountMeta::new(validator_stake_account, false),
                        AccountMeta::new(transient_stake_account, false),
                    ]
                } else {
                    vec![]
                }
            })
            .collect::<Vec<AccountMeta>>(),
    );
    Instruction {
        program_id: *program_id,
        accounts,
        data: StakePoolInstruction::UpdateValidatorListBalance {
            start_index,
            no_merge,
        }
        .try_to_vec()
        .unwrap(),
    }
}

/// Creates `UpdateStakePoolBalance` instruction (pool balance from the stake account list balances)
pub fn update_stake_pool_balance(
    program_id: &Pubkey,
    stake_pool: &Pubkey,
    manager: &Pubkey,
    withdraw_authority: &Pubkey,
    validator_list_storage: &Pubkey,
    reserve_stake: &Pubkey,
    manager_fee_account: &Pubkey,
    stake_pool_mint: &Pubkey,
    treasury_fee_account: &Pubkey,
    token_program_id: &Pubkey,
    max_validator_yield_per_epoch_numerator: u32,
) -> Instruction {
    let accounts = vec![
        AccountMeta::new(*stake_pool, false),
        AccountMeta::new_readonly(*manager, true),
        AccountMeta::new_readonly(*withdraw_authority, false),
        AccountMeta::new(*validator_list_storage, false),
        AccountMeta::new_readonly(*reserve_stake, false),
        AccountMeta::new(*manager_fee_account, false),
        AccountMeta::new(*stake_pool_mint, false),
        AccountMeta::new(*treasury_fee_account, false),
        AccountMeta::new_readonly(*token_program_id, false),
    ];
    Instruction {
        program_id: *program_id,
        accounts,
        data: StakePoolInstruction::UpdateStakePoolBalance(max_validator_yield_per_epoch_numerator)
            .try_to_vec()
            .unwrap(),
    }
}

/// Creates `CleanupRemovedValidatorEntries` instruction (removes entries from the validator list)
pub fn cleanup_removed_validator_entries(
    program_id: &Pubkey,
    stake_pool: &Pubkey,
    manager: &Pubkey,
    validator_list_storage: &Pubkey,
) -> Instruction {
    let accounts = vec![
        AccountMeta::new_readonly(*stake_pool, false),
        AccountMeta::new_readonly(*manager, true),
        AccountMeta::new(*validator_list_storage, false),
    ];
    Instruction {
        program_id: *program_id,
        accounts,
        data: StakePoolInstruction::CleanupRemovedValidatorEntries
            .try_to_vec()
            .unwrap(),
    }
}

/// Creates all `UpdateValidatorListBalance` and `UpdateStakePoolBalance`
/// instructions for fully updating a stake pool each epoch
pub fn update_stake_pool(
    program_id: &Pubkey,
    stake_pool: &StakePool,
    manager: &Pubkey,
    validator_list: &ValidatorList,
    stake_pool_address: &Pubkey,
    no_merge: bool,
    max_validator_yield_per_epoch_numerator: u32,
) -> (Vec<Instruction>, Vec<Instruction>) {
    let vote_accounts: Vec<Pubkey> = validator_list
        .validators
        .iter()
        .map(|item| item.vote_account_address)
        .collect();

    let (withdraw_authority, _) =
        find_withdraw_authority_program_address(program_id, stake_pool_address);

    let mut update_list_instructions: Vec<Instruction> = vec![];
    let mut start_index = 0;
    for accounts_chunk in vote_accounts.chunks(MAX_VALIDATORS_TO_UPDATE) {
        update_list_instructions.push(update_validator_list_balance(
            program_id,
            stake_pool_address,
            manager,
            &withdraw_authority,
            &stake_pool.validator_list,
            &stake_pool.reserve_stake,
            validator_list,
            accounts_chunk,
            start_index,
            no_merge,
        ));
        start_index += MAX_VALIDATORS_TO_UPDATE as u32;
    }

    let final_instructions = vec![
        update_stake_pool_balance(
            program_id,
            stake_pool_address,
            manager,
            &withdraw_authority,
            &stake_pool.validator_list,
            &stake_pool.reserve_stake,
            &stake_pool.manager_fee_account,
            &stake_pool.pool_mint,
            &stake_pool.treasury_fee_account,
            &stake_pool.token_program_id,
            max_validator_yield_per_epoch_numerator,
        ),
        cleanup_removed_validator_entries(
            program_id,
            stake_pool_address,
            manager,
            &stake_pool.validator_list,
        ),
    ];
    (update_list_instructions, final_instructions)
}

/// Creates instructions required to deposit into a stake pool, given a stake
/// account owned by the user.
pub fn deposit_stake(
    program_id: &Pubkey,
    stake_pool: &Pubkey,
    validator_list_storage: &Pubkey,
    stake_pool_withdraw_authority: &Pubkey,
    deposit_stake_address: &Pubkey,
    deposit_stake_withdraw_authority: &Pubkey,
    validator_stake_account: &Pubkey,
    reserve_stake_account: &Pubkey,
    pool_tokens_to: &Pubkey,
    manager_fee_account: &Pubkey,
    referrer_pool_tokens_account: &Pubkey,
    pool_mint: &Pubkey,
    token_program_id: &Pubkey,
) -> Vec<Instruction> {
    let stake_pool_deposit_authority =
        find_deposit_authority_program_address(program_id, stake_pool).0;
    let accounts = vec![
        AccountMeta::new(*stake_pool, false),
        AccountMeta::new(*validator_list_storage, false),
        AccountMeta::new_readonly(stake_pool_deposit_authority, false),
        AccountMeta::new_readonly(*stake_pool_withdraw_authority, false),
        AccountMeta::new(*deposit_stake_address, false),
        AccountMeta::new(*validator_stake_account, false),
        AccountMeta::new(*reserve_stake_account, false),
        AccountMeta::new(*pool_tokens_to, false),
        AccountMeta::new(*manager_fee_account, false),
        AccountMeta::new(*referrer_pool_tokens_account, false),
        AccountMeta::new(*pool_mint, false),
        AccountMeta::new_readonly(sysvar::clock::id(), false),
        AccountMeta::new_readonly(sysvar::stake_history::id(), false),
        AccountMeta::new_readonly(*token_program_id, false),
        AccountMeta::new_readonly(stake::program::id(), false),
    ];
    vec![
        stake::instruction::authorize(
            deposit_stake_address,
            deposit_stake_withdraw_authority,
            &stake_pool_deposit_authority,
            stake::state::StakeAuthorize::Staker,
            None,
        ),
        stake::instruction::authorize(
            deposit_stake_address,
            deposit_stake_withdraw_authority,
            &stake_pool_deposit_authority,
            stake::state::StakeAuthorize::Withdrawer,
            None,
        ),
        Instruction {
            program_id: *program_id,
            accounts,
            data: StakePoolInstruction::DepositStake.try_to_vec().unwrap(),
        },
    ]
}

/// Creates instructions required to deposit into a stake pool, given a stake
/// account owned by the user. The difference with `deposit()` is that a deposit
/// authority must sign this instruction, which is required for private pools.
pub fn deposit_stake_with_authority(
    program_id: &Pubkey,
    stake_pool: &Pubkey,
    validator_list_storage: &Pubkey,
    stake_pool_deposit_authority: &Pubkey,
    stake_pool_withdraw_authority: &Pubkey,
    deposit_stake_address: &Pubkey,
    deposit_stake_withdraw_authority: &Pubkey,
    validator_stake_account: &Pubkey,
    reserve_stake_account: &Pubkey,
    pool_tokens_to: &Pubkey,
    manager_fee_account: &Pubkey,
    referrer_pool_tokens_account: &Pubkey,
    pool_mint: &Pubkey,
    token_program_id: &Pubkey,
) -> Vec<Instruction> {
    let accounts = vec![
        AccountMeta::new(*stake_pool, false),
        AccountMeta::new(*validator_list_storage, false),
        AccountMeta::new_readonly(*stake_pool_deposit_authority, true),
        AccountMeta::new_readonly(*stake_pool_withdraw_authority, false),
        AccountMeta::new(*deposit_stake_address, false),
        AccountMeta::new(*validator_stake_account, false),
        AccountMeta::new(*reserve_stake_account, false),
        AccountMeta::new(*pool_tokens_to, false),
        AccountMeta::new(*manager_fee_account, false),
        AccountMeta::new(*referrer_pool_tokens_account, false),
        AccountMeta::new(*pool_mint, false),
        AccountMeta::new_readonly(sysvar::clock::id(), false),
        AccountMeta::new_readonly(sysvar::stake_history::id(), false),
        AccountMeta::new_readonly(*token_program_id, false),
        AccountMeta::new_readonly(stake::program::id(), false),
    ];
    vec![
        stake::instruction::authorize(
            deposit_stake_address,
            deposit_stake_withdraw_authority,
            stake_pool_deposit_authority,
            stake::state::StakeAuthorize::Staker,
            None,
        ),
        stake::instruction::authorize(
            deposit_stake_address,
            deposit_stake_withdraw_authority,
            stake_pool_deposit_authority,
            stake::state::StakeAuthorize::Withdrawer,
            None,
        ),
        Instruction {
            program_id: *program_id,
            accounts,
            data: StakePoolInstruction::DepositStake.try_to_vec().unwrap(),
        },
    ]
}

/// Creates instructions required to deposit SOL directly into a stake pool.
pub fn deposit_sol(
    program_id: &Pubkey,
    stake_pool: &Pubkey,
    stake_pool_withdraw_authority: &Pubkey,
    reserve_stake_account: &Pubkey,
    lamports_from: &Pubkey,
    pool_tokens_to: &Pubkey,
    manager_fee_account: &Pubkey,
    referrer_pool_tokens_account: &Pubkey,
    pool_mint: &Pubkey,
    token_program_id: &Pubkey,
    amount: u64,
) -> Instruction {
    let accounts = vec![
        AccountMeta::new(*stake_pool, false),
        AccountMeta::new_readonly(*stake_pool_withdraw_authority, false),
        AccountMeta::new(*reserve_stake_account, false),
        AccountMeta::new(*lamports_from, true),
        AccountMeta::new(*pool_tokens_to, false),
        AccountMeta::new(*manager_fee_account, false),
        AccountMeta::new(*referrer_pool_tokens_account, false),
        AccountMeta::new(*pool_mint, false),
        AccountMeta::new_readonly(system_program::id(), false),
        AccountMeta::new_readonly(*token_program_id, false),
    ];
    Instruction {
        program_id: *program_id,
        accounts,
        data: StakePoolInstruction::DepositSol(amount)
            .try_to_vec()
            .unwrap(),
    }
}

/// Creates instruction required to deposit SOL directly into a stake pool.
/// The difference with `deposit_sol()` is that a deposit
/// authority must sign this instruction.
pub fn deposit_sol_with_authority(
    program_id: &Pubkey,
    stake_pool: &Pubkey,
    sol_deposit_authority: &Pubkey,
    stake_pool_withdraw_authority: &Pubkey,
    reserve_stake_account: &Pubkey,
    lamports_from: &Pubkey,
    pool_tokens_to: &Pubkey,
    manager_fee_account: &Pubkey,
    referrer_pool_tokens_account: &Pubkey,
    pool_mint: &Pubkey,
    token_program_id: &Pubkey,
    amount: u64,
) -> Instruction {
    let accounts = vec![
        AccountMeta::new(*stake_pool, false),
        AccountMeta::new_readonly(*stake_pool_withdraw_authority, false),
        AccountMeta::new(*reserve_stake_account, false),
        AccountMeta::new(*lamports_from, true),
        AccountMeta::new(*pool_tokens_to, false),
        AccountMeta::new(*manager_fee_account, false),
        AccountMeta::new(*referrer_pool_tokens_account, false),
        AccountMeta::new(*pool_mint, false),
        AccountMeta::new_readonly(system_program::id(), false),
        AccountMeta::new_readonly(*token_program_id, false),
        AccountMeta::new_readonly(*sol_deposit_authority, true),
    ];
    Instruction {
        program_id: *program_id,
        accounts,
        data: StakePoolInstruction::DepositSol(amount)
            .try_to_vec()
            .unwrap(),
    }
}

/// Creates a 'WithdrawStake' instruction.
pub fn withdraw_stake(
    program_id: &Pubkey,
    stake_pool: &Pubkey,
    validator_list_storage: &Pubkey,
    stake_pool_withdraw: &Pubkey,
    stake_to_split: &Pubkey,
    stake_to_receive: &Pubkey,
    user_stake_authority: &Pubkey,
    user_transfer_authority: &Pubkey,
    user_pool_token_account: &Pubkey,
    manager_fee_account: &Pubkey,
    pool_mint: &Pubkey,
    token_program_id: &Pubkey,
    amount: u64,
) -> Instruction {
    let accounts = vec![
        AccountMeta::new(*stake_pool, false),
        AccountMeta::new(*validator_list_storage, false),
        AccountMeta::new_readonly(*stake_pool_withdraw, false),
        AccountMeta::new(*stake_to_split, false),
        AccountMeta::new(*stake_to_receive, false),
        AccountMeta::new_readonly(*user_stake_authority, false),
        AccountMeta::new_readonly(*user_transfer_authority, true),
        AccountMeta::new(*user_pool_token_account, false),
        AccountMeta::new(*manager_fee_account, false),
        AccountMeta::new(*pool_mint, false),
        AccountMeta::new_readonly(sysvar::clock::id(), false),
        AccountMeta::new_readonly(*token_program_id, false),
        AccountMeta::new_readonly(stake::program::id(), false),
    ];
    Instruction {
        program_id: *program_id,
        accounts,
        data: StakePoolInstruction::WithdrawStake(amount)
            .try_to_vec()
            .unwrap(),
    }
}

/// Creates instruction required to withdraw SOL directly from a stake pool.
pub fn withdraw_sol(
    program_id: &Pubkey,
    stake_pool: &Pubkey,
    stake_pool_withdraw_authority: &Pubkey,
    user_transfer_authority: &Pubkey,
    pool_tokens_from: &Pubkey,
    reserve_stake_account: &Pubkey,
    lamports_to: &Pubkey,
    manager_fee_account: &Pubkey,
    pool_mint: &Pubkey,
    token_program_id: &Pubkey,
    pool_tokens: u64,
) -> Instruction {
    let accounts = vec![
        AccountMeta::new(*stake_pool, false),
        AccountMeta::new_readonly(*stake_pool_withdraw_authority, false),
        AccountMeta::new_readonly(*user_transfer_authority, true),
        AccountMeta::new(*pool_tokens_from, false),
        AccountMeta::new(*reserve_stake_account, false),
        AccountMeta::new(*lamports_to, false),
        AccountMeta::new(*manager_fee_account, false),
        AccountMeta::new(*pool_mint, false),
        AccountMeta::new_readonly(sysvar::clock::id(), false),
        AccountMeta::new_readonly(sysvar::stake_history::id(), false),
        AccountMeta::new_readonly(stake::program::id(), false),
        AccountMeta::new_readonly(*token_program_id, false),
    ];
    Instruction {
        program_id: *program_id,
        accounts,
        data: StakePoolInstruction::WithdrawSol(pool_tokens)
            .try_to_vec()
            .unwrap(),
    }
}

/// Creates instruction required to withdraw SOL directly from a stake pool.
/// The difference with `withdraw_sol()` is that the sol withdraw authority
/// must sign this instruction.
pub fn withdraw_sol_with_authority(
    program_id: &Pubkey,
    stake_pool: &Pubkey,
    sol_withdraw_authority: &Pubkey,
    stake_pool_withdraw_authority: &Pubkey,
    user_transfer_authority: &Pubkey,
    pool_tokens_from: &Pubkey,
    reserve_stake_account: &Pubkey,
    lamports_to: &Pubkey,
    manager_fee_account: &Pubkey,
    pool_mint: &Pubkey,
    token_program_id: &Pubkey,
    pool_tokens: u64,
) -> Instruction {
    let accounts = vec![
        AccountMeta::new(*stake_pool, false),
        AccountMeta::new_readonly(*stake_pool_withdraw_authority, false),
        AccountMeta::new_readonly(*user_transfer_authority, true),
        AccountMeta::new(*pool_tokens_from, false),
        AccountMeta::new(*reserve_stake_account, false),
        AccountMeta::new(*lamports_to, false),
        AccountMeta::new(*manager_fee_account, false),
        AccountMeta::new(*pool_mint, false),
        AccountMeta::new_readonly(sysvar::clock::id(), false),
        AccountMeta::new_readonly(sysvar::stake_history::id(), false),
        AccountMeta::new_readonly(stake::program::id(), false),
        AccountMeta::new_readonly(*token_program_id, false),
        AccountMeta::new_readonly(*sol_withdraw_authority, true),
    ];
    Instruction {
        program_id: *program_id,
        accounts,
        data: StakePoolInstruction::WithdrawSol(pool_tokens)
            .try_to_vec()
            .unwrap(),
    }
}

/// Creates a 'set manager' instruction.
pub fn set_manager(
    program_id: &Pubkey,
    stake_pool: &Pubkey,
    manager: &Pubkey,
    new_manager: &Pubkey,
    new_fee_receiver: &Pubkey,
) -> Instruction {
    let accounts = vec![
        AccountMeta::new(*stake_pool, false),
        AccountMeta::new_readonly(*manager, true),
        AccountMeta::new_readonly(*new_manager, true),
        AccountMeta::new_readonly(*new_fee_receiver, false),
    ];
    Instruction {
        program_id: *program_id,
        accounts,
        data: StakePoolInstruction::SetManager.try_to_vec().unwrap(),
    }
}

/// Creates a 'set fee' instruction.
pub fn set_fee(
    program_id: &Pubkey,
    stake_pool: &Pubkey,
    manager: &Pubkey,
    fee: FeeType,
) -> Instruction {
    let accounts = vec![
        AccountMeta::new(*stake_pool, false),
        AccountMeta::new_readonly(*manager, true),
    ];
    Instruction {
        program_id: *program_id,
        accounts,
        data: StakePoolInstruction::SetFee { fee }.try_to_vec().unwrap(),
    }
}

/// Creates a 'set no fee deposit threshold' instruction.
pub fn set_no_fee_deposit_threshold(
    program_id: &Pubkey,
    stake_pool: &Pubkey,
    manager: &Pubkey,
    no_fee_deposit_threshold: u16,
) -> Instruction {
    let accounts = vec![
        AccountMeta::new(*stake_pool, false),
        AccountMeta::new_readonly(*manager, true),
    ];
    Instruction {
        program_id: *program_id,
        accounts,
        data: StakePoolInstruction::SetNoFeeDepositThreshold(
            no_fee_deposit_threshold
        ).try_to_vec().unwrap(),
    }
}

/// Creates a 'set staker' instruction.
pub fn set_staker(
    program_id: &Pubkey,
    stake_pool: &Pubkey,
    set_staker_authority: &Pubkey,
    new_staker: &Pubkey,
) -> Instruction {
    let accounts = vec![
        AccountMeta::new(*stake_pool, false),
        AccountMeta::new_readonly(*set_staker_authority, true),
        AccountMeta::new_readonly(*new_staker, false),
    ];
    Instruction {
        program_id: *program_id,
        accounts,
        data: StakePoolInstruction::SetStaker.try_to_vec().unwrap(),
    }
}

/// Creates a 'SetFundingAuthority' instruction.
pub fn set_funding_authority(
    program_id: &Pubkey,
    stake_pool: &Pubkey,
    manager: &Pubkey,
    new_sol_deposit_authority: Option<&Pubkey>,
    funding_type: FundingType,
) -> Instruction {
    let mut accounts = vec![
        AccountMeta::new(*stake_pool, false),
        AccountMeta::new_readonly(*manager, true),
    ];
    if let Some(auth) = new_sol_deposit_authority {
        accounts.push(AccountMeta::new_readonly(*auth, false))
    }
    Instruction {
        program_id: *program_id,
        accounts,
        data: StakePoolInstruction::SetFundingAuthority(funding_type)
            .try_to_vec()
            .unwrap(),
    }
}

/// Creates instructions required to deposit SOL directly into a stake pool liquidity.
pub fn deposit_liquidity_sol(
    program_id: &Pubkey,
    stake_pool: &Pubkey,
    manager: &Pubkey,
    stake_pool_withdraw_authority: &Pubkey,
    reserve_stake_account: &Pubkey,
    lamports_from: &Pubkey,
    amount: u64,
) -> Instruction {
    let accounts = vec![
        AccountMeta::new(*stake_pool, false),
        AccountMeta::new_readonly(*manager, true),
        AccountMeta::new_readonly(*stake_pool_withdraw_authority, false),
        AccountMeta::new(*reserve_stake_account, false),
        AccountMeta::new(*lamports_from, true),
        AccountMeta::new_readonly(system_program::id(), false),
    ];
    Instruction {
        program_id: *program_id,
        accounts,
        data: StakePoolInstruction::DepositLiquiditySol(amount)
            .try_to_vec()
            .unwrap(),
    }
}

/// Creates instruction required to deposit SOL directly into a stake pool liquidity.
/// The difference with `deposit_liquidity_sol()` is that a deposit
/// authority must sign this instruction.
pub fn deposit_liquidity_sol_with_authority(
    program_id: &Pubkey,
    stake_pool: &Pubkey,
    manager: &Pubkey,
    sol_deposit_authority: &Pubkey,
    stake_pool_withdraw_authority: &Pubkey,
    reserve_stake_account: &Pubkey,
    lamports_from: &Pubkey,
    amount: u64,
) -> Instruction {
    let accounts = vec![
        AccountMeta::new(*stake_pool, false),
        AccountMeta::new_readonly(*manager, true),
        AccountMeta::new_readonly(*stake_pool_withdraw_authority, false),
        AccountMeta::new(*reserve_stake_account, false),
        AccountMeta::new(*lamports_from, true),
        AccountMeta::new_readonly(system_program::id(), false),
        AccountMeta::new_readonly(*sol_deposit_authority, true),
    ];
    Instruction {
        program_id: *program_id,
        accounts,
        data: StakePoolInstruction::DepositLiquiditySol(amount)
            .try_to_vec()
            .unwrap(),
    }
}

/// Creates instructions required to withdraw SOL directly from stake pool liquidity.
pub fn withdraw_liquidity_sol(
    program_id: &Pubkey,
    stake_pool: &Pubkey,
    manager: &Pubkey,
    stake_pool_withdraw_authority: &Pubkey,
    reserve_stake_account: &Pubkey,
    lamports_to: &Pubkey,
    amount: u64,
) -> Instruction {
    let accounts = vec![
        AccountMeta::new(*stake_pool, false),
        AccountMeta::new_readonly(*manager, true),
        AccountMeta::new_readonly(*stake_pool_withdraw_authority, false),
        AccountMeta::new(*reserve_stake_account, false),
        AccountMeta::new(*lamports_to, false),
        AccountMeta::new_readonly(sysvar::clock::id(), false),
        AccountMeta::new_readonly(sysvar::stake_history::id(), false),
        AccountMeta::new_readonly(stake::program::id(), false),
    ];
    Instruction {
        program_id: *program_id,
        accounts,
        data: StakePoolInstruction::WithdrawLiquiditySol(amount)
            .try_to_vec()
            .unwrap(),
    }
}

/// Creates instruction required to withdraw SOL directly from stake pool liquidity.
/// The difference with `deposit_liquidity_sol()` is that a deposit
/// authority must sign this instruction.
pub fn withdraw_liquidity_sol_with_authority(
    program_id: &Pubkey,
    stake_pool: &Pubkey,
    manager: &Pubkey,
    sol_withdraw_authority: &Pubkey,
    stake_pool_withdraw_authority: &Pubkey,
    reserve_stake_account: &Pubkey,
    lamports_to: &Pubkey,
    amount: u64,
) -> Instruction {
    let accounts = vec![
        AccountMeta::new(*stake_pool, false),
        AccountMeta::new_readonly(*manager, true),
        AccountMeta::new_readonly(*stake_pool_withdraw_authority, false),
        AccountMeta::new(*reserve_stake_account, false),
        AccountMeta::new(*lamports_to, false),
        AccountMeta::new_readonly(sysvar::clock::id(), false),
        AccountMeta::new_readonly(sysvar::stake_history::id(), false),
        AccountMeta::new_readonly(stake::program::id(), false),
        AccountMeta::new_readonly(*sol_withdraw_authority, true),
    ];
    Instruction {
        program_id: *program_id,
        accounts,
        data: StakePoolInstruction::WithdrawLiquiditySol(amount)
            .try_to_vec()
            .unwrap(),
    }
}

/// Creates instruction required to create account for storing DAO`s Community token`s mint.
pub fn create_community_token(
    program_id: &Pubkey,
    stake_pool: &Pubkey,
    manager: &Pubkey,
    community_token_dto: &Pubkey,
    token_mint: &Pubkey,
    dao_state_dto: &Pubkey,
) -> Instruction {
    let accounts = vec![
        AccountMeta::new_readonly(*stake_pool, false),
        AccountMeta::new_readonly(*manager, true),
        AccountMeta::new(*community_token_dto, false),
        AccountMeta::new(*dao_state_dto, false),
        AccountMeta::new_readonly(sysvar::rent::id(), false),
        AccountMeta::new_readonly(system_program::ID, false),
    ]; 

    Instruction {
        program_id: *program_id,
        accounts,
        data: StakePoolInstruction::CreateCommunityToken {
            token_mint: *token_mint,
        }
            .try_to_vec()
            .unwrap(),
    }
}

/// Creates instruction required to create account for storing Community tokens counter.
pub fn create_community_tokens_counter(
    program_id: &Pubkey,
    stake_pool: &Pubkey,
    manager: &Pubkey,
    community_tokens_counter_dto: &Pubkey,
    community_token_dto: &Pubkey,
) -> Instruction {
    let accounts = vec![
        AccountMeta::new_readonly(*stake_pool, false),
        AccountMeta::new_readonly(*manager, true),
        AccountMeta::new(*community_tokens_counter_dto, false),
        AccountMeta::new_readonly(*community_token_dto, false),
        AccountMeta::new_readonly(sysvar::rent::id(), false),
        AccountMeta::new_readonly(system_program::ID, false),
    ]; 

    Instruction {
        program_id: *program_id,
        accounts,
        data: StakePoolInstruction::CreateCommunityTokensCounter
            .try_to_vec()
            .unwrap(),
    }
}

/// Creates instruction required to create account for storing DAO`s state
pub fn create_dao_state(
    program_id: &Pubkey,
    stake_pool: &Pubkey,
    manager: &Pubkey,
    dao_state_dto: &Pubkey,
    is_enabled: bool,
) -> Instruction {
    let accounts = vec![
        AccountMeta::new_readonly(*stake_pool, false),
        AccountMeta::new_readonly(*manager, true),
        AccountMeta::new(*dao_state_dto, false),
        AccountMeta::new_readonly(sysvar::rent::id(), false),
        AccountMeta::new_readonly(system_program::ID, false),
    ]; 

    Instruction {
        program_id: *program_id,
        accounts,
        data: StakePoolInstruction::CreateDaoState { 
            is_enabled,
        }
            .try_to_vec()
            .unwrap(),
    }
}

/// Creates instruction required to create account for storing information for DAO`s community tokens destribution strategy
pub fn create_community_token_staking_rewards(
    program_id: &Pubkey,
    stake_pool: &Pubkey,
    owner_wallet: &Pubkey,
    community_token_staking_rewards_dto: &Pubkey,
    community_token_staking_rewards_counter_dto: &Pubkey,
) -> Instruction {
    let accounts = vec![
        AccountMeta::new_readonly(*stake_pool, false),
        AccountMeta::new_readonly(*owner_wallet, true),
        AccountMeta::new(*community_token_staking_rewards_dto, false),
        AccountMeta::new(*community_token_staking_rewards_counter_dto, false),
        AccountMeta::new_readonly(sysvar::rent::id(), false),
        AccountMeta::new_readonly(system_program::ID, false),
    ]; 
    
    Instruction {
        program_id: *program_id,
        accounts,
        data: StakePoolInstruction::CreateCommunityTokenStakingRewards
            .try_to_vec()
            .unwrap(),
    }
}

/// Creates instructions required to deposit SOL directly into a stake pool with existing DAO`s community tokens strategy.
pub fn dao_strategy_deposit_sol(
    program_id: &Pubkey,
    stake_pool: &Pubkey,
    stake_pool_withdraw_authority: &Pubkey,
    reserve_stake_account: &Pubkey,
    lamports_from: &Pubkey,
    pool_tokens_to: &Pubkey,
    dao_community_tokens_to: &Pubkey,
    manager_fee_account: &Pubkey,
    pool_mint: &Pubkey,
    token_program_id: &Pubkey,
    community_token_staking_rewards_dto: &Pubkey,
    owner_wallet: &Pubkey,
    community_token_dto_pubkey: &Pubkey,
    amount: u64,
) -> Instruction {
    let accounts = vec![
        AccountMeta::new(*stake_pool, false),
        AccountMeta::new_readonly(*stake_pool_withdraw_authority, false),
        AccountMeta::new(*reserve_stake_account, false),
        AccountMeta::new(*lamports_from, true),
        AccountMeta::new(*pool_tokens_to, false),
        AccountMeta::new_readonly(*dao_community_tokens_to, false),
        AccountMeta::new(*manager_fee_account, false),
        AccountMeta::new(*manager_fee_account, false),
        AccountMeta::new(*pool_mint, false),
        AccountMeta::new_readonly(system_program::id(), false),
        AccountMeta::new_readonly(*token_program_id, false),
        AccountMeta::new(*community_token_staking_rewards_dto, false),
        AccountMeta::new_readonly(*owner_wallet, true),
        AccountMeta::new_readonly(*community_token_dto_pubkey, false),
    ];
    Instruction {
        program_id: *program_id,
        accounts,
        data: StakePoolInstruction::DaoStrategyDepositSol(amount)
            .try_to_vec()
            .unwrap(),
    }
}

/// Creates instructions required to deposit SOL directly into a stake pool with existing DAO`s community tokens strategy.
/// This instructions is a part of our Referral program and must include a whitelisted referral.
pub fn dao_strategy_deposit_sol_with_referrer(
    program_id: &Pubkey,
    stake_pool: &Pubkey,
    stake_pool_withdraw_authority: &Pubkey,
    reserve_stake_account: &Pubkey,
    lamports_from: &Pubkey,
    pool_tokens_to: &Pubkey,
    dao_community_tokens_to: &Pubkey,
    manager_fee_account: &Pubkey,
    referrer_sol_account: &Pubkey,
    referrer_list_account: &Pubkey,
    metrics_deposit_referrer_dto: &Pubkey,
    metrics_deposit_referrer_counter_dto: &Pubkey,
    pool_mint: &Pubkey,
    token_program_id: &Pubkey,
    community_token_staking_rewards_dto: &Pubkey,
    owner_wallet: &Pubkey,
    community_token_dto_pubkey: &Pubkey,
    amount: u64,
) -> Instruction {
    let accounts = vec![
        AccountMeta::new(*stake_pool, false),
        AccountMeta::new_readonly(*stake_pool_withdraw_authority, false),
        AccountMeta::new(*reserve_stake_account, false),
        AccountMeta::new(*lamports_from, true),
        AccountMeta::new(*pool_tokens_to, false),
        AccountMeta::new_readonly(*dao_community_tokens_to, false),
        AccountMeta::new(*manager_fee_account, false),
        AccountMeta::new(*referrer_sol_account, false),
        AccountMeta::new_readonly(*referrer_list_account, false),
        AccountMeta::new(*metrics_deposit_referrer_dto, false),
        AccountMeta::new(*metrics_deposit_referrer_counter_dto, false),
        AccountMeta::new(*pool_mint, false),
        AccountMeta::new_readonly(system_program::id(), false),
        AccountMeta::new_readonly(sysvar::rent::id(), false),
        AccountMeta::new_readonly(*token_program_id, false),
        AccountMeta::new(*community_token_staking_rewards_dto, false),
        AccountMeta::new_readonly(*owner_wallet, true),
        AccountMeta::new_readonly(*community_token_dto_pubkey, false),
    ];
    Instruction {
        program_id: *program_id,
        accounts,
        data: StakePoolInstruction::DaoStrategyDepositSolWithReferrer(amount)
            .try_to_vec()
            .unwrap(),
    }
}

/// Creates instructions required to deposit SOL directly into a stake pool with existing DAO`s community tokens strategy.
/// This instructions is a part of our Referral program v2 and must include a whitelisted referral.
pub fn dao_strategy_deposit_sol_with_referrer2(
    program_id: &Pubkey,
    stake_pool: &Pubkey,
    stake_pool_withdraw_authority: &Pubkey,
    reserve_stake_account: &Pubkey,
    lamports_from: &Pubkey,
    pool_tokens_to: &Pubkey,
    dao_community_tokens_to: &Pubkey,
    manager_fee_account: &Pubkey,
    referrer_sol_account: &Pubkey,
    referrer_list_account: &Pubkey,
    pool_mint: &Pubkey,
    token_program_id: &Pubkey,
    community_token_staking_rewards_dto: &Pubkey,
    owner_wallet: &Pubkey,
    community_token_dto_pubkey: &Pubkey,
    amount: u64,
) -> Instruction {
    let accounts = vec![
        AccountMeta::new(*stake_pool, false),
        AccountMeta::new_readonly(*stake_pool_withdraw_authority, false),
        AccountMeta::new(*reserve_stake_account, false),
        AccountMeta::new(*lamports_from, true),
        AccountMeta::new(*pool_tokens_to, false),
        AccountMeta::new_readonly(*dao_community_tokens_to, false),
        AccountMeta::new(*manager_fee_account, false),
        AccountMeta::new(*referrer_sol_account, false),
        AccountMeta::new_readonly(*referrer_list_account, false),
        AccountMeta::new(*pool_mint, false),
        AccountMeta::new_readonly(system_program::id(), false),
        AccountMeta::new_readonly(*token_program_id, false),
        AccountMeta::new(*community_token_staking_rewards_dto, false),
        AccountMeta::new_readonly(*owner_wallet, true),
        AccountMeta::new_readonly(*community_token_dto_pubkey, false),
    ];
    Instruction {
        program_id: *program_id,
        accounts,
        data: StakePoolInstruction::DaoStrategyDepositSolWithReferrer2(amount)
            .try_to_vec()
            .unwrap(),
    }
}

/// Creates instructions required to deposit SOL directly into a stake pool with existing DAO`s community tokens strategy.
/// The difference with `deposit_sol()` is that a deposit
/// authority must sign this instruction.
pub fn dao_strategy_deposit_sol_with_authority(
    program_id: &Pubkey,
    stake_pool: &Pubkey,
    sol_deposit_authority: &Pubkey,
    stake_pool_withdraw_authority: &Pubkey,
    reserve_stake_account: &Pubkey,
    lamports_from: &Pubkey,
    pool_tokens_to: &Pubkey,
    dao_community_tokens_to: &Pubkey,
    manager_fee_account: &Pubkey,
    pool_mint: &Pubkey,
    token_program_id: &Pubkey,
    community_token_staking_rewards_dto: &Pubkey,
    owner_wallet: &Pubkey,
    community_token_dto_pubkey: &Pubkey,
    amount: u64,
) -> Instruction {
    let accounts = vec![
        AccountMeta::new(*stake_pool, false),
        AccountMeta::new_readonly(*stake_pool_withdraw_authority, false),
        AccountMeta::new(*reserve_stake_account, false),
        AccountMeta::new(*lamports_from, true),
        AccountMeta::new(*pool_tokens_to, false),
        AccountMeta::new_readonly(*dao_community_tokens_to, false),
        AccountMeta::new(*manager_fee_account, false),
        AccountMeta::new(*manager_fee_account, false),
        AccountMeta::new(*pool_mint, false),
        AccountMeta::new_readonly(system_program::id(), false),
        AccountMeta::new_readonly(*token_program_id, false),
        AccountMeta::new(*community_token_staking_rewards_dto, false),
        AccountMeta::new_readonly(*owner_wallet, true),
        AccountMeta::new_readonly(*community_token_dto_pubkey, false),
        AccountMeta::new_readonly(*sol_deposit_authority, true),
    ];
    Instruction {
        program_id: *program_id,
        accounts,
        data: StakePoolInstruction::DaoStrategyDepositSol(amount)
            .try_to_vec()
            .unwrap(),
    }
}

/// Creates instructions required to deposit SOL directly into a stake pool with existing DAO`s community tokens strategy.
/// This instructions is a part of our Referral program and must include a whitelisted referral.
/// The difference with `deposit_sol_with_referrer()` is that a deposit
/// authority must sign this instruction.
/// 
pub fn dao_strategy_deposit_sol_with_authority_and_referrer(
    program_id: &Pubkey,
    stake_pool: &Pubkey,
    sol_deposit_authority: &Pubkey,
    stake_pool_withdraw_authority: &Pubkey,
    reserve_stake_account: &Pubkey,
    lamports_from: &Pubkey,
    pool_tokens_to: &Pubkey,
    dao_community_tokens_to: &Pubkey,
    manager_fee_account: &Pubkey,
    referrer_sol_account: &Pubkey,
    referrer_list_account: &Pubkey,
    metrics_deposit_referrer_dto: &Pubkey,
    metrics_deposit_referrer_counter_dto: &Pubkey,
    pool_mint: &Pubkey,
    token_program_id: &Pubkey,
    community_token_staking_rewards_dto: &Pubkey,
    owner_wallet: &Pubkey,
    community_token_dto_pubkey: &Pubkey,
    amount: u64,
) -> Instruction {
    let accounts = vec![
        AccountMeta::new(*stake_pool, false),
        AccountMeta::new_readonly(*stake_pool_withdraw_authority, false),
        AccountMeta::new(*reserve_stake_account, false),
        AccountMeta::new(*lamports_from, true),
        AccountMeta::new(*pool_tokens_to, false),
        AccountMeta::new_readonly(*dao_community_tokens_to, false),
        AccountMeta::new(*manager_fee_account, false),
        AccountMeta::new(*referrer_sol_account, false),
        AccountMeta::new_readonly(*referrer_list_account, false),
        AccountMeta::new(*metrics_deposit_referrer_dto, false),
        AccountMeta::new(*metrics_deposit_referrer_counter_dto, false),
        AccountMeta::new(*pool_mint, false),
        AccountMeta::new_readonly(system_program::id(), false),
        AccountMeta::new_readonly(sysvar::rent::id(), false),
        AccountMeta::new_readonly(*token_program_id, false),
        AccountMeta::new(*community_token_staking_rewards_dto, false),
        AccountMeta::new_readonly(*owner_wallet, true),
        AccountMeta::new_readonly(*community_token_dto_pubkey, false),
        AccountMeta::new_readonly(*sol_deposit_authority, true),
    ];
    Instruction {
        program_id: *program_id,
        accounts,
        data: StakePoolInstruction::DaoStrategyDepositSolWithReferrer(amount)
            .try_to_vec()
            .unwrap(),
    }
}

/// Creates instructions required to deposit SOL directly into a stake pool with existing DAO`s community tokens strategy.
/// This instructions is a part of our Referral program and must include a whitelisted referral.
/// The difference with `deposit_sol_with_referrer2()` is that a deposit
/// authority must sign this instruction.
/// 
pub fn dao_strategy_deposit_sol_with_authority_and_referrer2(
    program_id: &Pubkey,
    stake_pool: &Pubkey,
    sol_deposit_authority: &Pubkey,
    stake_pool_withdraw_authority: &Pubkey,
    reserve_stake_account: &Pubkey,
    lamports_from: &Pubkey,
    pool_tokens_to: &Pubkey,
    dao_community_tokens_to: &Pubkey,
    manager_fee_account: &Pubkey,
    referrer_sol_account: &Pubkey,
    referrer_list_account: &Pubkey,
    pool_mint: &Pubkey,
    token_program_id: &Pubkey,
    community_token_staking_rewards_dto: &Pubkey,
    owner_wallet: &Pubkey,
    community_token_dto_pubkey: &Pubkey,
    amount: u64,
) -> Instruction {
    let accounts = vec![
        AccountMeta::new(*stake_pool, false),
        AccountMeta::new_readonly(*stake_pool_withdraw_authority, false),
        AccountMeta::new(*reserve_stake_account, false),
        AccountMeta::new(*lamports_from, true),
        AccountMeta::new(*pool_tokens_to, false),
        AccountMeta::new_readonly(*dao_community_tokens_to, false),
        AccountMeta::new(*manager_fee_account, false),
        AccountMeta::new(*referrer_sol_account, false),
        AccountMeta::new_readonly(*referrer_list_account, false),
        AccountMeta::new(*pool_mint, false),
        AccountMeta::new_readonly(system_program::id(), false),
        AccountMeta::new_readonly(*token_program_id, false),
        AccountMeta::new(*community_token_staking_rewards_dto, false),
        AccountMeta::new_readonly(*owner_wallet, true),
        AccountMeta::new_readonly(*community_token_dto_pubkey, false),
        AccountMeta::new_readonly(*sol_deposit_authority, true),
    ];
    Instruction {
        program_id: *program_id,
        accounts,
        data: StakePoolInstruction::DaoStrategyDepositSolWithReferrer2(amount)
            .try_to_vec()
            .unwrap(),
    }
}

/// Creates instruction required to withdraw SOL directly from a stake pool with existing DAO`s community tokens strategy.
pub fn dao_strategy_withdraw_sol(
    program_id: &Pubkey,
    stake_pool: &Pubkey,
    stake_pool_withdraw_authority: &Pubkey,
    user_transfer_authority: &Pubkey,
    pool_tokens_from: &Pubkey,
    dao_community_tokens_to: &Pubkey,
    reserve_stake_account: &Pubkey,
    lamports_to: &Pubkey,
    manager_fee_account: &Pubkey,
    pool_mint: &Pubkey,
    token_program_id: &Pubkey,
    community_token_staking_rewards_dto: &Pubkey,
    owner_wallet: &Pubkey,
    community_token_dto_pubkey: &Pubkey,
    pool_tokens: u64,
) -> Instruction {
    let accounts = vec![
        AccountMeta::new(*stake_pool, false),
        AccountMeta::new_readonly(*stake_pool_withdraw_authority, false),
        AccountMeta::new_readonly(*user_transfer_authority, true),
        AccountMeta::new(*pool_tokens_from, false),
        AccountMeta::new_readonly(*dao_community_tokens_to, false),
        AccountMeta::new(*reserve_stake_account, false),
        AccountMeta::new(*lamports_to, false),
        AccountMeta::new(*manager_fee_account, false),
        AccountMeta::new(*pool_mint, false),
        AccountMeta::new_readonly(sysvar::clock::id(), false),
        AccountMeta::new_readonly(sysvar::stake_history::id(), false),
        AccountMeta::new_readonly(stake::program::id(), false),
        AccountMeta::new_readonly(*token_program_id, false),
        AccountMeta::new(*community_token_staking_rewards_dto, false),
        AccountMeta::new_readonly(*owner_wallet, true),
        AccountMeta::new_readonly(*community_token_dto_pubkey, false),
    ];
    Instruction {
        program_id: *program_id,
        accounts,
        data: StakePoolInstruction:: DaoStrategyWithdrawSol(pool_tokens)
            .try_to_vec()
            .unwrap(),
    }
}

/// Creates instruction required to withdraw SOL directly from a stake pool with existing DAO`s community tokens strategy.
/// The difference with `withdraw_sol()` is that the sol withdraw authority
/// must sign this instruction.
pub fn dao_strategy_withdraw_sol_with_authority(
    program_id: &Pubkey,
    stake_pool: &Pubkey,
    sol_withdraw_authority: &Pubkey,
    stake_pool_withdraw_authority: &Pubkey,
    user_transfer_authority: &Pubkey,
    pool_tokens_from: &Pubkey,
    dao_community_tokens_to: &Pubkey,
    reserve_stake_account: &Pubkey,
    lamports_to: &Pubkey,
    manager_fee_account: &Pubkey,
    pool_mint: &Pubkey,
    token_program_id: &Pubkey,
    community_token_staking_rewards_dto: &Pubkey,
    owner_wallet: &Pubkey,
    community_token_dto_pubkey: &Pubkey,
    pool_tokens: u64,
) -> Instruction {
    let accounts = vec![
        AccountMeta::new(*stake_pool, false),
        AccountMeta::new_readonly(*stake_pool_withdraw_authority, false),
        AccountMeta::new_readonly(*user_transfer_authority, true),
        AccountMeta::new(*pool_tokens_from, false),
        AccountMeta::new_readonly(*dao_community_tokens_to, false),
        AccountMeta::new(*reserve_stake_account, false),
        AccountMeta::new(*lamports_to, false),
        AccountMeta::new(*manager_fee_account, false),
        AccountMeta::new(*pool_mint, false),
        AccountMeta::new_readonly(sysvar::clock::id(), false),
        AccountMeta::new_readonly(sysvar::stake_history::id(), false),
        AccountMeta::new_readonly(stake::program::id(), false),
        AccountMeta::new_readonly(*token_program_id, false),
        AccountMeta::new(*community_token_staking_rewards_dto, false),
        AccountMeta::new_readonly(*owner_wallet, true),
        AccountMeta::new_readonly(*community_token_dto_pubkey, false),
        AccountMeta::new_readonly(*sol_withdraw_authority, true),
    ];
    Instruction {
        program_id: *program_id,
        accounts,
        data: StakePoolInstruction::DaoStrategyWithdrawSol(pool_tokens)
            .try_to_vec()
            .unwrap(),
    }
}

/// Creates a 'WithdrawStake' instruction.
pub fn dao_strategy_withdraw_stake(
    program_id: &Pubkey,
    stake_pool: &Pubkey,
    validator_list_storage: &Pubkey,
    stake_pool_withdraw: &Pubkey,
    stake_to_split: &Pubkey,
    stake_to_receive: &Pubkey,
    user_stake_authority: &Pubkey,
    user_transfer_authority: &Pubkey,
    user_pool_token_account: &Pubkey,
    manager_fee_account: &Pubkey,
    pool_mint: &Pubkey,
    token_program_id: &Pubkey,
    dao_community_tokens_to: &Pubkey,
    community_token_staking_rewards_dto: &Pubkey,
    owner_wallet: &Pubkey,
    community_token_dto_pubkey: &Pubkey,
    amount: u64,
) -> Instruction {
    let accounts = vec![
        AccountMeta::new(*stake_pool, false),
        AccountMeta::new(*validator_list_storage, false),
        AccountMeta::new_readonly(*stake_pool_withdraw, false),
        AccountMeta::new(*stake_to_split, false),
        AccountMeta::new(*stake_to_receive, false),
        AccountMeta::new_readonly(*user_stake_authority, false),
        AccountMeta::new_readonly(*user_transfer_authority, true),
        AccountMeta::new(*user_pool_token_account, false),
        AccountMeta::new(*manager_fee_account, false),
        AccountMeta::new(*pool_mint, false),
        AccountMeta::new_readonly(sysvar::clock::id(), false),
        AccountMeta::new_readonly(*token_program_id, false),
        AccountMeta::new_readonly(stake::program::id(), false),
        AccountMeta::new_readonly(*dao_community_tokens_to, false),
        AccountMeta::new(*community_token_staking_rewards_dto, false),
        AccountMeta::new_readonly(*owner_wallet, true),
        AccountMeta::new_readonly(*community_token_dto_pubkey, false),
    ];
    Instruction {
        program_id: *program_id,
        accounts,
        data: StakePoolInstruction::DaoStrategyWithdrawStake(amount)
            .try_to_vec()
            .unwrap(),
    }
}

/// Creates instructions required to deposit into a stake pool with existing DAO`s community tokens strategy, given a stake
/// account owned by the user.
pub fn dao_strategy_deposit_stake(
    program_id: &Pubkey,
    stake_pool: &Pubkey,
    validator_list_storage: &Pubkey,
    stake_pool_withdraw_authority: &Pubkey,
    deposit_stake_address: &Pubkey,
    deposit_stake_withdraw_authority: &Pubkey,
    dest_stake_account: &Pubkey,
    reserve_stake_account: &Pubkey,
    pool_tokens_to: &Pubkey,
    manager_fee_account: &Pubkey,
    referrer_pool_tokens_account: &Pubkey,
    pool_mint: &Pubkey,
    token_program_id: &Pubkey,
    dao_community_tokens_to: &Pubkey,
    community_token_staking_rewards_dto: &Pubkey,
    owner_wallet: &Pubkey,
    community_token_dto_pubkey: &Pubkey,
) -> Vec<Instruction> {
    let stake_pool_deposit_authority =
        find_deposit_authority_program_address(program_id, stake_pool).0;
    let accounts = vec![
        AccountMeta::new(*stake_pool, false),
        AccountMeta::new(*validator_list_storage, false),
        AccountMeta::new_readonly(*stake_pool_withdraw_authority, false),
        AccountMeta::new(*deposit_stake_address, false),
        AccountMeta::new(*dest_stake_account, false),
        AccountMeta::new(*reserve_stake_account, false),
        AccountMeta::new(*pool_tokens_to, false),
        AccountMeta::new(*manager_fee_account, false),
        AccountMeta::new(*referrer_pool_tokens_account, false),
        AccountMeta::new(*pool_mint, false),
        AccountMeta::new_readonly(sysvar::clock::id(), false),
        AccountMeta::new_readonly(sysvar::stake_history::id(), false),
        AccountMeta::new_readonly(*token_program_id, false),
        AccountMeta::new_readonly(stake::program::id(), false),
        AccountMeta::new_readonly(*dao_community_tokens_to, false),
        AccountMeta::new(*community_token_staking_rewards_dto, false),
        AccountMeta::new_readonly(*owner_wallet, true),
        AccountMeta::new_readonly(*community_token_dto_pubkey, false),
        AccountMeta::new_readonly(stake_pool_deposit_authority, false),
    ];
    vec![
        stake::instruction::authorize(
            deposit_stake_address,
            deposit_stake_withdraw_authority,
            &stake_pool_deposit_authority,
            stake::state::StakeAuthorize::Staker,
            None,
        ),
        stake::instruction::authorize(
            deposit_stake_address,
            deposit_stake_withdraw_authority,
            &stake_pool_deposit_authority,
            stake::state::StakeAuthorize::Withdrawer,
            None,
        ),
        Instruction {
            program_id: *program_id,
            accounts,
            data: StakePoolInstruction::DaoStrategyDepositStake.try_to_vec().unwrap(),
        },
    ]
}

/// Creates instructions required to deposit into a stake pool with existing DAO`s community tokens strategy, given a stake
/// account owned by the user. The difference with `deposit()` is that a deposit
/// authority must sign this instruction, which is required for private pools.
pub fn dao_strategy_deposit_stake_with_authority(
    program_id: &Pubkey,
    stake_pool: &Pubkey,
    validator_list_storage: &Pubkey,
    stake_pool_withdraw_authority: &Pubkey,
    deposit_stake_address: &Pubkey,
    deposit_stake_withdraw_authority: &Pubkey,
    dest_stake_account: &Pubkey,
    reserve_stake_account: &Pubkey,
    pool_tokens_to: &Pubkey,
    manager_fee_account: &Pubkey,
    referrer_pool_tokens_account: &Pubkey,
    pool_mint: &Pubkey,
    token_program_id: &Pubkey,
    dao_community_tokens_to: &Pubkey,
    community_token_staking_rewards_dto: &Pubkey,
    owner_wallet: &Pubkey,
    community_token_dto_pubkey: &Pubkey,
    stake_pool_deposit_authority: &Pubkey,
) -> Vec<Instruction> {
    let accounts = vec![
        AccountMeta::new(*stake_pool, false),
        AccountMeta::new(*validator_list_storage, false),
        AccountMeta::new_readonly(*stake_pool_withdraw_authority, false),
        AccountMeta::new(*deposit_stake_address, false),
        AccountMeta::new(*dest_stake_account, false),
        AccountMeta::new(*reserve_stake_account, false),
        AccountMeta::new(*pool_tokens_to, false),
        AccountMeta::new(*manager_fee_account, false),
        AccountMeta::new(*referrer_pool_tokens_account, false),
        AccountMeta::new(*pool_mint, false),
        AccountMeta::new_readonly(sysvar::clock::id(), false),
        AccountMeta::new_readonly(sysvar::stake_history::id(), false),
        AccountMeta::new_readonly(*token_program_id, false),
        AccountMeta::new_readonly(stake::program::id(), false),
        AccountMeta::new_readonly(*dao_community_tokens_to, false),
        AccountMeta::new(*community_token_staking_rewards_dto, false),
        AccountMeta::new_readonly(*owner_wallet, true),
        AccountMeta::new_readonly(*community_token_dto_pubkey, false),
        AccountMeta::new_readonly(*stake_pool_deposit_authority, true),
    ];
    vec![
        stake::instruction::authorize(
            deposit_stake_address,
            deposit_stake_withdraw_authority,
            stake_pool_deposit_authority,
            stake::state::StakeAuthorize::Staker,
            None,
        ),
        stake::instruction::authorize(
            deposit_stake_address,
            deposit_stake_withdraw_authority,
            stake_pool_deposit_authority,
            stake::state::StakeAuthorize::Withdrawer,
            None,
        ),
        Instruction {
            program_id: *program_id,
            accounts,
            data: StakePoolInstruction::DaoStrategyDepositStake.try_to_vec().unwrap(),
        },
    ]
}

/// Creates instruction required to create account for storing counter for community token staking rewards accounts
pub fn create_community_token_staking_rewards_counter(
    program_id: &Pubkey,
    stake_pool: &Pubkey,
    manager: &Pubkey,
    community_token_staking_rewards_counter_dto: &Pubkey,
    community_token_dto: &Pubkey,
) -> Instruction {
    let accounts = vec![
        AccountMeta::new_readonly(*stake_pool, false),
        AccountMeta::new_readonly(*manager, true),
        AccountMeta::new(*community_token_staking_rewards_counter_dto, false),
        AccountMeta::new_readonly(*community_token_dto, false),
        AccountMeta::new_readonly(sysvar::rent::id(), false),
        AccountMeta::new_readonly(system_program::ID, false),
    ]; 

    Instruction {
        program_id: *program_id,
        accounts,
        data: StakePoolInstruction::CreateCommunityTokenStakingRewardsCounter
            .try_to_vec()
            .unwrap(),
    }
}

/// Creates instruction required to create account for 
/// storing counter metrics of deposit sol transactions with referrer
pub fn create_metrics_deposit_referrer_counter(
    program_id: &Pubkey,
    stake_pool: &Pubkey,
    manager: &Pubkey,
    metrics_deposit_referrer_counter_dto: &Pubkey,
) -> Instruction {
    let accounts = vec![
        AccountMeta::new_readonly(*stake_pool, false),
        AccountMeta::new_readonly(*manager, true),
        AccountMeta::new(*metrics_deposit_referrer_counter_dto, false),
        AccountMeta::new_readonly(sysvar::rent::id(), false),
        AccountMeta::new_readonly(system_program::ID, false),
    ]; 

    Instruction {
        program_id: *program_id,
        accounts,
        data: StakePoolInstruction::CreateMetricsDepositReferrerCounter
            .try_to_vec()
            .unwrap(),
    }
}

/// Creates instructions required to deposit SOL directly into a stake pool.
pub fn mint_community_token(
    program_id: &Pubkey,
    stake_pool: &Pubkey,
    manager: &Pubkey,
    user_wallet: &Pubkey,
    stake_pool_withdraw_authority: &Pubkey,
    dao_community_tokens_to: &Pubkey,
    dao_community_token_mint: &Pubkey,
    community_token_dto: &Pubkey,
    community_tokens_counter_dto: &Pubkey,
    community_token_staking_rewards_dto: &Pubkey,
    token_program_id: &Pubkey,
    amount: u64,
    current_epoch: u64
) -> Instruction {
    let accounts = vec![
        AccountMeta::new_readonly(*stake_pool, false),
        AccountMeta::new_readonly(*manager, true),
        AccountMeta::new_readonly(*user_wallet, false),
        AccountMeta::new_readonly(*stake_pool_withdraw_authority, false),
        AccountMeta::new(*dao_community_tokens_to, false),
        AccountMeta::new(*dao_community_token_mint, false),
        AccountMeta::new_readonly(*community_token_dto, false),
        AccountMeta::new(*community_tokens_counter_dto, false),
        AccountMeta::new(*community_token_staking_rewards_dto, false),
        AccountMeta::new_readonly(system_program::id(), false),
        AccountMeta::new_readonly(*token_program_id, false),
        AccountMeta::new_readonly(sysvar::clock::id(), false),
    ];
    Instruction {
        program_id: *program_id,
        accounts,
        data: StakePoolInstruction::MintCommunityToken {
            amount,
            current_epoch
        }
            .try_to_vec()
            .unwrap(),
    }
}

/// Creates instruction required to delete account for storing information for DAO`s community tokens destribution strategy
pub fn delete_community_token_staking_rewards(
    program_id: &Pubkey,
    stake_pool: &Pubkey,
    manager: &Pubkey,
    user_wallet: &Pubkey,
    community_token_staking_rewards_dto: &Pubkey,
    pool_mint: &Pubkey,
    user_pool_token_account: &Pubkey,
) -> Instruction {
    let accounts = vec![
        AccountMeta::new_readonly(*stake_pool, false),
        AccountMeta::new_readonly(*manager, true),
        AccountMeta::new_readonly(*user_wallet, false),
        AccountMeta::new(*community_token_staking_rewards_dto, false),
        AccountMeta::new_readonly(*pool_mint, false),
        AccountMeta::new_readonly(*user_pool_token_account, false),
        AccountMeta::new_readonly(system_program::ID, false),
    ]; 
    
    Instruction {
        program_id: *program_id,
        accounts,
        data: StakePoolInstruction::DeleteCommunityTokenStakingRewards
            .try_to_vec()
            .unwrap(),
    }
}

/// Creates instruction for merge of inactive pool's stake with the pool's reserve stake
pub fn merge_inactive_stake(
    program_id: &Pubkey,
    stake_pool: &Pubkey,
    manager: &Pubkey,
    stake_pool_withdraw_authority: &Pubkey,
    stake: &Pubkey,
    reserve_stake_account: &Pubkey,
) -> Instruction {
    let accounts = vec![
        AccountMeta::new(*stake_pool, false),
        AccountMeta::new_readonly(*manager, true),
        AccountMeta::new_readonly(*stake_pool_withdraw_authority, false),
        AccountMeta::new(*stake, false),
        AccountMeta::new(*reserve_stake_account, false),
        AccountMeta::new_readonly(sysvar::clock::id(), false),
        AccountMeta::new_readonly(sysvar::stake_history::id(), false),
        AccountMeta::new_readonly(stake::program::id(), false),
    ]; 
    
    Instruction {
        program_id: *program_id,
        accounts,
        data: StakePoolInstruction::MergeInactiveStake
            .try_to_vec()
            .unwrap(),
    }
}

/// Instruction for removal of flushed metrics accounts
/// Please use it carefully, accounts should be remoed in adequate chunks
pub fn remove_metrics_deposit_referrer(
    program_id: &Pubkey,
    stake_pool: &Pubkey,
    manager: &Pubkey,
    metrics_deposit_referrer_counter: &Pubkey,
    metrics_deposit_referrer_keys: Vec<Pubkey>,
) -> Instruction {
    let mut accounts = vec![
        AccountMeta::new_readonly(*stake_pool, false),
        AccountMeta::new_readonly(*manager, true),
        AccountMeta::new(*metrics_deposit_referrer_counter, false),
        AccountMeta::new_readonly(system_program::ID, false),
    ];
    
    for metrics_deposit_referrer_key in metrics_deposit_referrer_keys {
        accounts.push(AccountMeta::new(metrics_deposit_referrer_key, false))
    }
    
    Instruction {
        program_id: *program_id,
        accounts,
        data: StakePoolInstruction::RemoveMetricsDepositReferrer
            .try_to_vec()
            .unwrap(),
    }
}

/// Create a list for storing stake pool's referrers
pub fn create_referrer_list(
    program_id: &Pubkey,
    stake_pool: &Pubkey,
    manager: &Pubkey,
    referrer_list: &Pubkey,
    max_referrers: u32,
) -> Instruction {
    let accounts = vec![
        AccountMeta::new_readonly(*stake_pool, false),
        AccountMeta::new_readonly(*manager, true),
        AccountMeta::new(*referrer_list, false),
        AccountMeta::new_readonly(sysvar::rent::id(), false),
        AccountMeta::new_readonly(system_program::ID, false),
    ]; 

    Instruction {
        program_id: *program_id,
        accounts,
        data: StakePoolInstruction::CreateReferrerList(max_referrers)
            .try_to_vec()
            .unwrap(),
    }
}

/// Add a referrer to the list of stake pool's referrers
pub fn add_referrer(
    program_id: &Pubkey,
    stake_pool: &Pubkey,
    manager: &Pubkey,
    referrer_list: &Pubkey,
    referrer: &Pubkey,
) -> Instruction {
    let accounts = vec![
        AccountMeta::new_readonly(*stake_pool, false),
        AccountMeta::new_readonly(*manager, true),
        AccountMeta::new(*referrer_list, false),
        AccountMeta::new_readonly(*referrer, false),        
    ]; 

    Instruction {
        program_id: *program_id,
        accounts,
        data: StakePoolInstruction::AddReferrer
            .try_to_vec()
            .unwrap(),
    }
}

/// Remove a referrer from the stake pool's referrers
pub fn remove_referrer(
    program_id: &Pubkey,
    stake_pool: &Pubkey,
    manager: &Pubkey,
    referrer_list: &Pubkey,
    referrer: &Pubkey,
) -> Instruction {
    let accounts = vec![
        AccountMeta::new_readonly(*stake_pool, false),
        AccountMeta::new_readonly(*manager, true),
        AccountMeta::new(*referrer_list, false),
        AccountMeta::new_readonly(*referrer, false),
    ];

    Instruction {
        program_id: *program_id,
        accounts,
        data: StakePoolInstruction::RemoveReferrer
            .try_to_vec()
            .unwrap(),
    }
}

/// Update or create pool or community token metadata
pub fn update_token_metadata(
    program_id: &Pubkey,
    stake_pool: &Pubkey,
    manager: &Pubkey,
    stake_pool_withdraw_authority: &Pubkey,
    token_mint: &Pubkey,
    metadata_key: &Pubkey,
    name: &str,
    symbol: &str,
    uri: &str,
) -> Instruction {
    let accounts = vec![
        AccountMeta::new_readonly(*stake_pool, false),
        AccountMeta::new_readonly(*manager, true),
        AccountMeta::new_readonly(*stake_pool_withdraw_authority, false),
        AccountMeta::new_readonly(*token_mint, false),
        AccountMeta::new(*metadata_key, false),
        AccountMeta::new_readonly(system_program::ID, false), 
        AccountMeta::new_readonly(sysvar::rent::id(), false),
        AccountMeta::new_readonly(mpl_token_metadata::id(), false),
    ];

    Instruction {
        program_id: *program_id,
        accounts,
        data: StakePoolInstruction::UpdateTokenMetadata { 
            name: name.to_string(),
            symbol: symbol.to_string(),
            uri: uri.to_string() 
        }
            .try_to_vec()
            .unwrap(),
    }
}
