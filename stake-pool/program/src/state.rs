//! State transition types

use spl_token::state::{Account, AccountState};
use {
    crate::{
        big_vec::BigVec, error::StakePoolError, MAX_WITHDRAWAL_FEE_INCREASE,
        WITHDRAWAL_BASELINE_FEE,
    },
    borsh::{BorshDeserialize, BorshSchema, BorshSerialize},
    num_derive::FromPrimitive,
    num_traits::FromPrimitive,
    solana_program::{
        account_info::AccountInfo,
        borsh::get_instance_packed_len,
        msg,
        program_error::ProgramError,
        program_memory::sol_memcmp,
        program_pack::{Pack, Sealed},
        pubkey::{Pubkey, PUBKEY_BYTES},
        stake::state::Lockup,
    },
    std::{convert::TryFrom, fmt, matches},
};

/// Enum representing the account type managed by the program
#[derive(Clone, Debug, PartialEq, BorshDeserialize, BorshSerialize, BorshSchema)]
pub enum AccountType {
    /// If the account has not been initialized, the enum will be 0
    Uninitialized,
    /// Stake pool
    StakePool,
    /// Validator stake list
    ValidatorList,
    /// Referrer list
    ReferrerList,
}

impl Default for AccountType {
    fn default() -> Self {
        AccountType::Uninitialized
    }
}

/// Initialized program details.
#[repr(C)]
#[derive(Clone, Debug, Default, PartialEq, BorshDeserialize, BorshSerialize, BorshSchema)]
pub struct StakePool {
    /// Account type, must be StakePool currently
    pub account_type: AccountType,

    /// Manager authority, allows for updating the staker, manager, and fee account
    pub manager: Pubkey,

    /// Staker authority, allows for adding and removing validators, and managing stake
    /// distribution
    pub staker: Pubkey,

    /// Stake deposit authority
    ///
    /// If a depositor pubkey is specified on initialization, then deposits must be
    /// signed by this authority. If no deposit authority is specified,
    /// then the stake pool will default to the result of:
    /// `Pubkey::find_program_address(
    ///     &[&stake_pool_address.to_bytes()[..32], b"deposit"],
    ///     program_id,
    /// )`
    pub stake_deposit_authority: Pubkey,

    /// Stake withdrawal authority bump seed
    /// for `create_program_address(&[state::StakePool account, "withdrawal"])`
    pub stake_withdraw_bump_seed: u8,

    /// Validator stake list storage account
    pub validator_list: Pubkey,

    /// Reserve stake account, holds deactivated stake
    pub reserve_stake: Pubkey,

    /// Pool Mint
    pub pool_mint: Pubkey,

    /// Manager fee account
    pub manager_fee_account: Pubkey,

    /// Pool token program id
    pub token_program_id: Pubkey,

    /// Total stake under management.
    /// Note that if `last_update_epoch` does not match the current epoch then
    /// this field may not be accurate
    pub total_lamports: u64,

    /// Total supply of pool tokens (should always match the supply in the Pool Mint)
    pub pool_token_supply: u64,

    /// Last epoch the `total_lamports` field was updated
    pub last_update_epoch: u64,

    /// Lockup that all stakes in the pool must have
    pub lockup: Lockup,

    /// Fee taken as a proportion of rewards each epoch
    pub epoch_fee: Fee,

    /// Fee for next epoch
    pub next_epoch_fee: Option<Fee>,

    /// Preferred deposit validator vote account pubkey
    pub preferred_deposit_validator_vote_address: Option<Pubkey>,

    /// Preferred withdraw validator vote account pubkey
    pub preferred_withdraw_validator_vote_address: Option<Pubkey>,

    /// Fee assessed on stake deposits
    pub stake_deposit_fee: Fee,

    /// Fee assessed on withdrawals
    pub stake_withdrawal_fee: Fee,

    /// Future stake withdrawal fee, to be set for the following epoch
    pub next_stake_withdrawal_fee: Option<Fee>,

    /// Fees paid out to referrers on referred stake deposits.
    /// Expressed as a percentage (0 - 100) of deposit fees.
    /// i.e. `stake_deposit_fee`% of stake deposited is collected as deposit fees for every deposit
    /// and `stake_referral_fee`% of the collected stake deposit fees is paid out to the referrer
    pub stake_referral_fee: u8,

    /// Toggles whether the `DepositSol` instruction requires a signature from
    /// this `sol_deposit_authority`
    pub sol_deposit_authority: Option<Pubkey>,

    /// Fee assessed on SOL deposits
    pub sol_deposit_fee: Fee,

    /// Fees paid out to referrers on referred SOL deposits.
    /// Expressed as a percentage (0 - 100) of SOL deposit fees.
    /// i.e. `sol_deposit_fee`% of SOL deposited is collected as deposit fees for every deposit
    /// and `sol_referral_fee`% of the collected SOL deposit fees is paid out to the referrer
    pub sol_referral_fee: u8,

    /// Toggles whether the `WithdrawSol` instruction requires a signature from
    /// the `deposit_authority`
    pub sol_withdraw_authority: Option<Pubkey>,

    /// Fee assessed on SOL withdrawals
    pub sol_withdrawal_fee: Fee,

    /// Future SOL withdrawal fee, to be set for the following epoch
    pub next_sol_withdrawal_fee: Option<Fee>,

    /// Last epoch's total pool tokens, used only for APR estimation
    pub last_epoch_pool_token_supply: u64,

    /// Last epoch's total lamports, used only for APR estimation
    pub last_epoch_total_lamports: u64,

    /// Last epoch's exchange rate for SOL deposit and withdraw
    pub rate_of_exchange: Option<RateOfExchange>,

    /// Treasury fee account
    pub treasury_fee_account: Pubkey,

    /// Fee assessed on taking rewards for treasury
    pub treasury_fee: Fee,

    /// Total liquidity in Sol equivalent under management.
    pub total_lamports_liquidity: u64,

    /// Numerator for the Fix APY feature based on the 
    pub max_validator_yield_per_epoch_numerator: u32,

    /// Deposit fee is taken from the deposit amount up to no_fee_deposit_threshold (sol)
    pub no_fee_deposit_threshold: u16,
}
impl StakePool {
    /// 0.060144% - numerator of validator yield per epoch for validator 8% APY with 128 epochs in year.
    pub const DEFAULT_VALIDATOR_YIELD_PER_EPOCH_NUMERATOR: u32 = 60144;
    /// Default number of epochs per year
    pub const DEFAULT_EPOCHS_PER_YEAR: u32 = 128;
    /// 0.060144% - denominator of validator yield per epoch for validator 8% APY with 128 epochs in year.
    pub const VALIDATOR_YIELD_PER_EPOCH_DENOMINATOR: u32 = 100_000_000;

    /// calculate the pool tokens that should be minted from lamports
    #[inline]
    pub fn convert_amount_of_lamports_to_amount_of_pool_tokens(
        &self,
        stake_lamports: u64,
    ) -> Option<u64> {
        match self.rate_of_exchange {
            Some(ref rate_of_exchange) => u64::try_from(
                (stake_lamports as u128)
                    .checked_mul(rate_of_exchange.denominator as u128)?
                    .checked_div(rate_of_exchange.numerator as u128)?,
            )
            .ok(),
            None => Some(stake_lamports),
        }
    }

    /// calculate lamports amount on withdrawal
    #[inline]
    pub fn convert_amount_of_pool_tokens_to_amount_of_lamports(
        &self,
        pool_tokens: u64,
    ) -> Option<u64> {
        match self.rate_of_exchange {
            Some(ref rate_of_exchange) => u64::try_from(
                (pool_tokens as u128)
                    .checked_mul(rate_of_exchange.numerator as u128)?
                    .checked_div(rate_of_exchange.denominator as u128)?,
            )
            .ok(),
            None => Some(pool_tokens),
        }
    }

    /// calculate pool tokens to be deducted as withdrawal fees
    #[inline]
    pub fn calc_pool_tokens_stake_withdrawal_fee(&self, pool_tokens: u64) -> Option<u64> {
        u64::try_from(self.stake_withdrawal_fee.apply(pool_tokens)?).ok()
    }

    /// calculate pool tokens to be deducted as withdrawal fees
    #[inline]
    pub fn calc_pool_tokens_sol_withdrawal_fee(&self, pool_tokens: u64) -> Option<u64> {
        u64::try_from(self.sol_withdrawal_fee.apply(pool_tokens)?).ok()
    }

    /// calculate pool tokens to be deducted as stake deposit fees
    #[inline]
    pub fn calc_pool_tokens_stake_deposit_fee(&self, pool_tokens_minted: u64) -> Option<u64> {
        u64::try_from(self.stake_deposit_fee.apply(pool_tokens_minted)?).ok()
    }

