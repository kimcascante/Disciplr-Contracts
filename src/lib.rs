#![no_std]
#![allow(clippy::too_many_arguments)]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, Address, BytesN, Env, Symbol,
};

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    VaultNotFound = 1,
    VaultNotActive = 2,
    InvalidTimestamp = 3,
    MilestoneExpired = 4,
}

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
}

#[contracttype]
pub enum DataKey {
    NextVaultId,
    Vault(u32),
}

#[contract]
pub struct DisciplrVault;

fn load_vault(env: &Env, vault_id: u32) -> Result<ProductivityVault, Error> {
    env.storage()
        .persistent()
        .get(&DataKey::Vault(vault_id))
        .ok_or(Error::VaultNotFound)
}

fn save_vault(env: &Env, vault_id: u32, vault: &ProductivityVault) {
    env.storage()
        .persistent()
        .set(&DataKey::Vault(vault_id), vault);
}

#[contractimpl]
impl DisciplrVault {
    /// Creates a vault record and emits a `vault_created` event.
    ///
    /// Security assumptions:
    /// - `creator` must authorize the call.
    /// - The caller is expected to have approved the external USDC asset transfer separately.
    /// - This contract does not verify issuer or asset-admin policy; that trust model is documented in
    ///   `USDC_INTEGRATION.md` and must be validated by deployers.
    ///
    /// Panics if:
    /// - `amount <= 0`
    /// - `end_timestamp <= start_timestamp`
    pub fn create_vault(
        env: Env,
        creator: Address,
        amount: i128,
        start_timestamp: u64,
        end_timestamp: u64,
        milestone_hash: BytesN<32>,
        verifier: Option<Address>,
        success_destination: Address,
        failure_destination: Address,
    ) -> u32 {
        creator.require_auth();

        if amount <= 0 {
            panic!("amount must be positive");
        }

        if end_timestamp <= start_timestamp {
            panic!("create_vault: start_timestamp must be strictly less than end_timestamp");
        }

        let vault_id: u32 = env
            .storage()
            .instance()
            .get(&DataKey::NextVaultId)
            .unwrap_or(0);
        let vault = ProductivityVault {
            creator,
            amount,
            start_timestamp,
            end_timestamp,
            milestone_hash,
            verifier,
            success_destination,
            failure_destination,
            status: VaultStatus::Active,
        };

        save_vault(&env, vault_id, &vault);
        env.storage()
            .instance()
            .set(&DataKey::NextVaultId, &(vault_id + 1));
        env.events()
            .publish((Symbol::new(&env, "vault_created"), vault_id), vault);

        vault_id
    }

    /// Marks a vault as completed before its deadline.
    ///
    /// Authorization model:
    /// - If `verifier` is set, that address must authorize.
    /// - Otherwise the `creator` must authorize.
    pub fn validate_milestone(env: Env, vault_id: u32) -> Result<bool, Error> {
        let mut vault = load_vault(&env, vault_id)?;

        if vault.status != VaultStatus::Active {
            return Err(Error::VaultNotActive);
        }

        if let Some(verifier) = vault.verifier.clone() {
            verifier.require_auth();
        } else {
            vault.creator.require_auth();
        }

        if env.ledger().timestamp() >= vault.end_timestamp {
            return Err(Error::MilestoneExpired);
        }

        vault.status = VaultStatus::Completed;
        save_vault(&env, vault_id, &vault);
        env.events()
            .publish((Symbol::new(&env, "milestone_validated"), vault_id), ());

        Ok(true)
    }

    /// Marks a vault as completed.
    ///
    /// This is currently a metadata/state transition only. Token movement remains an integration task.
    pub fn release_funds(env: Env, vault_id: u32) -> Result<bool, Error> {
        let mut vault = load_vault(&env, vault_id)?;

        if vault.status != VaultStatus::Active {
            return Err(Error::VaultNotActive);
        }

        vault.status = VaultStatus::Completed;
        save_vault(&env, vault_id, &vault);

        Ok(true)
    }

