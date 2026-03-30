#![no_std]
#![deny(warnings)]
#![deny(clippy::all)]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, token, Address, BytesN, Env, Symbol,
};

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    VaultNotFound = 1,
    NotAuthorized = 2,
    VaultNotActive = 3,
    InvalidTimestamp = 4,
    MilestoneExpired = 5,
    InvalidStatus = 6,
    InvalidAmount = 7,
    InvalidTimestamps = 8,
    DurationTooLong = 9,
}

// ---------------------------------------------------------------------------
// Data types
// ---------------------------------------------------------------------------

#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VaultStatus {
    Active = 0,
    Completed = 1,
    Failed = 2,
    Cancelled = 3,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProductivityVault {
    pub creator: Address,
    pub amount: i128,
    pub start_timestamp: u64,
    pub end_timestamp: u64,
    pub milestone_hash: BytesN<32>,
    pub verifier: Option<Address>,
    pub success_destination: Address,
    pub failure_destination: Address,
    pub status: VaultStatus,
    pub milestone_validated: bool,
}

/// ✅ NEW: avoids clippy::too_many_arguments
#[contracttype]
#[derive(Clone)]
pub struct CreateVaultParams {
    pub usdc_token: Address,
    pub creator: Address,
    pub amount: i128,
    pub start_timestamp: u64,
    pub end_timestamp: u64,
    pub milestone_hash: BytesN<32>,
    pub verifier: Option<Address>,
    pub success_destination: Address,
    pub failure_destination: Address,
}

// ---------------------------------------------------------------------------
// Storage keys
// ---------------------------------------------------------------------------

pub const MAX_VAULT_DURATION: u64 = 365 * 24 * 60 * 60;
pub const MIN_AMOUNT: i128 = 10_000_000;
pub const MAX_AMOUNT: i128 = 10_000_000_000_000;

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Vault(u32),
    VaultCount,
}

// ---------------------------------------------------------------------------
// Contract
// ---------------------------------------------------------------------------

#[contract]
pub struct DisciplrVault;

#[contractimpl]
impl DisciplrVault {
    pub fn create_vault(env: Env, params: CreateVaultParams) -> Result<u32, Error> {
        params.creator.require_auth();

        if params.amount < MIN_AMOUNT || params.amount > MAX_AMOUNT {
            return Err(Error::InvalidAmount);
        }

        let now = env.ledger().timestamp();

        if params.start_timestamp < now {
            return Err(Error::InvalidTimestamp);
        }

        if params.end_timestamp <= params.start_timestamp {
            return Err(Error::InvalidTimestamps);
        }

        let duration = params.end_timestamp - params.start_timestamp;
        if duration > MAX_VAULT_DURATION {
            return Err(Error::DurationTooLong);
        }

        let token_client = token::Client::new(&env, &params.usdc_token);
        token_client.transfer(
            &params.creator,
            &env.current_contract_address(),
            &params.amount,
        );

        let mut vault_count: u32 = env
            .storage()
            .instance()
            .get(&DataKey::VaultCount)
            .unwrap_or(0);

        let vault_id = vault_count;
        vault_count += 1;

        env.storage()
            .instance()
            .set(&DataKey::VaultCount, &vault_count);

        let vault = ProductivityVault {
            creator: params.creator,
            amount: params.amount,
            start_timestamp: params.start_timestamp,
            end_timestamp: params.end_timestamp,
            milestone_hash: params.milestone_hash,
            verifier: params.verifier,
            success_destination: params.success_destination,
            failure_destination: params.failure_destination,
            status: VaultStatus::Active,
            milestone_validated: false,
        };

        env.storage()
            .instance()
            .set(&DataKey::Vault(vault_id), &vault);

        env.events().publish(
            (Symbol::new(&env, "vault_created"), vault_id),
            vault.clone(),
        );

        Ok(vault_id)
    }