    /// calculate pool tokens to be deducted from deposit fees as referral fees
    #[inline]
    pub fn calc_pool_tokens_stake_referral_fee(&self, stake_deposit_fee: u64) -> Option<u64> {
        u64::try_from(
            (stake_deposit_fee as u128)
                .checked_mul(self.stake_referral_fee as u128)?
                .checked_div(100u128)?,
        )
        .ok()
    }

    /// calculate pool tokens to be deducted as SOL deposit fees
    #[inline]
    pub fn calc_pool_tokens_sol_deposit_fee(&self, pool_tokens_minted: u64) -> Option<u64> {
        u64::try_from(self.sol_deposit_fee.apply(pool_tokens_minted)?).ok()
    }

    /// calculate pool tokens to be deducted from SOL deposit fees as referral fees
    #[inline]
    pub fn calc_pool_tokens_sol_referral_fee(&self, sol_deposit_fee: u64) -> Option<u64> {
        u64::try_from(
            (sol_deposit_fee as u128)
                .checked_mul(self.sol_referral_fee as u128)?
                .checked_div(100u128)?,
        )
        .ok()
    }

    /// calculate SOL to be deducted from SOL deposit as referral fees
    #[inline]
    pub fn calc_sol_referral_fee(&self, deposit_lamports: u64) -> Option<u64> {
        let sol_deposit_fee = self.sol_deposit_fee.apply(deposit_lamports)?;
        u64::try_from(
            (sol_deposit_fee as u128)
                .checked_mul(self.sol_referral_fee as u128)?
                .checked_div(100u128)?,
        )
        .ok()
    }

    /// Calculate the fee in pool tokens that goes to the manager
    #[inline]
    pub fn calc_pool_tokens_epoch_fee(&self, reward_lamports: u64) -> Option<u64> {
        let fee_lamports = self.epoch_fee.apply(reward_lamports)?;

        self.convert_amount_of_lamports_to_amount_of_pool_tokens(u64::try_from(fee_lamports).ok()?)
    }

    /// Calculate the fee in pool tokens that goes to the treasury
    #[inline]
    pub fn calc_pool_tokens_treasury_fee(&self, reward_lamports: u64) -> Option<u64> {
        let fee_lamports = self.treasury_fee.apply(reward_lamports)?;

        self.convert_amount_of_lamports_to_amount_of_pool_tokens(u64::try_from(fee_lamports).ok()?)
    }

    /// Checks that the withdraw or deposit authority is valid
    fn check_program_derived_authority(
        authority_address: &Pubkey,
        program_id: &Pubkey,
        stake_pool_address: &Pubkey,
        authority_seed: &[u8],
        bump_seed: u8,
    ) -> Result<(), ProgramError> {
        let expected_address = Pubkey::create_program_address(
            &[
                &stake_pool_address.to_bytes()[..32],
                authority_seed,
                &[bump_seed],
            ],
            program_id,
        )?;

        if *authority_address == expected_address {
            Ok(())
        } else {
            msg!(
                "Incorrect authority provided, expected {}, received {}",
                expected_address,
                authority_address
            );
            Err(StakePoolError::InvalidProgramAddress.into())
        }
    }

    /// Check if the manager fee info is a valid token program account
    /// capable of receiving tokens from the mint.
    pub(crate) fn check_manager_fee(
        &self,
        manager_fee_info: &AccountInfo,
    ) -> Result<(), ProgramError> {
        let token_account = Account::unpack(&manager_fee_info.data.borrow())?;
        if manager_fee_info.owner != &self.token_program_id
            || *manager_fee_info.key != self.manager_fee_account
            || token_account.state != AccountState::Initialized
            || token_account.mint != self.pool_mint
        {
            msg!("Manager fee account is not owned by token program, is not valid, is not initialized, or does not match stake pool's mint");
            return Err(StakePoolError::InvalidManagerFeeAccount.into());
        }
        Ok(())
    }

    /// Check if the treasury fee info is a valid token program account
    /// capable of receiving tokens from the mint.
    pub(crate) fn check_treasury_fee(
        &self,
        treasury_fee_info: &AccountInfo,
    ) -> Result<(), ProgramError> {
        let token_account = Account::unpack(&treasury_fee_info.data.borrow())?;
        if treasury_fee_info.owner != &self.token_program_id
            || *treasury_fee_info.key != self.treasury_fee_account
            || token_account.state != AccountState::Initialized
            || token_account.mint != self.pool_mint
        {
            msg!("Treasury fee account is not owned by token program, is not valid, is not initialized, or does not match stake pool's mint");
            return Err(StakePoolError::InvalidTreasuryFeeAccount.into());
        }
        Ok(())
    }

    /// Checks that the withdraw authority is valid
    #[inline]
    pub(crate) fn check_authority_withdraw(
        &self,
        withdraw_authority: &Pubkey,
        program_id: &Pubkey,
        stake_pool_address: &Pubkey,
    ) -> Result<(), ProgramError> {
        Self::check_program_derived_authority(
            withdraw_authority,
            program_id,
            stake_pool_address,
            crate::AUTHORITY_WITHDRAW,
            self.stake_withdraw_bump_seed,
        )
    }
    /// Checks that the deposit authority is valid
    #[inline]
    pub(crate) fn check_stake_deposit_authority(
        &self,
        stake_deposit_authority: &Pubkey,
    ) -> Result<(), ProgramError> {
        if self.stake_deposit_authority == *stake_deposit_authority {
            Ok(())
        } else {
            Err(StakePoolError::InvalidStakeDepositAuthority.into())
        }
    }

    /// Checks that the deposit authority is valid
    /// Does nothing if `sol_deposit_authority` is currently not set
    #[inline]
    pub(crate) fn check_sol_deposit_authority(
        &self,
        maybe_sol_deposit_authority: Result<&AccountInfo, ProgramError>,
    ) -> Result<(), ProgramError> {
        if let Some(auth) = self.sol_deposit_authority {
            let sol_deposit_authority = maybe_sol_deposit_authority?;
            if auth != *sol_deposit_authority.key {
                msg!("Expected {}, received {}", auth, sol_deposit_authority.key);
                return Err(StakePoolError::InvalidSolDepositAuthority.into());
            }
            if !sol_deposit_authority.is_signer {
                msg!("SOL Deposit authority signature missing");
                return Err(StakePoolError::SignatureMissing.into());
            }
        }
        Ok(())
    }

    /// Checks that the sol withdraw authority is valid
    /// Does nothing if `sol_withdraw_authority` is currently not set
    #[inline]
    pub(crate) fn check_sol_withdraw_authority(
        &self,
        maybe_sol_withdraw_authority: Result<&AccountInfo, ProgramError>,
    ) -> Result<(), ProgramError> {
        if let Some(auth) = self.sol_withdraw_authority {
            let sol_withdraw_authority = maybe_sol_withdraw_authority?;
            if auth != *sol_withdraw_authority.key {
                return Err(StakePoolError::InvalidSolWithdrawAuthority.into());
            }
            if !sol_withdraw_authority.is_signer {
                msg!("SOL withdraw authority signature missing");
                return Err(StakePoolError::SignatureMissing.into());
            }
        }
        Ok(())
    }

    /// Check mint is correct
    #[inline]
    pub(crate) fn check_mint(&self, mint_info: &AccountInfo) -> Result<(), ProgramError> {
        if *mint_info.key != self.pool_mint {
            Err(StakePoolError::WrongPoolMint.into())
        } else {
            Ok(())
        }
    }

    /// Check manager validity and signature
    pub(crate) fn check_manager(&self, manager_info: &AccountInfo) -> Result<(), ProgramError> {
        if *manager_info.key != self.manager {
            msg!(
                "Incorrect manager provided, expected {}, received {}",
                self.manager,
                manager_info.key
            );
            return Err(StakePoolError::WrongManager.into());
        }
        if !manager_info.is_signer {
            msg!("Manager signature missing");
            return Err(StakePoolError::SignatureMissing.into());
        }
        Ok(())
    }

    /// Check staker validity and signature
    pub(crate) fn check_staker(&self, staker_info: &AccountInfo) -> Result<(), ProgramError> {
        if *staker_info.key != self.staker {
            msg!(
                "Incorrect staker provided, expected {}, received {}",
                self.staker,
                staker_info.key
            );
            return Err(StakePoolError::WrongStaker.into());
        }
        if !staker_info.is_signer {
            msg!("Staker signature missing");
            return Err(StakePoolError::SignatureMissing.into());
        }
        Ok(())
    }