    /// Marks a vault as failed once its deadline has passed.
    pub fn redirect_funds(env: Env, vault_id: u32) -> Result<bool, Error> {
        let mut vault = load_vault(&env, vault_id)?;

        if vault.status != VaultStatus::Active {
            return Err(Error::VaultNotActive);
        }

        if env.ledger().timestamp() < vault.end_timestamp {
            return Err(Error::InvalidTimestamp);
        }

        vault.status = VaultStatus::Failed;
        save_vault(&env, vault_id, &vault);

        Ok(true)
    }

    /// Cancels an active vault.
    ///
    /// Only the original creator may cancel the vault.
    pub fn cancel_vault(env: Env, vault_id: u32) -> Result<bool, Error> {
        let mut vault = load_vault(&env, vault_id)?;
        vault.creator.require_auth();

        if vault.status != VaultStatus::Active {
            return Err(Error::VaultNotActive);
        }

        vault.status = VaultStatus::Cancelled;
        save_vault(&env, vault_id, &vault);

        Ok(true)
    }

    /// Returns the currently persisted state for `vault_id`, if it exists.
    pub fn get_vault_state(env: Env, vault_id: u32) -> Option<ProductivityVault> {
        env.storage().persistent().get(&DataKey::Vault(vault_id))
    }
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::*;
    use soroban_sdk::testutils::{Address as _, Ledger};

    struct Setup {
        env: Env,
        client: DisciplrVaultClient<'static>,
        creator: Address,
        verifier: Address,
        success_destination: Address,
        failure_destination: Address,
    }

    fn setup() -> Setup {
        let env = Env::default();
        let contract_id = env.register(DisciplrVault, ());
        let client = DisciplrVaultClient::new(&env, &contract_id);
        let creator = Address::generate(&env);
        let verifier = Address::generate(&env);
        let success_destination = Address::generate(&env);
        let failure_destination = Address::generate(&env);

        Setup {
            env,
            client,
            creator,
            verifier,
            success_destination,
            failure_destination,
        }
    }

    fn milestone_hash(env: &Env, byte: u8) -> BytesN<32> {
        BytesN::from_array(env, &[byte; 32])
    }

    fn create_default_vault(setup: &Setup, verifier: Option<Address>) -> u32 {
        setup.env.mock_all_auths();
        setup.client.create_vault(
            &setup.creator,
            &1_000_000,
            &1_000,
            &2_000,
            &milestone_hash(&setup.env, 7),
            &verifier,
            &setup.success_destination,
            &setup.failure_destination,
        )
    }

    #[test]
    #[should_panic(expected = "amount must be positive")]
    fn test_create_vault_zero_amount() {
        let setup = setup();
        setup.env.mock_all_auths();

        setup.client.create_vault(
            &setup.creator,
            &0,
            &1_000,
            &2_000,
            &milestone_hash(&setup.env, 1),
            &None,
            &setup.success_destination,
            &setup.failure_destination,
        );
    }