    pub fn validate_milestone(env: Env, vault_id: u32) -> Result<bool, Error> {
        let key = DataKey::Vault(vault_id);
        let mut vault: ProductivityVault = env
            .storage()
            .instance()
            .get(&key)
            .ok_or(Error::VaultNotFound)?;

        if vault.status != VaultStatus::Active {
            return Err(Error::VaultNotActive);
        }

        if let Some(ref verifier) = vault.verifier {
            verifier.require_auth();
        } else {
            vault.creator.require_auth();
        }

        if env.ledger().timestamp() >= vault.end_timestamp {
            return Err(Error::MilestoneExpired);
        }

        vault.milestone_validated = true;
        env.storage().instance().set(&key, &vault);

        env.events().publish(
            (Symbol::new(&env, "milestone_validated"), vault_id),
            (),
        );

        Ok(true)
    }

    pub fn release_funds(env: Env, vault_id: u32, usdc_token: Address) -> Result<bool, Error> {
        let key = DataKey::Vault(vault_id);
        let mut vault: ProductivityVault = env
            .storage()
            .instance()
            .get(&key)
            .ok_or(Error::VaultNotFound)?;

        if vault.status != VaultStatus::Active {
            return Err(Error::VaultNotActive);
        }

        let now = env.ledger().timestamp();
        let deadline_reached = now >= vault.end_timestamp;

        if !vault.milestone_validated && !deadline_reached {
            return Err(Error::NotAuthorized);
        }

        let token_client = token::Client::new(&env, &usdc_token);
        token_client.transfer(
            &env.current_contract_address(),
            &vault.success_destination,
            &vault.amount,
        );

        vault.status = VaultStatus::Completed;
        env.storage().instance().set(&key, &vault);

        env.events().publish(
            (Symbol::new(&env, "funds_released"), vault_id),
            vault.amount,
        );

        Ok(true)
    }

    pub fn redirect_funds(env: Env, vault_id: u32, usdc_token: Address) -> Result<bool, Error> {
        let key = DataKey::Vault(vault_id);
        let mut vault: ProductivityVault = env
            .storage()
            .instance()
            .get(&key)
            .ok_or(Error::VaultNotFound)?;

        if vault.status != VaultStatus::Active {
            return Err(Error::VaultNotActive);
        }

        if env.ledger().timestamp() < vault.end_timestamp {
            return Err(Error::InvalidTimestamp);
        }

        if vault.milestone_validated {
            return Err(Error::NotAuthorized);
        }

        let token_client = token::Client::new(&env, &usdc_token);
        token_client.transfer(
            &env.current_contract_address(),
            &vault.failure_destination,
            &vault.amount,
        );

        vault.status = VaultStatus::Failed;
        env.storage().instance().set(&key, &vault);

        env.events().publish(
            (Symbol::new(&env, "funds_redirected"), vault_id),
            vault.amount,
        );

        Ok(true)
    }

    pub fn cancel_vault(env: Env, vault_id: u32, usdc_token: Address) -> Result<bool, Error> {
        let key = DataKey::Vault(vault_id);
        let mut vault: ProductivityVault = env
            .storage()
            .instance()
            .get(&key)
            .ok_or(Error::VaultNotFound)?;

        vault.creator.require_auth();

        if vault.status != VaultStatus::Active {
            return Err(Error::VaultNotActive);
        }

        let token_client = token::Client::new(&env, &usdc_token);
        token_client.transfer(
            &env.current_contract_address(),
            &vault.creator,
            &vault.amount,
        );

        vault.status = VaultStatus::Cancelled;
        env.storage().instance().set(&key, &vault);

        env.events().publish(
            (Symbol::new(&env, "vault_cancelled"), vault_id),
            (),
        );

        Ok(true)
    }

    pub fn get_vault_state(env: Env, vault_id: u32) -> Option<ProductivityVault> {
        env.storage().instance().get(&DataKey::Vault(vault_id))
    }

    pub fn vault_count(env: Env) -> u32 {
        env.storage()
            .instance()
            .get(&DataKey::VaultCount)
            .unwrap_or(0)
    }
}