    /// Check the validator list is valid
    pub fn check_validator_list(
        &self,
        validator_list_info: &AccountInfo,
    ) -> Result<(), ProgramError> {
        if *validator_list_info.key != self.validator_list {
            msg!(
                "Invalid validator list provided, expected {}, received {}",
                self.validator_list,
                validator_list_info.key
            );
            Err(StakePoolError::InvalidValidatorStakeList.into())
        } else {
            Ok(())
        }
    }

    /// Check the reserve stake is valid
    pub fn check_reserve_stake(
        &self,
        reserve_stake_info: &AccountInfo,
    ) -> Result<(), ProgramError> {
        if *reserve_stake_info.key != self.reserve_stake {
            msg!(
                "Invalid reserve stake provided, expected {}, received {}",
                self.reserve_stake,
                reserve_stake_info.key
            );
            Err(StakePoolError::InvalidProgramAddress.into())
        } else {
            Ok(())
        }
    }

        /// Check the referrer list is valid
        pub fn check_referrer_list(
            &self,
            referrer_list_dto_info: &AccountInfo,
            program_id: &Pubkey,
            stake_pool_address: &Pubkey,
        ) -> Result<(), ProgramError> {
            let referrer_list_address = ReferrerList::find_address(program_id, stake_pool_address).0;
            if *referrer_list_dto_info.key != referrer_list_address {
                msg!(
                    "Invalid referrer list provided, expected {}, received {}",
                    referrer_list_address,
                    referrer_list_dto_info.key
                );
                Err(StakePoolError::InvalidProgramAddress.into())
            } else {
                Ok(())
            }
        }

    /// Check if StakePool is actually initialized as a stake pool
    pub fn is_valid(&self) -> bool {
        self.account_type == AccountType::StakePool
    }

    /// Check if StakePool is currently uninitialized
    pub fn is_uninitialized(&self) -> bool {
        self.account_type == AccountType::Uninitialized
    }

    /// Updates one of the StakePool's fees.
    pub fn update_fee(&mut self, fee: &FeeType) -> Result<(), StakePoolError> {
        match fee {
            FeeType::SolReferral(new_fee) => self.sol_referral_fee = *new_fee,
            FeeType::StakeReferral(new_fee) => self.stake_referral_fee = *new_fee,
            FeeType::Epoch(new_fee) => self.next_epoch_fee = Some(*new_fee),
            FeeType::StakeWithdrawal(new_fee) => {
                new_fee.check_withdrawal(&self.stake_withdrawal_fee)?;
                self.next_stake_withdrawal_fee = Some(*new_fee)
            }
            FeeType::SolWithdrawal(new_fee) => {
                new_fee.check_withdrawal(&self.sol_withdrawal_fee)?;
                self.next_sol_withdrawal_fee = Some(*new_fee)
            }
            FeeType::SolDeposit(new_fee) => self.sol_deposit_fee = *new_fee,
            FeeType::StakeDeposit(new_fee) => self.stake_deposit_fee = *new_fee,
            FeeType::Treasury(new_fee) => self.treasury_fee = *new_fee,
        };
        Ok(())
    }

    /// Calculates the amount of SOL that the user would have paid before applying the APY retention strategy. 
    /// From this amount, you need to take DepositFee, if it is installed.
    pub fn calculate_deposit_amount_by_reward_simulation(&self, amount: u64) -> Option<u64> {
        let numerator = if self.max_validator_yield_per_epoch_numerator == 0 {
            Self::DEFAULT_VALIDATOR_YIELD_PER_EPOCH_NUMERATOR
        } else {
            self.max_validator_yield_per_epoch_numerator
        };
        u64::try_from(
            (amount as u128)
                .checked_mul((Self::VALIDATOR_YIELD_PER_EPOCH_DENOMINATOR - numerator) as u128)?
                .checked_div(Self::VALIDATOR_YIELD_PER_EPOCH_DENOMINATOR as u128)?,

        ).ok()
    }
}

/// Referrer type
#[repr(C)]
#[derive(Clone, Debug, Default, PartialEq, BorshDeserialize, BorshSerialize, BorshSchema)]
pub struct Referrer {
    /// Referrer's public key
    pub key: Pubkey,
}

impl Referrer {
    /// Create an instance of the Refferer structure
    pub fn new(key: Pubkey) -> Self {
        Self { key }
    }

    /// Performs a very cheap comparison, for checking if this referrer pubkey
    /// matches the provided pubkey
    pub fn memcmp_pubkey(data: &[u8], referrer_address_bytes: &[u8]) -> bool {
        sol_memcmp(
            &data[0..0 + PUBKEY_BYTES],
            referrer_address_bytes,
            PUBKEY_BYTES,
        ) == 0
    }

    /// Check whether the provided Referrer is not default
    pub fn is_not_default(data: &[u8]) -> bool {
        sol_memcmp(
            &data[0..0 + PUBKEY_BYTES],
            Pubkey::default().as_ref(),
            PUBKEY_BYTES,
        ) != 0        
    }
}

impl Sealed for Referrer {}

impl Pack for Referrer {
    const LEN: usize = 32;
    fn pack_into_slice(&self, data: &mut [u8]) {
        let mut data = data;
        self.serialize(&mut data).unwrap();
    }
    fn unpack_from_slice(src: &[u8]) -> Result<Self, ProgramError> {
        let unpacked = Self::try_from_slice(src)?;
        Ok(unpacked)
    }
}

/// Storage list for all referrals in the pool.
#[repr(C)]
#[derive(Clone, Debug, Default, PartialEq, BorshDeserialize, BorshSerialize, BorshSchema)]
pub struct ReferrerList {
    /// Data outside of the referrer list, separated out for cheaper deserializations
    pub header: ReferrerListHeader,

    /// List of referrers in the pool
    pub referrers: Vec<Referrer>,
}

/// Helper type to deserialize just the start of a ValidatorList
#[repr(C)]
#[derive(Clone, Debug, Default, PartialEq, BorshDeserialize, BorshSerialize, BorshSchema)]
pub struct ReferrerListHeader {
    /// Account type, must be ReferrerList currently
    pub account_type: AccountType,

    /// Maximum allowable number of referrers
    pub max_referrers: u32,
}

impl ReferrerList {
    const SEED_PREFIX: &'static [u8] = b"referrer_list";

    /// Create an empty instance containing space for `max_validators` and preferred validator keys
    pub fn new(max_referrers: u32) -> Self {
        Self {
            header: ReferrerListHeader {
                account_type: AccountType::ReferrerList,
                max_referrers,
            },
            referrers: vec![Referrer::default(); max_referrers as usize],
        }
    }

    /// Check if contains referrer with particular pubkey
    pub fn contains(&self, referrer_address: &Pubkey) -> bool {
        self.referrers
            .iter()
            .any(|x| x.key == *referrer_address)
    }
}

impl SimplePda for ReferrerList {
    fn get_seed_prefix() -> &'static [u8] {
        return Self::SEED_PREFIX;
    }
}

impl ReferrerListHeader {
    #[allow(dead_code)]
    const LEN: usize = 1 + 4;

    /// Check if referrer list is actually initialized as a referrer list
    pub fn is_valid(&self) -> bool {
        self.account_type == AccountType::ReferrerList
    }

    /// Check if referrer list is uninitialized
    pub fn is_uninitialized(&self) -> bool {
        self.account_type == AccountType::Uninitialized
    }

    /// Extracts a slice of Pubkey from the vec part
    /// of the ReferrerList
    pub fn deserialize_mut_slice(
        data: &mut [u8],
        skip: usize,
        len: usize,
    ) -> Result<(Self, Vec<&mut Referrer>), ProgramError> {
        let (header, mut big_vec) = Self::deserialize_vec(data)?;
        let referrer_list = big_vec.deserialize_mut_slice::<Referrer>(skip, len)?;
        Ok((header, referrer_list))
    }

    /// Extracts the referrer list into its header and internal BigVec
    pub fn deserialize_vec(data: &mut [u8]) -> Result<(Self, BigVec), ProgramError> {
        let mut data_mut = &data[..];
        let header = ReferrerListHeader::deserialize(&mut data_mut)?;
        let length = get_instance_packed_len(&header)?;

        let big_vec = BigVec {
            data: &mut data[length..],
        };
        Ok((header, big_vec))
    }
}

/// Storage list for all validator stake accounts in the pool.
#[repr(C)]
#[derive(Clone, Debug, Default, PartialEq, BorshDeserialize, BorshSerialize, BorshSchema)]
pub struct ValidatorList {
    /// Data outside of the validator list, separated out for cheaper deserializations
    pub header: ValidatorListHeader,

