use {
    serde::{Deserialize, Serialize},
    solana_cli_output::{QuietDisplay, VerboseDisplay},
    solana_sdk::native_token::Sol,
    solana_sdk::{pubkey::Pubkey, stake::state::Lockup},
    spl_stake_pool::state::{Fee, StakePool, StakeStatus, ValidatorList, ValidatorStakeInfo, ReferrerList, Referrer, MetricsDepositReferrerCounter},
    std::fmt::{Display, Formatter, Result, Write},
    super::{ ValidatorsInfo, ValidatorsDataVec, ValidatorsData, 
        MetricsDepositReferrerInfoVec, MetricsDepositReferrerInfo, MetricsDepositReferrerCounterInfo,
        VALIDATOR_MAXIMUM_FEE, VALIDATOR_MAXIMUM_SKIPPED_SLOTS, VALIDATOR_MINIMUM_APY, VALIDATOR_MINIMUM_TOTAL_ACTIVE_STAKE,
        VALIDATORS_OFFSET, VALIDATORS_QUANTITY, VALIDATORS_QUERY_SIZE,
    },
    chrono::prelude::*,
};

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CliStakePools {
    pub pools: Vec<CliStakePool>,
}

impl Display for CliStakePools {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        for pool in &self.pools {
            writeln!(
                f,
                "Address: {}\tManager: {}\tLamports: {}\tPool tokens: {}\tValidators: {}",
                pool.address,
                pool.manager,
                pool.total_lamports,
                pool.pool_token_supply,
                pool.validator_list.len()
            )?;
        }
        writeln!(f, "Total number of pools: {}", &self.pools.len())?;
        Ok(())
    }
}

impl QuietDisplay for CliStakePools {}
impl VerboseDisplay for CliStakePools {}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CliStakePool {
    pub address: String,
    pub pool_withdraw_authority: String,
    pub manager: String,
    pub staker: String,
    pub stake_deposit_authority: String,
    pub stake_withdraw_bump_seed: u8,
    pub max_validators: u32,
    pub validator_list: Vec<CliStakePoolValidator>,
    pub validator_list_storage_account: String,
    pub reserve_stake: String,
    pub pool_mint: String,
    pub manager_fee_account: String,
    pub token_program_id: String,
    pub total_lamports: u64,
    pub pool_token_supply: u64,
    pub last_update_epoch: u64,
    pub lockup: CliStakePoolLockup,
    pub epoch_fee: CliStakePoolFee,
    pub next_epoch_fee: Option<CliStakePoolFee>,
    pub preferred_deposit_validator_vote_address: Option<String>,
    pub preferred_withdraw_validator_vote_address: Option<String>,
    pub stake_deposit_fee: CliStakePoolFee,
    pub stake_withdrawal_fee: CliStakePoolFee,
    pub next_stake_withdrawal_fee: Option<CliStakePoolFee>,
    pub stake_referral_fee: u8,
    pub sol_deposit_authority: Option<String>,
    pub sol_deposit_fee: CliStakePoolFee,
    pub sol_referral_fee: u8,
    pub sol_withdraw_authority: Option<String>,
    pub sol_withdrawal_fee: CliStakePoolFee,
    pub next_sol_withdrawal_fee: Option<CliStakePoolFee>,
    pub last_epoch_pool_token_supply: u64,
    pub last_epoch_total_lamports: u64,
    pub treasury_fee_account: String,
    pub treasury_fee: CliStakePoolFee,
    pub referrer_list: Vec<CliStakePoolReferrer>,
    pub referrer_list_storage_account: String,
    pub max_referrers: u32,
    pub max_validator_yield_per_epoch_numerator: u32,
    pub no_fee_deposit_threshold: u16,
    pub details: Option<CliStakePoolDetails>,
}