    #[test]
    #[should_panic(
        expected = "create_vault: start_timestamp must be strictly less than end_timestamp"
    )]
    fn create_vault_rejects_start_equal_end() {
        let setup = setup();
        setup.env.mock_all_auths();

        setup.client.create_vault(
            &setup.creator,
            &1_000_000,
            &1_000,
            &1_000,
            &milestone_hash(&setup.env, 2),
            &None,
            &setup.success_destination,
            &setup.failure_destination,
        );
    }

    #[test]
    #[should_panic(
        expected = "create_vault: start_timestamp must be strictly less than end_timestamp"
    )]
    fn create_vault_rejects_start_greater_than_end() {
        let setup = setup();
        setup.env.mock_all_auths();

        setup.client.create_vault(
            &setup.creator,
            &1_000_000,
            &2_000,
            &1_000,
            &milestone_hash(&setup.env, 3),
            &None,
            &setup.success_destination,
            &setup.failure_destination,
        );
    }

    #[test]
    fn test_create_vault_persists_state() {
        let setup = setup();
        setup.env.mock_all_auths();

        let vault_id = setup.client.create_vault(
            &setup.creator,
            &1_000_000,
            &1_000,
            &2_000,
            &milestone_hash(&setup.env, 4),
            &Some(setup.verifier.clone()),
            &setup.success_destination,
            &setup.failure_destination,
        );

        assert_eq!(vault_id, 0);

        let vault = setup.client.get_vault_state(&vault_id).unwrap();
        assert_eq!(vault.creator, setup.creator);
        assert_eq!(vault.verifier, Some(setup.verifier.clone()));
        assert_eq!(vault.status, VaultStatus::Active);
    }

    #[test]
    fn test_create_vault_assigns_incrementing_ids() {
        let setup = setup();
        let first_id = create_default_vault(&setup, Some(setup.verifier.clone()));
        let second_id = create_default_vault(&setup, None);

        assert_eq!(first_id, 0);
        assert_eq!(second_id, 1);
    }

    #[test]
    fn test_validate_milestone_rejects_after_end() {
        let setup = setup();
        let vault_id = create_default_vault(&setup, Some(setup.verifier.clone()));

        setup.env.ledger().set_timestamp(2_000);
        let result = setup.client.try_validate_milestone(&vault_id);
        assert!(result.is_err());

        setup.env.ledger().set_timestamp(2_001);
        let result = setup.client.try_validate_milestone(&vault_id);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_milestone_succeeds_before_end() {
        let setup = setup();
        let vault_id = create_default_vault(&setup, Some(setup.verifier.clone()));

        setup.env.ledger().set_timestamp(1_999);
        let result = setup.client.validate_milestone(&vault_id);
        assert!(result);

        let vault = setup.client.get_vault_state(&vault_id).unwrap();
        assert_eq!(vault.status, VaultStatus::Completed);
    }

    #[test]
    fn test_release_funds_marks_vault_completed() {
        let setup = setup();
        let vault_id = create_default_vault(&setup, None);

        let result = setup.client.release_funds(&vault_id);
        assert!(result);

        let vault = setup.client.get_vault_state(&vault_id).unwrap();
        assert_eq!(vault.status, VaultStatus::Completed);
    }

    #[test]
    fn test_release_funds_rejects_non_existent_vault() {
        let setup = setup();
        let result = setup.client.try_release_funds(&999);
        assert!(result.is_err());
    }

    #[test]
    fn test_redirect_funds_rejects_before_end() {
        let setup = setup();
        let vault_id = create_default_vault(&setup, None);

        setup.env.ledger().set_timestamp(1_999);
        let result = setup.client.try_redirect_funds(&vault_id);
        assert!(result.is_err());
    }

    #[test]
    fn test_redirect_funds_succeeds_after_end() {
        let setup = setup();
        let vault_id = create_default_vault(&setup, None);

        setup.env.ledger().set_timestamp(2_000);
        let result = setup.client.redirect_funds(&vault_id);
        assert!(result);

        let vault = setup.client.get_vault_state(&vault_id).unwrap();
        assert_eq!(vault.status, VaultStatus::Failed);
    }

    #[test]
    fn test_cancel_vault_changes_status() {
        let setup = setup();
        let vault_id = create_default_vault(&setup, None);

        let result = setup.client.cancel_vault(&vault_id);
        assert!(result);

        let vault = setup.client.get_vault_state(&vault_id).unwrap();
        assert_eq!(vault.status, VaultStatus::Cancelled);
    }

    #[test]
    fn test_cancel_vault_rejects_non_active() {
        let setup = setup();
        let vault_id = create_default_vault(&setup, None);
        assert!(setup.client.cancel_vault(&vault_id));

        let result = setup.client.try_cancel_vault(&vault_id);
        assert!(result.is_err());
    }
}