    /// List of stake info for each validator in the pool
    pub validators: Vec<ValidatorStakeInfo>,
}

/// Helper type to deserialize just the start of a ValidatorList
#[repr(C)]
#[derive(Clone, Debug, Default, PartialEq, BorshDeserialize, BorshSerialize, BorshSchema)]
pub struct ValidatorListHeader {
    /// Account type, must be ValidatorList currently
    pub account_type: AccountType,

    /// Maximum allowable number of validators
    pub max_validators: u32,
}

/// Status of the stake account in the validator list, for accounting
#[derive(
    FromPrimitive, Copy, Clone, Debug, PartialEq, BorshDeserialize, BorshSerialize, BorshSchema,
)]
pub enum StakeStatus {
    /// Stake account is active, there may be a transient stake as well
    Active,
    /// Only transient stake account exists, when a transient stake is
    /// deactivating during validator removal
    DeactivatingTransient,
    /// No more validator stake accounts exist, entry ready for removal during
    /// `UpdateStakePoolBalance`
    ReadyForRemoval,
}

impl Default for StakeStatus {
    fn default() -> Self {
        Self::Active
    }
}

/// Information about a validator in the pool
///
/// NOTE: ORDER IS VERY IMPORTANT HERE, PLEASE DO NOT RE-ORDER THE FIELDS UNLESS
/// THERE'S AN EXTREMELY GOOD REASON.
///
/// To save on BPF instructions, the serialized bytes are reinterpreted with an
/// unsafe pointer cast, which means that this structure cannot have any
/// undeclared alignment-padding in its representation.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, BorshDeserialize, BorshSerialize, BorshSchema)]
pub struct ValidatorStakeInfo {
    /// Amount of active stake delegated to this validator, minus the minimum
    /// required stake amount of rent-exemption + `crate::MINIMUM_ACTIVE_STAKE`
    /// (currently 0.001 SOL).
    ///
    /// Note that if `last_update_epoch` does not match the current epoch then
    /// this field may not be accurate
    pub active_stake_lamports: u64,

    /// Amount of transient stake delegated to this validator
    ///
    /// Note that if `last_update_epoch` does not match the current epoch then
    /// this field may not be accurate
    pub transient_stake_lamports: u64,

    /// Last epoch the active and transient stake lamports fields were updated
    pub last_update_epoch: u64,

    /// Start of the validator transient account seed suffixess
    pub transient_seed_suffix_start: u64,

    /// End of the validator transient account seed suffixes
    pub transient_seed_suffix_end: u64,

    /// Status of the validator stake account
    pub status: StakeStatus,

    /// Validator vote account address
    pub vote_account_address: Pubkey,
}

impl ValidatorStakeInfo {
    /// Get the total lamports delegated to this validator (active and transient)
    pub fn stake_lamports(&self) -> u64 {
        self.active_stake_lamports
            .checked_add(self.transient_stake_lamports)
            .unwrap()
    }

    /// Performs a very cheap comparison, for checking if this validator stake
    /// info matches the vote account address
    pub fn memcmp_pubkey(data: &[u8], vote_address_bytes: &[u8]) -> bool {
        sol_memcmp(
            &data[41..41 + PUBKEY_BYTES],
            vote_address_bytes,
            PUBKEY_BYTES,
        ) == 0
    }

    /// Performs a very cheap comparison, for checking if this validator stake
    /// info does not have active lamports equal to the given bytes
    pub fn active_lamports_not_equal(data: &[u8], lamports_le_bytes: &[u8]) -> bool {
        sol_memcmp(&data[0..8], lamports_le_bytes, 8) != 0
    }

    /// Performs a very cheap comparison, for checking if this validator stake
    /// info does not have lamports equal to the given bytes
    pub fn transient_lamports_not_equal(data: &[u8], lamports_le_bytes: &[u8]) -> bool {
        sol_memcmp(&data[8..16], lamports_le_bytes, 8) != 0
    }

    /// Check that the validator stake info is valid
    pub fn is_not_removed(data: &[u8]) -> bool {
        FromPrimitive::from_u8(data[40]) != Some(StakeStatus::ReadyForRemoval)
    }
}

impl Sealed for ValidatorStakeInfo {}

impl Pack for ValidatorStakeInfo {
    const LEN: usize = 73;
    fn pack_into_slice(&self, data: &mut [u8]) {
        let mut data = data;
        self.serialize(&mut data).unwrap();
    }
    fn unpack_from_slice(src: &[u8]) -> Result<Self, ProgramError> {
        let unpacked = Self::try_from_slice(src)?;
        Ok(unpacked)
    }
}

impl ValidatorList {
    /// Create an empty instance containing space for `max_validators` and preferred validator keys
    pub fn new(max_validators: u32) -> Self {
        Self {
            header: ValidatorListHeader {
                account_type: AccountType::ValidatorList,
                max_validators,
            },
            validators: vec![ValidatorStakeInfo::default(); max_validators as usize],
        }
    }

    /// Calculate the number of validator entries that fit in the provided length
    pub fn calculate_max_validators(buffer_length: usize) -> usize {
        let header_size = ValidatorListHeader::LEN + 4;
        buffer_length.saturating_sub(header_size) / ValidatorStakeInfo::LEN
    }

    /// Check if contains validator with particular pubkey
    pub fn contains(&self, vote_account_address: &Pubkey) -> bool {
        self.validators
            .iter()
            .any(|x| x.vote_account_address == *vote_account_address)
    }

    /// Check if contains validator with particular pubkey
    pub fn find_mut(&mut self, vote_account_address: &Pubkey) -> Option<&mut ValidatorStakeInfo> {
        self.validators
            .iter_mut()
            .find(|x| x.vote_account_address == *vote_account_address)
    }
    /// Check if contains validator with particular pubkey
    pub fn find(&self, vote_account_address: &Pubkey) -> Option<&ValidatorStakeInfo> {
        self.validators
            .iter()
            .find(|x| x.vote_account_address == *vote_account_address)
    }

    /// Check if the list has any active stake
    pub fn has_active_stake(&self) -> bool {
        self.validators.iter().any(|x| x.active_stake_lamports > 0)
    }
}

impl ValidatorListHeader {
    const LEN: usize = 1 + 4;

    /// Check if validator stake list is actually initialized as a validator stake list
    pub fn is_valid(&self) -> bool {
        self.account_type == AccountType::ValidatorList
    }

    /// Check if the validator stake list is uninitialized
    pub fn is_uninitialized(&self) -> bool {
        self.account_type == AccountType::Uninitialized
    }

    /// Extracts a slice of ValidatorStakeInfo types from the vec part
    /// of the ValidatorList
    pub fn deserialize_mut_slice(
        data: &mut [u8],
        skip: usize,
        len: usize,
    ) -> Result<(Self, Vec<&mut ValidatorStakeInfo>), ProgramError> {
        let (header, mut big_vec) = Self::deserialize_vec(data)?;
        let validator_list = big_vec.deserialize_mut_slice::<ValidatorStakeInfo>(skip, len)?;
        Ok((header, validator_list))
    }

    /// Extracts the validator list into its header and internal BigVec
    pub fn deserialize_vec(data: &mut [u8]) -> Result<(Self, BigVec), ProgramError> {
        let mut data_mut = &data[..];
        let header = ValidatorListHeader::deserialize(&mut data_mut)?;
        let length = get_instance_packed_len(&header)?;

        let big_vec = BigVec {
            data: &mut data[length..],
        };
        Ok((header, big_vec))
    }
}

/// Fee rate as a ratio, minted on `UpdateStakePoolBalance` as a proportion of
/// the rewards
/// If either the numerator or the denominator is 0, the fee is considered to be 0
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, BorshSerialize, BorshDeserialize, BorshSchema)]
pub struct Fee {
    /// denominator of the fee ratio
    pub denominator: u64,
    /// numerator of the fee ratio
    pub numerator: u64,
}

impl Fee {
    /// Applies the Fee's rates to a given amount, `amt`
    /// returning the amount to be subtracted from it as fees
    /// (0 if denominator is 0 or amt is 0),
    /// or None if overflow occurs
    #[inline]
    pub fn apply(&self, amt: u64) -> Option<u128> {
        if self.denominator == 0 {
            return Some(0);
        }
        (amt as u128)
            .checked_mul(self.numerator as u128)?
            .checked_div(self.denominator as u128)
    }