impl QuietDisplay for CliStakePool {}
impl VerboseDisplay for CliStakePool {
    fn write_str(&self, w: &mut dyn Write) -> Result {
        writeln!(w, "Stake Pool Info")?;
        writeln!(w, "===============")?;
        writeln!(w, "Stake Pool: {}", &self.address)?;
        writeln!(
            w,
            "Validator List: {}",
            &self.validator_list_storage_account
        )?;
        writeln!(
            w,
            "Referrer List: {}",
            &self.referrer_list_storage_account
        )?;
        writeln!(w, "Manager: {}", &self.manager)?;
        writeln!(w, "Staker: {}", &self.staker)?;
        writeln!(w, "Depositor: {}", &self.stake_deposit_authority)?;
        writeln!(
            w,
            "SOL Deposit Authority: {}",
            &self
                .sol_deposit_authority
                .as_ref()
                .unwrap_or(&"None".to_string())
        )?;
        writeln!(
            w,
            "SOL Withdraw Authority: {}",
            &self
                .sol_withdraw_authority
                .as_ref()
                .unwrap_or(&"None".to_string())
        )?;
        writeln!(w, "Withdraw Authority: {}", &self.pool_withdraw_authority)?;
        writeln!(w, "Pool Token Mint: {}", &self.pool_mint)?;
        writeln!(w, "Manager (Epoch) Fee Account: {}", &self.manager_fee_account)?;
        writeln!(w, "Treasury Fee Account: {}", &self.treasury_fee_account)?; 
        match &self.preferred_deposit_validator_vote_address {
            None => {}
            Some(s) => {
                writeln!(w, "Preferred Deposit Validator: {}", s)?;
            }
        }
        match &self.preferred_withdraw_validator_vote_address {
            None => {}
            Some(s) => {
                writeln!(w, "Preferred Withraw Validator: {}", s)?;
            }
        }
        writeln!(w, "Manager (Epoch) Fee: {} of epoch rewards", &self.epoch_fee)?;
        if let Some(next_epoch_fee) = &self.next_epoch_fee {
            writeln!(w, "Next Epoch Manager Fee: {} of epoch rewards", next_epoch_fee)?;
        }
        writeln!(
            w,
            "Stake Withdrawal Fee: {} of withdrawal amount",
            &self.stake_withdrawal_fee
        )?;
        if let Some(next_stake_withdrawal_fee) = &self.next_stake_withdrawal_fee {
            writeln!(
                w,
                "Next Stake Withdrawal Fee: {} of withdrawal amount",
                next_stake_withdrawal_fee
            )?;
        }
        writeln!(
            w,
            "SOL Withdrawal Fee: {} of withdrawal amount",
            &self.sol_withdrawal_fee
        )?;
        if let Some(next_sol_withdrawal_fee) = &self.next_sol_withdrawal_fee {
            writeln!(
                w,
                "Next SOL Withdrawal Fee: {} of withdrawal amount",
                next_sol_withdrawal_fee
            )?;
        }
        writeln!(
            w,
            "Stake Deposit Fee: {} of deposit amount",
            &self.stake_deposit_fee
        )?;
        writeln!(
            w,
            "SOL Deposit Fee: {} of deposit amount",
            &self.sol_deposit_fee
        )?;
        writeln!(
            w,
            "Stake Deposit Referral Fee: {}% of Stake Deposit Fee",
            &self.stake_referral_fee
        )?;
        writeln!(
            w,
            "SOL Deposit Referral Fee: {}% of SOL Deposit Fee",
            &self.sol_referral_fee
        )?;           
        writeln!(w, "Treasury Fee: {} of epoch rewards", &self.treasury_fee)?;
        writeln!(w, "Max validator yield per epoch (numerator): {}", self.max_validator_yield_per_epoch_numerator)?;
        writeln!(w, "No fee deposit threshold: {}", self.no_fee_deposit_threshold)?;

        match &self.details {
            None => {}
            Some(details) => {
                VerboseDisplay::write_str(details, w)?;
            }
        }
        writeln!(w)?;
        writeln!(w, "Max number of referrers: {}", self.max_referrers)?;
        writeln!(w, "Current number of referrers: {}", self.referrer_list.len())?;
        if self.referrer_list.len() > 0 {
            writeln!(w)?;
            writeln!(w, "Referrers")?;
            writeln!(w, "--------------")?;
            for referrer in &self.referrer_list {
                writeln!(w, "{}", referrer.referrer_address)?;
            }
        }
        Ok(())
    }
}