    /// Withdrawal fees have some additional restrictions,
    /// this fn checks if those are met, returning an error if not.
    /// Does nothing and returns Ok if fee type is not withdrawal
    pub fn check_withdrawal(&self, old_withdrawal_fee: &Fee) -> Result<(), StakePoolError> {
        // If the previous withdrawal fee was 0, we allow the fee to be set to a
        // maximum of (WITHDRAWAL_BASELINE_FEE * MAX_WITHDRAWAL_FEE_INCREASE)
        let (old_num, old_denom) =
            if old_withdrawal_fee.denominator == 0 || old_withdrawal_fee.numerator == 0 {
                (
                    WITHDRAWAL_BASELINE_FEE.numerator,
                    WITHDRAWAL_BASELINE_FEE.denominator,
                )
            } else {
                (old_withdrawal_fee.numerator, old_withdrawal_fee.denominator)
            };

        // Check that new_fee / old_fee <= MAX_WITHDRAWAL_FEE_INCREASE
        // Program fails if provided numerator or denominator is too large, resulting in overflow
        if (old_num as u128)
            .checked_mul(self.denominator as u128)
            .map(|x| x.checked_mul(MAX_WITHDRAWAL_FEE_INCREASE.numerator as u128))
            .ok_or(StakePoolError::CalculationFailure)?
            < (self.numerator as u128)
                .checked_mul(old_denom as u128)
                .map(|x| x.checked_mul(MAX_WITHDRAWAL_FEE_INCREASE.denominator as u128))
                .ok_or(StakePoolError::CalculationFailure)?
        {
            msg!(
                "Fee increase exceeds maximum allowed, proposed increase factor ({} / {})",
                self.numerator * old_denom,
                old_num * self.denominator,
            );
            return Err(StakePoolError::FeeIncreaseTooHigh);
        }
        Ok(())
    }
}

impl fmt::Display for Fee {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.numerator > 0 && self.denominator > 0 {
            write!(f, "{}/{}", self.numerator, self.denominator)
        } else {
            write!(f, "none")
        }
    }
}

/// The type of fees that can be set on the stake pool
#[derive(Clone, Debug, PartialEq, BorshDeserialize, BorshSerialize, BorshSchema)]
pub enum FeeType {
    /// Referral fees for SOL deposits
    SolReferral(u8),
    /// Referral fees for stake deposits
    StakeReferral(u8),
    /// Management fee paid per epoch
    Epoch(Fee),
    /// Stake withdrawal fee
    StakeWithdrawal(Fee),
    /// Deposit fee for SOL deposits
    SolDeposit(Fee),
    /// Deposit fee for stake deposits
    StakeDeposit(Fee),
    /// SOL withdrawal fee
    SolWithdrawal(Fee),
    /// Fee for treasury from reward
    Treasury(Fee),
}

impl FeeType {
    /// Checks if the provided fee is too high, returning an error if so
    pub fn check_too_high(&self) -> Result<(), StakePoolError> {
        let too_high = match self {
            Self::SolReferral(pct) => *pct > 100u8,
            Self::StakeReferral(pct) => *pct > 100u8,
            Self::Epoch(fee) => fee.numerator > fee.denominator,
            Self::StakeWithdrawal(fee) => fee.numerator > fee.denominator,
            Self::SolWithdrawal(fee) => fee.numerator > fee.denominator,
            Self::SolDeposit(fee) => fee.numerator > fee.denominator,
            Self::StakeDeposit(fee) => fee.numerator > fee.denominator,
            Self::Treasury(fee) => fee.numerator > fee.denominator,
        };
        if too_high {
            msg!("Fee greater than 100%: {:?}", self);
            return Err(StakePoolError::FeeTooHigh);
        }
        Ok(())
    }

    /// Returns if the contained fee can only be updated earliest on the next epoch
    #[inline]
    pub fn can_only_change_next_epoch(&self) -> bool {
        matches!(
            self,
            Self::StakeWithdrawal(_)
                | Self::SolWithdrawal(_)
                | Self::Epoch(_)
        )
    }
}

/// Exchange rate for SOL deposit and withdraw
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, BorshSerialize, BorshDeserialize, BorshSchema)]
pub struct RateOfExchange {
    /// denominator of the fee ratio, total supply of pool tokens
    pub denominator: u64,
    /// numerator of the fee ratio, total lamports under management.
    pub numerator: u64,
}

impl fmt::Display for RateOfExchange {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.numerator > 0 && self.denominator > 0 {
            write!(f, "{}/{}", self.numerator, self.denominator)
        } else {
            write!(f, "none")
        }
    }
}

/// Finds Pda adresses by common simple rule
pub trait SimplePda {
    /// Find PDA 
    fn find_address(
        program_id: &Pubkey,
        stake_pool_address: &Pubkey,
    ) -> (Pubkey, u8) {
        Pubkey::find_program_address(
            &[
                Self::get_seed_prefix(),
                &stake_pool_address.to_bytes()[..],
                &program_id.to_bytes()[..]
            ],
            program_id
        )
    }

     /// Get seed prefix for Pda
    fn get_seed_prefix() -> &'static [u8];
}

/// Stores Ccmmunity token mint.
#[repr(C)]
#[derive(Clone, Debug, Default, PartialEq, BorshDeserialize, BorshSerialize, BorshSchema)]
pub struct CommunityToken {
    /// DAO`s community token`s mint address
    pub token_mint: Pubkey
}
impl CommunityToken {
    /// Seed prefix for PDA
    const SEED_PREFIX: &'static [u8] = b"community_token";
}
impl SimplePda for CommunityToken {
    fn get_seed_prefix() -> &'static [u8] {
        return Self::SEED_PREFIX;
    }
}

/// Stores a state indicating the presence of a DAO in the pool. if DAO is enabled, then Community token mint should exist.
#[repr(C)]
#[derive(Clone, Debug, Default, PartialEq, BorshDeserialize, BorshSerialize, BorshSchema)]
pub struct DaoState {
    /// Is DAO enabled for StakePool
    pub is_enabled: bool
}
impl DaoState {
    /// Seed prefix for PDA
    const SEED_PREFIX: &'static [u8] = b"dao_state";
}
impl SimplePda for DaoState {
    fn get_seed_prefix() -> &'static [u8] {
        return Self::SEED_PREFIX;
    }
}

/// Account type. Needed to find from the network. Quantity of variants must be less than or equal 256. (1 byte)
/// Please do not change the order of this enum without essential needs and understanding consequenses
#[derive(Clone, Debug, PartialEq, BorshDeserialize, BorshSerialize, BorshSchema)]
pub enum NetworkAccountType {
    /// If the account has not been initialized, the enum will be 0
    Uninitialized,
    /// Account for CommunityTokenStakingRewards dto
    CommunityTokenStakingRewards,
    /// Account for MetricDepositReferrer dto
    MetricsDepositRefferer,
}

/// Stores the number of minted tokens for each category.
#[repr(C)]
#[derive(Clone, Debug, PartialEq, BorshDeserialize, BorshSerialize, BorshSchema)]
pub struct CommunityTokensCounter {
    evs_dao_reserve: u64,
    evs_strategic_reserve: u64,
}

impl CommunityTokensCounter {
    /// Seed prefix
    const SEED_PREFIX: &'static [u8] = b"community_tokens_counter";

    /// Decimals of community token
    pub const DECIMALS: u8 = spl_token::native_mint::DECIMALS;

    /// Multiplier for raw represenation
    const MULT_RAW: u64 = 10u64.pow(Self::DECIMALS as u32);

    /// Max supply for EVS DAO reserve
    pub const MAX_EVS_DAO_RESERVE_SUPPLY: u64 = 75_000_000 * Self::MULT_RAW;

    /// Max supply for EVS strategic reserve
    pub const MAX_EVS_STRATEGIC_RESERVE_SUPPLY: u64 = 25_000_000 * Self::MULT_RAW;

    /// Constrtructor
    pub fn new(evs_dao_reserve: u64, evs_strategic_reserve: u64) -> Self {
        Self {
            evs_dao_reserve,
            evs_strategic_reserve,
        }
    }

    /// Get max EVS supply
    pub fn get_max_supply() -> u64 {
        Self::MAX_EVS_DAO_RESERVE_SUPPLY + Self::MAX_EVS_STRATEGIC_RESERVE_SUPPLY
    }

    /// Getter for ui representation of evs_dao_reserve
    pub fn get_ui_evs_dao_reserve(&self) -> f64 {
        self.evs_dao_reserve as f64 / Self::MULT_RAW as f64
    }
    
    /// Getter for ui representation of evs_strategic_reserve
    pub fn get_ui_evs_strategic_reserve(&self) -> f64 {
        self.evs_strategic_reserve as f64 / Self::MULT_RAW as f64
    }

    /// Get the number of tokens allowed to be minted for EVS DAO reserve
    pub fn get_dao_reserve_allowed_tokens_number(&self, num: u64) -> Option<u64> {
        if self.evs_dao_reserve.checked_add(num)? > Self::MAX_EVS_DAO_RESERVE_SUPPLY {
            Self::MAX_EVS_DAO_RESERVE_SUPPLY.checked_sub(self.evs_dao_reserve)
        } else {
            Some(num)
        }
    }

    /// Get the number of tokens allowed to be minted for EVS strategic reserve
    pub fn get_strategic_reserve_allowed_tokens_number(&self, num: u64) -> Option<u64> {
        if self.evs_strategic_reserve.checked_add(num)? > Self::MAX_EVS_STRATEGIC_RESERVE_SUPPLY {
            Self::MAX_EVS_STRATEGIC_RESERVE_SUPPLY.checked_sub(self.evs_strategic_reserve)
        } else {
            Some(num)
        }
    }

    /// Fills evs dao reserve with the specified number of tokens and
    /// returns the actual number of tokens evs dao reserve has been filled with 
    pub fn fill_evs_dao_reserve(&mut self, num: u64) -> Option<u64> {
        let anum = self.get_dao_reserve_allowed_tokens_number(num)?;
        if anum > 0 {
            self.evs_dao_reserve = self.evs_dao_reserve.checked_add(anum)?;
            return Some(anum)
        }
        None
    }
}

impl SimplePda for CommunityTokensCounter {
    fn get_seed_prefix() -> &'static [u8] {
        return Self::SEED_PREFIX;
    }
}

/// Stores information that can be used to mint Community tokens according to a distribution strategy.
#[repr(C)]
#[derive(Clone, Debug, PartialEq, BorshDeserialize, BorshSerialize, BorshSchema)]
pub struct CommunityTokenStakingRewards {
    /// Account type. Needed to find from the network
    network_account_type: NetworkAccountType,
    /// value for determaning concrete group of account
    account_group: AccountGroup,
    /// Programm id. Needed to find from the network
    program_id: Pubkey,
    /// Stakr pool address. Needed to find from the network
    stake_pool_address: Pubkey,
    /// Owner wallet
    owner_wallet: Pubkey,
    /// The epoch in wich a person staked or changed the stake
    initial_staking_epoch: u64,
    /// The epoch in wich a person receive rewards in community tokens at last time
    last_rewarded_epoch: u64,
}

impl CommunityTokenStakingRewards {
    /// Seed prefix for PDA
    pub const SEED_PREFIX: &'static [u8] = b"c_t_staking_rewards";
    /// Find PDA 
    pub fn find_address(
        program_id: &Pubkey,
        stake_pool_address: &Pubkey,
        owner_wallet: &Pubkey,
    ) -> (Pubkey, u8) {
        Pubkey::find_program_address(
            &[
                Self::SEED_PREFIX,
                &stake_pool_address.to_bytes()[..],
                &owner_wallet.to_bytes()[..],
                &program_id.to_bytes()[..]
            ],
            program_id
        )
    }

    /// Constructor
    pub fn new(
        community_token_staking_rewards_counter: &mut CommunityTokenStakingRewardsCounter,
        network_account_type: NetworkAccountType,
        program_id: Pubkey,
        stake_pool_address: Pubkey,
        owner_wallet: Pubkey,
        initial_staking_epoch: u64,
        last_rewarded_epoch: u64,
    ) -> Self {
        return Self {
            network_account_type,
            account_group: community_token_staking_rewards_counter.calculate_account_group(),
            program_id,
            stake_pool_address,
            owner_wallet,
            initial_staking_epoch,
            last_rewarded_epoch
        }
    }

    /// Update initial_staking_epoch
    pub fn set_initial_staking_epoch(&mut self, epoch: u64) {
        self.initial_staking_epoch = epoch;
    }

    /// Update last_rewarded_epoch
    pub fn set_last_rewarded_epoch(&mut self, epoch: u64) {
        self.last_rewarded_epoch = epoch;
    }

    /// The owner_wallet getter
    pub fn get_owner_wallet(&self) -> &Pubkey {
        return &self.owner_wallet;
    }

    /// The initial_staking_epoch getter
    pub fn get_initial_staking_epoch(&self) -> u64 {
        return self.initial_staking_epoch
    }

    /// The last_rewarded_epoch getter
    pub fn get_last_rewarded_epoch(&self) -> u64 {
        return self.last_rewarded_epoch
    }
}

/// Stores information that allows to request accounts for storing CommunityTokenStakingRewards 
/// structure from the network in parts.
#[repr(C)]
#[derive(Clone, Debug, PartialEq, BorshDeserialize, BorshSerialize, BorshSchema)]
pub struct CommunityTokenStakingRewardsCounter {
    /// value for determaning concrete group of account
    account_group: AccountGroup,
    /// value for determaning account quantity in the one group
    counter_for_group: u16
}
impl CommunityTokenStakingRewardsCounter {
    /// Maximum quantity of account for requesting from the network at one time
    const MAX_QUANTITY_OF_ACCOUNTS_IN_GROUP: u8 = 100;
    /// Seed prefix for PDA
    const SEED_PREFIX: &'static [u8] = b"c_t_staking_rewards_counter";
    /// Initial value for account group
    pub const ACCOUNT_GROUP_INITIAL_VALUE: u64 = 1;

    /// Constructor
    pub fn new() -> Self {
        return Self {
            account_group: AccountGroup::new(Self::ACCOUNT_GROUP_INITIAL_VALUE),
            counter_for_group: 0
        }
    }

    /// Get calculated account group
    pub fn calculate_account_group(&mut self) -> AccountGroup {
        if (self.counter_for_group + 1) > (Self::MAX_QUANTITY_OF_ACCOUNTS_IN_GROUP as u16) {
            self.account_group = AccountGroup::new(self.account_group.value + 1);
            self.counter_for_group = 1;
        } else {
            self.counter_for_group = self.counter_for_group + 1;
        }

        return self.account_group.clone();
    }

    /// account_group getter
    pub fn get_account(&self) -> &AccountGroup {
        &self.account_group
    }

    /// counter_for_group getter
    pub fn get_counter(&self) -> u16 {
        self.counter_for_group
    }

    ///Get the total number of accounts
    pub fn get_number_of_accounts(&self) -> u64 {
        self.get_counter() as u64 + self
            .get_account()
            .get_value()
            .checked_sub(1)
            .map_or(0, |num| num * Self::MAX_QUANTITY_OF_ACCOUNTS_IN_GROUP as u64)
    }
}
impl SimplePda for CommunityTokenStakingRewardsCounter {
    fn get_seed_prefix() -> &'static [u8] {
        return Self::SEED_PREFIX;
    }
}

/// Initialized information for determaning account group
#[repr(C)]
#[derive(Clone, Debug, PartialEq, BorshDeserialize, BorshSerialize, BorshSchema)]
pub struct AccountGroup {
    /// value
    value: u64
}
impl AccountGroup {
    /// Constructor
    pub fn new(value: u64) -> Self {
        return Self {
            value
        }
    }

    /// Value getter
    pub fn get_value(&self) -> u64 {
        return self.value;
    }
}

/// Stores information that allows to request accounts for storing CommunityTokenStakingRewards 
/// structure from the network in parts.
#[repr(C)]
#[derive(Clone, Debug, PartialEq, BorshDeserialize, BorshSerialize, BorshSchema)]
pub struct MetricsDepositReferrerCounter {
    /// value for determaning concrete group of account
    account_group: AccountGroup,
    /// value for determaning account quantity in the one group
    counter_for_group: u16,
    /// how many groups flushed
    flushed_group: AccountGroup,
    /// how many accounts flushed in the last flushed group
    flushed_counter: u16,
}

impl MetricsDepositReferrerCounter {
    /// Maximum quantity of account for requesting from the network at one time
    pub const MAX_QUANTITY_OF_ACCOUNTS_IN_GROUP: u8 = 100;
    /// Seed prefix for PDA
    const SEED_PREFIX: &'static [u8] = b"metrics_deposit_referrer_counter";
    /// Initial value for account group
    pub const ACCOUNT_GROUP_INITIAL_VALUE: u64 = 1;

    /// Constructor
    pub fn new() -> Self {
        return Self {
            account_group: AccountGroup::new(Self::ACCOUNT_GROUP_INITIAL_VALUE),
            counter_for_group: 0,
            flushed_group: AccountGroup::new(Self::ACCOUNT_GROUP_INITIAL_VALUE),
            flushed_counter: 0,
        }
    }

    /// Get calculated account group
    pub fn calculate_account_group(&mut self) -> AccountGroup {
        if (self.counter_for_group + 1) > (Self::MAX_QUANTITY_OF_ACCOUNTS_IN_GROUP as u16) {
            self.account_group = AccountGroup::new(self.account_group.value + 1);
            self.counter_for_group = 1;
        } else {
            self.counter_for_group = self.counter_for_group + 1;
        }

        return self.account_group.clone();
    }