impl Display for CliStakePool {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        writeln!(f, "Stake Pool: {}", &self.address)?;
        writeln!(
            f,
            "Validator List: {}",
            &self.validator_list_storage_account
        )?;
        writeln!(
            f,
            "Referrer List: {}",
            &self.referrer_list_storage_account
        )?;
        writeln!(f, "Pool Token Mint: {}", &self.pool_mint)?;
        match &self.preferred_deposit_validator_vote_address {
            None => {}
            Some(s) => {
                writeln!(f, "Preferred Deposit Validator: {}", s)?;
            }
        }
        match &self.preferred_withdraw_validator_vote_address {
            None => {}
            Some(s) => {
                writeln!(f, "Preferred Withraw Validator: {}", s)?;
            }
        }
        writeln!(f, "Manager (Epoch) Fee: {} of epoch rewards", &self.epoch_fee)?;
        writeln!(
            f,
            "Stake Withdrawal Fee: {} of withdrawal amount",
            &self.stake_withdrawal_fee
        )?;
        writeln!(
            f,
            "SOL Withdrawal Fee: {} of withdrawal amount",
            &self.sol_withdrawal_fee
        )?;
        writeln!(
            f,
            "Stake Deposit Fee: {} of deposit amount",
            &self.stake_deposit_fee
        )?;
        writeln!(
            f,
            "SOL Deposit Fee: {} of deposit amount",
            &self.sol_deposit_fee
        )?;
        writeln!(
            f,
            "Stake Deposit Referral Fee: {}% of Stake Deposit Fee",
            &self.stake_referral_fee
        )?;
        writeln!(
            f,
            "SOL Deposit Referral Fee: {}% of SOL Deposit Fee",
            &self.sol_referral_fee
        )?;
        writeln!(f, "Treasury Fee: {} of epoch rewards", &self.treasury_fee)?;

        Ok(())
    }
}

#[derive(Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CliDaoDetails {
    pub community_token: String,
    pub ct_staking_reward_group: u64,
    pub ct_staking_reward_counter: u16,
    pub ct_staking_reward_accounts_num: u64,
    pub ct_evs_dao_reserve: f64,
    pub ct_evs_strategic_reserve: f64,
}

impl From<(String, (u64, u16, u64), (f64, f64))> for CliDaoDetails {
    fn from (args: (String, (u64, u16, u64), (f64, f64))) -> Self {
        let (community_token, (ct_staking_reward_group, ct_staking_reward_counter, ct_staking_reward_accounts_num), (ct_evs_dao_reserve, ct_evs_strategic_reserve)) = args;
        Self { community_token, ct_staking_reward_group, ct_staking_reward_counter, ct_staking_reward_accounts_num,  ct_evs_dao_reserve, ct_evs_strategic_reserve}
    }
}

impl Display for CliDaoDetails {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        VerboseDisplay::write_str(self, f)?;
        Ok(())
    }    
}
impl QuietDisplay for CliDaoDetails {}
impl VerboseDisplay for CliDaoDetails {
    fn write_str(&self, w: &mut dyn Write) -> Result {
        writeln!(w, "Community Token Mint: {}", &self.community_token,)?;
        writeln!(w, "Community Token Staking Rewards Group: {}", &self.ct_staking_reward_group,)?;
        writeln!(w, "Community Token Staking Rewards Group Counter: {}", &self.ct_staking_reward_counter,)?;
        writeln!(w, "Community Token Staking Rewards Accounts Number: {}", &self.ct_staking_reward_accounts_num,)?;
        writeln!(w, "EVS DAO Reserve: {}", &self.ct_evs_dao_reserve,)?;
        writeln!(w, "EVS Strategic Reserve: {}", &self.ct_evs_strategic_reserve,)?;
        Ok(())
    }    
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CliStakePoolDetails {
    pub reserve_stake_account_address: String,
    pub reserve_stake_lamports: u64,
    pub minimum_reserve_stake_balance: u64,
    pub stake_accounts: Vec<CliStakePoolStakeAccountInfo>,
    pub total_lamports: u64,
    pub total_liquidity_lamports: u64,
    pub total_pool_tokens: f64,
    pub current_number_of_validators: u32,
    pub max_number_of_validators: u32,
    pub update_required: bool,
    pub metrics_deposit_referrer_counter: Option<MetricsDepositReferrerCounterInfo>,
    pub dao_details: Option<CliDaoDetails>,
}

impl Display for CliStakePoolDetails {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        writeln!(
            f,
            "Reserve Account: {}\tAvailable Balance: {}",
            &self.reserve_stake_account_address,
            Sol(self.reserve_stake_lamports - self.minimum_reserve_stake_balance),
        )?;
        for stake_account in &self.stake_accounts {
            writeln!(
                f,
                "Vote Account: {}\tBalance: {}\tLast Update Epoch: {}",
                stake_account.vote_account_address,
                Sol(stake_account.validator_lamports),
                stake_account.validator_last_update_epoch,
            )?;
        }
        writeln!(
            f,
            "Total Pool Stake: {} {}",
            Sol(self.total_lamports),
            if self.update_required {
                " [UPDATE REQUIRED]"
            } else {
                ""
            },
        )?;
        writeln!(f, "Total Pool Tokens: {}", &self.total_pool_tokens,)?;
        writeln!(
            f,
            "Current Number of Validators: {}",
            &self.current_number_of_validators,
        )?;
        writeln!(
            f,
            "Max Number of Validators: {}",
            &self.max_number_of_validators,
        )?;
        Ok(())
    }
}