    /// Get calculated flushed account group
    pub fn increase_flushed_counter(&mut self) {
        if self.flushed_counter >= Self::MAX_QUANTITY_OF_ACCOUNTS_IN_GROUP as u16 {
            self.flushed_group = AccountGroup::new(self.flushed_group.value + 1);
            self.flushed_counter = 1;
        } else {
            self.flushed_counter = self.flushed_counter + 1;
        }
    }

    /// account_group getter
    pub fn get_account(&self) -> &AccountGroup {
        &self.account_group
    }

    /// account_group getter
    pub fn get_flushed_group(&self) -> &AccountGroup {
        &self.flushed_group
    }    

    /// counter_for_group getter
    pub fn get_counter(&self) -> u16 {
        self.counter_for_group
    }

    /// flushed_counter getter
    pub fn get_flushed_counter(&self) -> u16 {
        self.flushed_counter
    }

    /// Get the total number of accounts
    pub fn get_number_of_accounts(&self) -> u64 {
        self.get_counter() as u64 + self
            .get_account()
            .get_value()
            .checked_sub(1)
            .map_or(0, |num| num * Self::MAX_QUANTITY_OF_ACCOUNTS_IN_GROUP as u64)
    }

    /// Get id by index
    pub fn get_id_by_indexes(group_index: u64, account_index: u16) -> u64 {
        account_index as u64 + group_index
            .checked_sub(1)
            .map_or(0, |num| num * Self::MAX_QUANTITY_OF_ACCOUNTS_IN_GROUP as u64)
    }   

    /// Get the total number of flushed accounts
    pub fn get_number_of_flushed_accounts(&self) -> u64 {
        self.get_flushed_counter() as u64 + self
            .get_flushed_group()
            .get_value()
            .checked_sub(1)
            .map_or(0, |num| num * Self::MAX_QUANTITY_OF_ACCOUNTS_IN_GROUP as u64)
    }

    /// let's record how many accounts flushed
    pub fn flush(&mut self) {
        self.flushed_group = self.account_group.clone();
        self.flushed_counter = self.counter_for_group;
    }
}

impl SimplePda for MetricsDepositReferrerCounter {
    fn get_seed_prefix() -> &'static [u8] {
        return Self::SEED_PREFIX;
    }
}

/// Metrics for DepositSolWithReferrer instruction.
#[repr(C)]
#[derive(Clone, Debug, PartialEq, BorshDeserialize, BorshSerialize, BorshSchema)]
pub struct MetricsDepositReferrer {
    /// Account type. Needed to find from the network
    network_account_type: NetworkAccountType,
    /// Value for determaning concrete group of account
    account_group: AccountGroup,
    /// Programm id. Needed to find from the network
    program_id: Pubkey,
    /// Stake pool address. Needed to find from the network
    stake_pool_address: Pubkey,
    /// Epoch
    pub epoch: u64,
    /// Transaction timestamp
    pub timestamp: i64,
    /// From
    pub from: Pubkey,
    /// Referrer
    pub referrer: Pubkey,
    /// Number of lamports
    pub amount: u64,
}

impl MetricsDepositReferrer {
    /// Seed prefix for PDA
    pub const SEED_PREFIX: &'static [u8] = b"metric_deposit_referrer";
    /// Find PDA 
    pub fn find_address(
        program_id: &Pubkey,
        stake_pool_address: &Pubkey,
        id: u64,
    ) -> (Pubkey, u8) {
        Pubkey::find_program_address(
            &[
                Self::SEED_PREFIX,
                &stake_pool_address.to_bytes()[..],
                id.to_string().as_bytes(),
                &program_id.to_bytes()[..],
            ],
            program_id
        )
    }

    /// Constructor
    pub fn new(
        metrics_deposit_referrer_counter: &mut MetricsDepositReferrerCounter,
        network_account_type: NetworkAccountType,
        program_id: Pubkey,
        stake_pool_address: Pubkey,
        epoch: u64,
        timestamp: i64,
        from: Pubkey,
        referrer: Pubkey,
        amount: u64,
    ) -> Self {
        return Self {
            network_account_type,
            account_group: metrics_deposit_referrer_counter.calculate_account_group(),
            program_id,
            stake_pool_address,
            epoch,
            timestamp,
            from,
            referrer,
            amount,
        }
    }
}

impl SimplePda for MetricsDepositReferrer {
    fn get_seed_prefix() -> &'static [u8] {
        return Self::SEED_PREFIX;
    }
}

#[cfg(test)]
mod test {
    use {
        super::*,
        proptest::prelude::*,
        solana_program::borsh::{
            get_instance_packed_len, get_packed_len, try_from_slice_unchecked,
        },
        solana_program::native_token::LAMPORTS_PER_SOL,
    };

    fn uninitialized_validator_list() -> ValidatorList {
        ValidatorList {
            header: ValidatorListHeader {
                account_type: AccountType::Uninitialized,
                max_validators: 0,
            },
            validators: vec![],
        }
    }

    fn test_validator_list(max_validators: u32) -> ValidatorList {
        ValidatorList {
            header: ValidatorListHeader {
                account_type: AccountType::ValidatorList,
                max_validators,
            },
            validators: vec![
                ValidatorStakeInfo {
                    status: StakeStatus::Active,
                    vote_account_address: Pubkey::new_from_array([1; 32]),
                    active_stake_lamports: u64::from_le_bytes([255; 8]),
                    transient_stake_lamports: u64::from_le_bytes([128; 8]),
                    last_update_epoch: u64::from_le_bytes([64; 8]),
                    transient_seed_suffix_start: 0,
                    transient_seed_suffix_end: 0,
                },
                ValidatorStakeInfo {
                    status: StakeStatus::DeactivatingTransient,
                    vote_account_address: Pubkey::new_from_array([2; 32]),
                    active_stake_lamports: 998877665544,
                    transient_stake_lamports: 222222222,
                    last_update_epoch: 11223445566,
                    transient_seed_suffix_start: 0,
                    transient_seed_suffix_end: 0,
                },
                ValidatorStakeInfo {
                    status: StakeStatus::ReadyForRemoval,
                    vote_account_address: Pubkey::new_from_array([3; 32]),
                    active_stake_lamports: 0,
                    transient_stake_lamports: 0,
                    last_update_epoch: 999999999999999,
                    transient_seed_suffix_start: 0,
                    transient_seed_suffix_end: 0,
                },
            ],
        }
    }

    #[test]
    fn state_packing() {
        let max_validators = 10_000;
        let size = get_instance_packed_len(&ValidatorList::new(max_validators)).unwrap();
        let stake_list = uninitialized_validator_list();
        let mut byte_vec = vec![0u8; size];
        let mut bytes = byte_vec.as_mut_slice();
        stake_list.serialize(&mut bytes).unwrap();
        let stake_list_unpacked = try_from_slice_unchecked::<ValidatorList>(&byte_vec).unwrap();
        assert_eq!(stake_list_unpacked, stake_list);

        // Empty, one preferred key
        let stake_list = ValidatorList {
            header: ValidatorListHeader {
                account_type: AccountType::ValidatorList,
                max_validators: 0,
            },
            validators: vec![],
        };
        let mut byte_vec = vec![0u8; size];
        let mut bytes = byte_vec.as_mut_slice();
        stake_list.serialize(&mut bytes).unwrap();
        let stake_list_unpacked = try_from_slice_unchecked::<ValidatorList>(&byte_vec).unwrap();
        assert_eq!(stake_list_unpacked, stake_list);

        // With several accounts
        let stake_list = test_validator_list(max_validators);
        let mut byte_vec = vec![0u8; size];
        let mut bytes = byte_vec.as_mut_slice();
        stake_list.serialize(&mut bytes).unwrap();
        let stake_list_unpacked = try_from_slice_unchecked::<ValidatorList>(&byte_vec).unwrap();
        assert_eq!(stake_list_unpacked, stake_list);
    }

    #[test]
    fn validator_list_active_stake() {
        let max_validators = 10_000;
        let mut validator_list = test_validator_list(max_validators);
        assert!(validator_list.has_active_stake());
        for validator in validator_list.validators.iter_mut() {
            validator.active_stake_lamports = 0;
        }
        assert!(!validator_list.has_active_stake());
    }

    #[test]
    fn validator_list_deserialize_mut_slice() {
        let max_validators = 10;
        let stake_list = test_validator_list(max_validators);
        let mut serialized = stake_list.try_to_vec().unwrap();
        let (header, list) = ValidatorListHeader::deserialize_mut_slice(
            &mut serialized,
            0,
            stake_list.validators.len(),
        )
        .unwrap();
        assert_eq!(header.account_type, AccountType::ValidatorList);
        assert_eq!(header.max_validators, max_validators);
        assert!(list
            .iter()
            .zip(stake_list.validators.iter())
            .all(|(a, b)| *a == b));

        let (_, list) = ValidatorListHeader::deserialize_mut_slice(&mut serialized, 1, 2).unwrap();
        assert!(list
            .iter()
            .zip(stake_list.validators[1..].iter())
            .all(|(a, b)| *a == b));
        let (_, list) = ValidatorListHeader::deserialize_mut_slice(&mut serialized, 2, 1).unwrap();
        assert!(list
            .iter()
            .zip(stake_list.validators[2..].iter())
            .all(|(a, b)| *a == b));
        let (_, list) = ValidatorListHeader::deserialize_mut_slice(&mut serialized, 0, 2).unwrap();
        assert!(list
            .iter()
            .zip(stake_list.validators[..2].iter())
            .all(|(a, b)| *a == b));

        assert_eq!(
            ValidatorListHeader::deserialize_mut_slice(&mut serialized, 0, 4).unwrap_err(),
            ProgramError::AccountDataTooSmall
        );
        assert_eq!(
            ValidatorListHeader::deserialize_mut_slice(&mut serialized, 1, 3).unwrap_err(),
            ProgramError::AccountDataTooSmall
        );
    }

    #[test]
    fn validator_list_iter() {
        let max_validators = 10;
        let stake_list = test_validator_list(max_validators);
        let mut serialized = stake_list.try_to_vec().unwrap();
        let (_, big_vec) = ValidatorListHeader::deserialize_vec(&mut serialized).unwrap();
        for (a, b) in big_vec
            .iter::<ValidatorStakeInfo>()
            .zip(stake_list.validators.iter())
        {
            assert_eq!(a, b);
        }
    }

    proptest! {
        #[test]
        fn stake_list_size_calculation(test_amount in 0..=100_000_u32) {
            let validators = ValidatorList::new(test_amount);
            let size = get_instance_packed_len(&validators).unwrap();
            assert_eq!(ValidatorList::calculate_max_validators(size), test_amount as usize);
            assert_eq!(ValidatorList::calculate_max_validators(size.saturating_add(1)), test_amount as usize);
            assert_eq!(ValidatorList::calculate_max_validators(size.saturating_add(get_packed_len::<ValidatorStakeInfo>())), (test_amount + 1)as usize);
            assert_eq!(ValidatorList::calculate_max_validators(size.saturating_sub(1)), (test_amount.saturating_sub(1)) as usize);
        }
    }

    prop_compose! {
        fn fee()(denominator in 1..=u16::MAX)(
            denominator in Just(denominator),
            numerator in 0..=denominator,
        ) -> (u64, u64) {
            (numerator as u64, denominator as u64)
        }
    }

    #[test]
    fn specific_fee_calculation() {
        // 10% of 10 SOL in rewards should be 1 SOL in fees
        let epoch_fee = Fee {
            numerator: 1,
            denominator: 10,
        };
        let stake_pool = StakePool {
            total_lamports: 100 * LAMPORTS_PER_SOL,
            pool_token_supply: 100 * LAMPORTS_PER_SOL,
            epoch_fee,
            ..StakePool::default()
        };
        let reward_lamports = 10 * LAMPORTS_PER_SOL;
        let pool_token_fee = stake_pool
            .calc_pool_tokens_epoch_fee(reward_lamports)
            .unwrap();

        let fee_lamports = stake_pool
            .convert_amount_of_pool_tokens_to_amount_of_lamports(pool_token_fee)
            .unwrap();
        assert_eq!(fee_lamports, LAMPORTS_PER_SOL);
    }

    #[test]
    fn zero_withdraw_calculation() {
        let stake_pool = StakePool::default();
        assert_eq!(
            stake_pool
                .convert_amount_of_pool_tokens_to_amount_of_lamports(0)
                .unwrap(),
            0
        );
    }

    // #[test]
    // fn approximate_apr_calculation() {
    //     // 8% / year means roughly .044% / epoch
    //     let stake_pool = StakePool {
    //         last_epoch_total_lamports: 100_000,
    //         last_epoch_pool_token_supply: 100_000,
    //         total_lamports: 100_044,
    //         pool_token_supply: 100_000,
    //         ..StakePool::default()
    //     };
    //     let pool_token_value =
    //         stake_pool.total_lamports as f64 / stake_pool.pool_token_supply as f64;
    //     let last_epoch_pool_token_value = stake_pool.last_epoch_total_lamports as f64
    //         / stake_pool.last_epoch_pool_token_supply as f64;
    //     let epoch_rate = pool_token_value / last_epoch_pool_token_value - 1.0;
    //     const SECONDS_PER_EPOCH: f64 = DEFAULT_SLOTS_PER_EPOCH as f64 * DEFAULT_S_PER_SLOT;
    //     const EPOCHS_PER_YEAR: f64 = SECONDS_PER_DAY as f64 * 365.25 / SECONDS_PER_EPOCH;
    //     const EPSILON: f64 = 0.00001;
    //     let yearly_rate = epoch_rate * EPOCHS_PER_YEAR;
    //     assert!((yearly_rate - 0.080355).abs() < EPSILON);
    // }

    proptest! {
        #[test]
        fn fee_calculation(
            (numerator_fee, denominator_fee) in fee(),
            amount in 1..u64::MAX,
            (numerator, denominator) in rate_of_exchange()
        ) {
            let epsilon = 2;
            let epoch_fee = Fee { denominator: denominator_fee, numerator: numerator_fee };

            let stake_pool_with_simple_roe = StakePool {
                epoch_fee,
                ..StakePool::default()
            };
            let pool_token_fee = stake_pool_with_simple_roe.calc_pool_tokens_epoch_fee(amount).unwrap();
            let fee_lamports = stake_pool_with_simple_roe.convert_amount_of_pool_tokens_to_amount_of_lamports(pool_token_fee).unwrap();
            let max_fee_lamports = u64::try_from((amount as u128) * (epoch_fee.numerator as u128) / (epoch_fee.denominator as u128)).unwrap();
            assert!(max_fee_lamports >= fee_lamports);
            assert!(max_fee_lamports <= epsilon + fee_lamports);

            let stake_pool_with_hard_roe = StakePool {
                epoch_fee,
                rate_of_exchange: Some(
                    RateOfExchange {
                        denominator: denominator as u64,
                        numerator: denominator as u64 + numerator as u64
                    }
                ),
                ..StakePool::default()
            };
            let pool_token_fee = stake_pool_with_hard_roe.calc_pool_tokens_epoch_fee(amount).unwrap();
            let fee_lamports = stake_pool_with_hard_roe.convert_amount_of_pool_tokens_to_amount_of_lamports(pool_token_fee).unwrap();
            let max_fee_lamports = u64::try_from((amount as u128) * (epoch_fee.numerator as u128) / (epoch_fee.denominator as u128)).unwrap();
            assert!(max_fee_lamports >= fee_lamports);
            assert!(max_fee_lamports <= epsilon + fee_lamports);
        }
    }

    prop_compose! {
        fn rate_of_exchange()
        (denominator in 2..u32::MAX)
        (
            denominator in Just(denominator),
            numerator in 1..(denominator - 1)
        ) -> (u32, u32) {
            (numerator, denominator)
        }
    }

    proptest! {
        #[test]
        fn convertation(
            amount in 1..u64::MAX,
            (numerator, denominator) in rate_of_exchange()
        ) {
            let stake_pool_with_simple_roe = StakePool::default();
            assert!(
                amount == stake_pool_with_simple_roe.convert_amount_of_pool_tokens_to_amount_of_lamports(
                    stake_pool_with_simple_roe.convert_amount_of_lamports_to_amount_of_pool_tokens(amount).unwrap()
                ).unwrap()
            );

            let stake_pool_with_hard_roe = StakePool {
                rate_of_exchange: Some(
                    RateOfExchange {
                        denominator: denominator as u64,
                        numerator: denominator as u64 + numerator as u64
                    }
                ),
                ..StakePool::default()
            };
            let epsilon = 2;
            assert!(
                amount <= epsilon + stake_pool_with_hard_roe.convert_amount_of_pool_tokens_to_amount_of_lamports(
                    stake_pool_with_hard_roe.convert_amount_of_lamports_to_amount_of_pool_tokens(amount).unwrap()
                ).unwrap()
            );
        }
    }
}