impl QuietDisplay for CliStakePoolDetails {}
impl VerboseDisplay for CliStakePoolDetails {
    fn write_str(&self, w: &mut dyn Write) -> Result {
        writeln!(w)?;
        writeln!(w, "Stake Accounts")?;
        writeln!(w, "--------------")?;
        writeln!(
            w,
            "Reserve Account: {}\tAvailable Balance: {}",
            &self.reserve_stake_account_address,
            Sol(self.reserve_stake_lamports - self.minimum_reserve_stake_balance),
        )?;
        writeln!(
            w,
            "Liquidity Balance: {}",
            Sol(self.total_liquidity_lamports)
        )?;
        for stake_account in &self.stake_accounts {
            writeln!(
                w,
                "Vote Account: {}\tStake Account: {}\tActive Balance: {}\tTransient Stake Account: {}\tTransient Balance: {}\tLast Update Epoch: {}{}",
                stake_account.vote_account_address,
                stake_account.stake_account_address,
                Sol(stake_account.validator_active_stake_lamports),
                stake_account.validator_transient_stake_account_address,
                Sol(stake_account.validator_transient_stake_lamports),
                stake_account.validator_last_update_epoch,
                if stake_account.update_required {
                    " [UPDATE REQUIRED]"
                } else {
                    ""
                },
            )?;
        }
        writeln!(
            w,
            "Total Pool Stake: {} {}",
            Sol(self.total_lamports),
            if self.update_required {
                " [UPDATE REQUIRED]"
            } else {
                ""
            },
        )?;
        writeln!(w, "Total Pool Tokens: {}", &self.total_pool_tokens,)?;
        writeln!(
            w,
            "Current Number of Validators: {}",
            &self.current_number_of_validators,
        )?;
        writeln!(
            w,
            "Max Number of Validators: {}",
            &self.max_number_of_validators,
        )?;

        if let Some(m) = &self.metrics_deposit_referrer_counter {
            writeln!(w, "{}", m)?;
        } else {
            writeln!(w, "No metrics counter for Referrer deposits found")?;
        }
        
        writeln!(w)?;
        writeln!(w, "DAO Info")?;
        writeln!(w, "--------------")?;

        match &self.dao_details {
            None => {  writeln!(w, "DAO State: Disabled")? }
            Some(details) => {
                writeln!(w, "DAO state: Enabled")?;
                VerboseDisplay::write_str(details, w)?;
            }
        }
        Ok(())
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CliStakePoolStakeAccountInfo {
    pub vote_account_address: String,
    pub stake_account_address: String,
    pub validator_active_stake_lamports: u64,
    pub validator_last_update_epoch: u64,
    pub validator_lamports: u64,
    pub validator_transient_stake_account_address: String,
    pub validator_transient_stake_lamports: u64,
    pub update_required: bool,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CliStakePoolValidator {
    pub active_stake_lamports: u64,
    pub transient_stake_lamports: u64,
    pub last_update_epoch: u64,
    pub transient_seed_suffix_start: u64,
    pub transient_seed_suffix_end: u64,
    pub status: CliStakePoolValidatorStakeStatus,
    pub vote_account_address: String,
}

impl From<ValidatorStakeInfo> for CliStakePoolValidator {
    fn from(v: ValidatorStakeInfo) -> Self {
        Self {
            active_stake_lamports: v.active_stake_lamports,
            transient_stake_lamports: v.transient_stake_lamports,
            last_update_epoch: v.last_update_epoch,
            transient_seed_suffix_start: v.transient_seed_suffix_start,
            transient_seed_suffix_end: v.transient_seed_suffix_end,
            status: CliStakePoolValidatorStakeStatus::from(v.status),
            vote_account_address: v.vote_account_address.to_string(),
        }
    }
}

impl From<StakeStatus> for CliStakePoolValidatorStakeStatus {
    fn from(s: StakeStatus) -> CliStakePoolValidatorStakeStatus {
        match s {
            StakeStatus::Active => CliStakePoolValidatorStakeStatus::Active,
            StakeStatus::DeactivatingTransient => {
                CliStakePoolValidatorStakeStatus::DeactivatingTransient
            }
            StakeStatus::ReadyForRemoval => CliStakePoolValidatorStakeStatus::ReadyForRemoval,
        }
    }
}

#[derive(Serialize, Deserialize)]
pub(crate) enum CliStakePoolValidatorStakeStatus {
    Active,
    DeactivatingTransient,
    ReadyForRemoval,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CliStakePoolLockup {
    pub unix_timestamp: i64,
    pub epoch: u64,
    pub custodian: String,
}

impl From<Lockup> for CliStakePoolLockup {
    fn from(l: Lockup) -> Self {
        Self {
            unix_timestamp: l.unix_timestamp,
            epoch: l.epoch,
            custodian: l.custodian.to_string(),
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CliStakePoolFee {
    pub denominator: u64,
    pub numerator: u64,
}

impl Display for CliStakePoolFee {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        write!(f, "{}/{}", &self.numerator, &self.denominator)
    }
}

impl From<Fee> for CliStakePoolFee {
    fn from(f: Fee) -> Self {
        Self {
            denominator: f.denominator,
            numerator: f.numerator,
        }
    }
}

impl From<(Pubkey, StakePool, ValidatorList, Pubkey, ReferrerList, Pubkey)> for CliStakePool {
    fn from(s: (Pubkey, StakePool, ValidatorList, Pubkey, ReferrerList, Pubkey)) -> Self {
        let (
            address,
            stake_pool,
            validator_list,
            pool_withdraw_authority,
            referrer_list,
            referrer_list_storage_account,
        ) = s;
        
        Self {
            address: address.to_string(),
            pool_withdraw_authority: pool_withdraw_authority.to_string(),
            manager: stake_pool.manager.to_string(),
            staker: stake_pool.staker.to_string(),
            stake_deposit_authority: stake_pool.stake_deposit_authority.to_string(),
            stake_withdraw_bump_seed: stake_pool.stake_withdraw_bump_seed,
            max_validators: validator_list.header.max_validators,
            validator_list: validator_list
                .validators
                .into_iter()
                .map(CliStakePoolValidator::from)
                .collect(),
            validator_list_storage_account: stake_pool.validator_list.to_string(),
            reserve_stake: stake_pool.reserve_stake.to_string(),
            pool_mint: stake_pool.pool_mint.to_string(),
            manager_fee_account: stake_pool.manager_fee_account.to_string(),
            token_program_id: stake_pool.token_program_id.to_string(),
            total_lamports: stake_pool.total_lamports,
            pool_token_supply: stake_pool.pool_token_supply,
            last_update_epoch: stake_pool.last_update_epoch,
            lockup: CliStakePoolLockup::from(stake_pool.lockup),
            epoch_fee: CliStakePoolFee::from(stake_pool.epoch_fee),
            next_epoch_fee: stake_pool.next_epoch_fee.map(CliStakePoolFee::from),
            preferred_deposit_validator_vote_address: stake_pool
                .preferred_deposit_validator_vote_address
                .map(|x| x.to_string()),
            preferred_withdraw_validator_vote_address: stake_pool
                .preferred_withdraw_validator_vote_address
                .map(|x| x.to_string()),
            stake_deposit_fee: CliStakePoolFee::from(stake_pool.stake_deposit_fee),
            stake_withdrawal_fee: CliStakePoolFee::from(stake_pool.stake_withdrawal_fee),
            next_stake_withdrawal_fee: stake_pool
                .next_stake_withdrawal_fee
                .map(CliStakePoolFee::from),
            stake_referral_fee: stake_pool.stake_referral_fee,
            sol_deposit_authority: stake_pool.sol_deposit_authority.map(|x| x.to_string()),
            sol_deposit_fee: CliStakePoolFee::from(stake_pool.sol_deposit_fee),
            sol_referral_fee: stake_pool.sol_referral_fee,
            sol_withdraw_authority: stake_pool.sol_withdraw_authority.map(|x| x.to_string()),
            sol_withdrawal_fee: CliStakePoolFee::from(stake_pool.sol_withdrawal_fee),
            next_sol_withdrawal_fee: stake_pool
                .next_sol_withdrawal_fee
                .map(CliStakePoolFee::from),
            last_epoch_pool_token_supply: stake_pool.last_epoch_pool_token_supply,
            last_epoch_total_lamports: stake_pool.last_epoch_total_lamports,
            treasury_fee_account: stake_pool.treasury_fee_account.to_string(),
            treasury_fee: CliStakePoolFee::from(stake_pool.treasury_fee),
            referrer_list: referrer_list
                .referrers
                .into_iter()
                .map(CliStakePoolReferrer::from)
                .collect(),
            referrer_list_storage_account: referrer_list_storage_account.to_string(),
            max_referrers: referrer_list.header.max_referrers,
            max_validator_yield_per_epoch_numerator: stake_pool.max_validator_yield_per_epoch_numerator,
            no_fee_deposit_threshold: stake_pool.no_fee_deposit_threshold,
            details: None,
        }
    }
}

impl VerboseDisplay for ValidatorsData {
    fn write_str(&self, w: &mut dyn Write) -> Result {
        write!(w, "name: {}, APY: {}, score: {}, vote: {}, node: {}, stake: {}", 
            self.name, self.apy, self.score, self.vote_pk, self.node_pk, self.total_active_stake)?;
        if self.drop_reasons.is_some() {
            write!(w, " drop reasons: {:?}", self.drop_reasons.as_ref().unwrap())?;
        }
        Ok(())
    }    
}

impl Display for ValidatorsData {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        write!(f, "name: {}, APY: {}, vote: {}", 
            self.name, self.apy, self.vote_pk)?;
        if self.drop_reasons.is_some() {
            write!(f, " drop reasons: {:?}", self.drop_reasons.as_ref().unwrap())?;
        }
        Ok(())
    }
}

impl VerboseDisplay for ValidatorsDataVec {
    fn write_str(&self, w: &mut dyn Write) -> Result {
        writeln!(w, "==========================================================")?; 
        if self.desc.is_some() {   
            writeln!(w, "{}", self.desc.as_ref().unwrap())?;
            writeln!(w, "==========================================================")?;
        }

        for (i, validator_data) in self.vec.iter().enumerate() {
            write!(w, "{}. ", i+1)?;
            VerboseDisplay::write_str(validator_data, w)?;
            writeln!(w)?;
        }
        writeln!(w)?;
        Ok(())        
    }
}

impl Display for ValidatorsDataVec {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        writeln!(f, "==========================================================")?; 
        if self.desc.is_some() {
            writeln!(f, "{}", self.desc.as_ref().unwrap())?;
            writeln!(f, "==========================================================")?;
        }
        for (i, validator_data) in self.vec.iter().enumerate() {
            writeln!(f, "{}. {}", i+1, validator_data)?
        }
        Ok(())
    }
}

impl VerboseDisplay for ValidatorsInfo {
    fn write_str(&self, w: &mut dyn Write) -> Result {
        VerboseDisplay::write_str(&self.current_validators, w)?;
        VerboseDisplay::write_str(&self.potential_validators, w)?;
        VerboseDisplay::write_str(&self.validators_to_be_added, w)?;
        VerboseDisplay::write_str(&self.validators_to_be_removed, w)?;

        //Print Constants
        writeln!(w, "==========================================================")?;
        writeln!(w, "Validators config")?;
        writeln!(w, "==========================================================")?;
        writeln!(w, "VALIDATOR_MAXIMUM_FEE {}", VALIDATOR_MAXIMUM_FEE)?;
        writeln!(w, "VALIDATOR_MAXIMUM_SKIPPED_SLOTS {}", VALIDATOR_MAXIMUM_SKIPPED_SLOTS)?;
        writeln!(w, "VALIDATOR_MINIMUM_APY {}", VALIDATOR_MINIMUM_APY)?;
        writeln!(w, "VALIDATOR_MINIMUM_TOTAL_ACTIVE_STAKE {}", VALIDATOR_MINIMUM_TOTAL_ACTIVE_STAKE)?;
        writeln!(w, "VALIDATORS_QUANTITY {}", VALIDATORS_QUANTITY)?;
        writeln!(w, "VALIDATORS_QUERY_SIZE {}", VALIDATORS_QUERY_SIZE)?;
        writeln!(w, "VALIDATORS_OFFSET {}", VALIDATORS_OFFSET)?;
        Ok(())
    }
}

impl QuietDisplay for ValidatorsInfo {}
impl Display for ValidatorsInfo {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        writeln!(f, "{}", self.current_validators)?;
        writeln!(f, "{}", self.potential_validators)?;
        writeln!(f, "{}", self.validators_to_be_added)?;
        writeln!(f, "{}", self.validators_to_be_removed)?;
        Ok(())
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CliStakePoolReferrer {
    pub referrer_address: String,
}

impl From<Referrer> for CliStakePoolReferrer {
    fn from(r: Referrer) -> Self {
        Self {
            referrer_address: r.key.to_string(),
        }
    }
}

impl VerboseDisplay for MetricsDepositReferrerInfo {
    fn write_str(&self, w: &mut dyn Write) -> Result {
        let nt = NaiveDateTime::from_timestamp(self.timestamp, 0);
        let dt: DateTime<Utc> = DateTime::from_utc(nt, Utc);
        let datetime = dt.format("%Y-%m-%d %H:%M:%S");

        writeln!(w, "{}. Epoch: {} Timestamp: {} Referrer: {} From: {} Amount: {}", self.id, self.epoch, datetime, self.referrer, self.from, self.amount)?;
        Ok(())       
    }  
}

impl Display for MetricsDepositReferrerInfo {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        let nt = NaiveDateTime::from_timestamp(self.timestamp, 0);
        let dt: DateTime<Utc> = DateTime::from_utc(nt, Utc);
        let datetime = dt.format("%Y-%m-%d %H:%M:%S");

        writeln!(f, "{},{},{},{},{},{}", self.id, self.epoch, datetime, self.referrer, self.from, self.amount)?;
        Ok(())
    }
}

impl VerboseDisplay for MetricsDepositReferrerInfoVec {
    fn write_str(&self, w: &mut dyn Write) -> Result {
        for metrics in &self.metrics_buffer {
            VerboseDisplay::write_str(metrics, w)?;
        }
        Ok(())        
    }
}
impl QuietDisplay for MetricsDepositReferrerInfoVec {}
impl Display for MetricsDepositReferrerInfoVec {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        writeln!(f, "Id,Epoch,Timestamp,Referrer,From,Amount")?;
        for metrics in &self.metrics_buffer {
            write!(f, "{}", metrics)?;
        }
        Ok(())
    }
}

impl From<MetricsDepositReferrerCounter> for MetricsDepositReferrerCounterInfo {
    fn from(mc: MetricsDepositReferrerCounter) -> Self {
        Self {
            max_accounts_in_group: MetricsDepositReferrerCounter::MAX_QUANTITY_OF_ACCOUNTS_IN_GROUP,
            group_initial_index: MetricsDepositReferrerCounter::ACCOUNT_GROUP_INITIAL_VALUE,
            group_index: mc.get_account().get_value(),
            account_index: mc.get_counter(),
            total_accounts: mc.get_number_of_accounts(),
            flushed_group_index: mc.get_flushed_group().get_value(),
            flushed_account_index: mc.get_flushed_counter(),
            total_flushed_accounts: mc.get_number_of_flushed_accounts(),
        }
    }
}

impl Display for MetricsDepositReferrerCounterInfo {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        writeln!(f)?;
        writeln!(f, "Referrer deposits counter")?;
        writeln!(f, "-------------------------")?;
        writeln!(f, "group initial index: {}", self.group_initial_index)?;
        writeln!(f, "max_accounts_in_group: {}", self.max_accounts_in_group)?;
        writeln!(f, "total accounts: {}", self.total_accounts)?;
        writeln!(f, "total flushed accounts: {}", self.total_flushed_accounts)?;
        writeln!(f, "group index: {}", self.group_index)?;
        writeln!(f, "account index: {}", self.account_index)?;
        writeln!(f, "flushed group index: {}", self.flushed_group_index)?;
        writeln!(f, "flushed account index: {}", self.flushed_account_index)?;
        Ok(())
    }
}