# Disciplr Vault Contract Documentation

## Error Reference

All public contract methods return `Result<T, Error>`. The `Error` enum is defined with `#[contracterror]` and maps to on-chain error codes.

| Code | Variant | Description |
|------|---------|-------------|
| `#1` | `VaultNotFound` | No vault record exists for the given `vault_id`. |
| `#2` | `NotAuthorized` | Caller is not authorized (e.g. release before deadline without validation, redirect when milestone was validated). |
| `#3` | `VaultNotActive` | Vault is not in `Active` status — already `Completed`, `Failed`, or `Cancelled`. |
| `#4` | `InvalidTimestamp` | Timestamp constraint violated (e.g. `start_timestamp` is in the past, or redirect called before `end_timestamp`). |
| `#5` | `MilestoneExpired` | Validation rejected because `current_time >= end_timestamp`. |
| `#6` | `InvalidStatus` | Vault is in an invalid status for the requested operation (reserved for future use). |
| `#7` | `InvalidAmount` | Amount is outside the allowed range (`< MIN_AMOUNT` or `> MAX_AMOUNT`). |
| `#8` | `InvalidTimestamps` | `start_timestamp` is not strictly less than `end_timestamp`. |
| `#9` | `DurationTooLong` | Vault duration (`end − start`) exceeds `MAX_VAULT_DURATION` (1 year). |

> Panics are reserved exclusively for invariant violations that indicate a bug in the contract itself (e.g. authorization failures enforced by the Soroban runtime). All user-facing error paths return structured `Error` variants.

---

## Overview

The Disciplr Vault is a Soroban smart contract deployed on the Stellar blockchain that enables **programmable time-locked USDC vaults** for productivity-based milestone funding. It allows creators to lock USDC tokens with specific milestones and conditions, ensuring funds are only released upon verified completion or redirected to a failure destination if milestones are not met.

### Use Cases

- **Vesting schedules**: Lock tokens that vest over time based on milestone completion
- **Grant funding**: Enable grant providers to fund projects with accountability
- **Team incentives**: Align team compensation with deliverable completion
- **Bug bounties**: Create time-bound bounty programs with predefined payout conditions

---

## Data Model

### VaultStatus Enum

Represents the current state of a vault:

```rust
#[contracttype]
pub enum VaultStatus {
    Active = 0,      // Vault created and funds locked
    Completed = 1,  // Milestone validated, funds released to success destination
    Failed = 2,     // Milestone not completed by deadline, funds redirected
    Cancelled = 3,  // Vault cancelled by creator, funds returned
}
```

| Status | Description |
|--------|-------------|
| `Active` | Vault is live, waiting for milestone validation or deadline |
| `Completed` | Milestone verified, funds released to success destination |
| `Failed` | Deadline passed without validation, funds redirected |
| `Cancelled` | Creator cancelled vault, funds returned |

### ProductivityVault Struct

The main data structure representing a vault:

```rust
#[contracttype]
pub struct ProductivityVault {
    pub creator: Address,           // Address that created the vault
    pub amount: i128,                // Amount of USDC locked (in stroops)
    pub start_timestamp: u64,       // Unix timestamp when vault becomes active
    pub end_timestamp: u64,          // Unix deadline for milestone validation
    pub milestone_hash: BytesN<32>, // SHA-256 hash of milestone requirements
    pub verifier: Option<Address>,  // Optional trusted verifier address
    pub success_destination: Address, // Address for fund release on success
    pub failure_destination: Address, // Address for fund redirect on failure
    pub status: VaultStatus,        // Current vault status
}
```

| Field | Type | Description |
|-------|------|-------------|
| `creator` | `Address` | Wallet address that created the vault |
| `amount` | `i128` | Total USDC amount locked (in stroops, 1 USDC = 10^7 stroops) |
| `start_timestamp` | `u64` | Unix timestamp (seconds) when vault becomes active |
| `end_timestamp` | `u64` | Unix timestamp (seconds) deadline for milestone validation |
| `milestone_hash` | `BytesN<32>` | SHA-256 hash documenting milestone requirements |
| `verifier` | `Option<Address>` | Optional trusted party who can validate milestones |
| `success_destination` | `Address` | Recipient address on successful milestone completion |
| `failure_destination` | `Address` | Recipient address when milestone is not completed |
| `status` | `VaultStatus` | Current lifecycle state of the vault |

---

## Contract Methods

### `create_vault`

Creates a new productivity vault and locks USDC funds.

```rust
pub fn create_vault(
    env: Env,
    usdc_token: Address,
    creator: Address,
    amount: i128,
    start_timestamp: u64,
    end_timestamp: u64,
    milestone_hash: BytesN<32>,
    verifier: Option<Address>,
    success_destination: Address,
    failure_destination: Address,
) -> Result<u32, Error>
```

**Parameters:**
- `usdc_token`: Address of the USDC token contract
- `creator`: Address of the vault creator (must authorize transaction)
- `amount`: USDC amount to lock in stroops (`MIN_AMOUNT..=MAX_AMOUNT`)
- `start_timestamp`: When vault becomes active (unix seconds, must be >= current ledger time)
- `end_timestamp`: Deadline for milestone validation (must be > `start_timestamp`)
- `milestone_hash`: SHA-256 hash of milestone document
- `verifier`: Optional verifier address (`None` means only creator can validate)
- `success_destination`: Address to receive funds on success
- `failure_destination`: Address to receive funds on failure

**Returns:** `Result<u32, Error>` — unique vault ID on success

**Errors:**
- `Error::InvalidAmount` — `amount < MIN_AMOUNT` or `amount > MAX_AMOUNT`
- `Error::InvalidTimestamp` — `start_timestamp` is in the past
- `Error::InvalidTimestamps` — `end_timestamp <= start_timestamp`
- `Error::DurationTooLong` — duration exceeds `MAX_VAULT_DURATION`

**Emits:** [`vault_created`](#vault_created) event

---

### `validate_milestone`

Verifier (or authorized party) validates milestone completion. Sets `milestone_validated = true`, enabling early release via `release_funds`.

```rust
pub fn validate_milestone(env: Env, vault_id: u32) -> Result<bool, Error>
```

**Parameters:**
- `vault_id`: ID of the vault to validate

**Returns:** `Result<bool, Error>` — `true` on success

**Authorization:** If `verifier` is `Some(addr)`, only that address may call this. If `verifier` is `None`, only the `creator` may call it.

**Errors:**
- `Error::VaultNotFound` — vault does not exist
- `Error::VaultNotActive` — vault is not in `Active` status
- `Error::MilestoneExpired` — `current_time >= end_timestamp`

**Emits:** [`milestone_validated`](#milestone_validated) event

---

### `release_funds`

Releases locked funds to `success_destination`. Allowed after milestone validation or once the deadline has passed.

```rust
pub fn release_funds(env: Env, vault_id: u32, usdc_token: Address) -> Result<bool, Error>
```

**Parameters:**
- `vault_id`: ID of the vault to release funds from
- `usdc_token`: Address of the USDC token contract

**Returns:** `Result<bool, Error>` — `true` on success

**Errors:**
- `Error::VaultNotFound` — vault does not exist
- `Error::VaultNotActive` — vault is not in `Active` status
- `Error::NotAuthorized` — milestone not validated and deadline not yet reached

---

### `redirect_funds`

Redirects funds to `failure_destination` when the deadline passes without milestone validation.

```rust
pub fn redirect_funds(env: Env, vault_id: u32, usdc_token: Address) -> Result<bool, Error>
```

**Parameters:**
- `vault_id`: ID of the vault to redirect funds from
- `usdc_token`: Address of the USDC token contract

**Returns:** `Result<bool, Error>` — `true` on success

**Errors:**
- `Error::VaultNotFound` — vault does not exist
- `Error::VaultNotActive` — vault is not in `Active` status
- `Error::InvalidTimestamp` — deadline has not yet passed
- `Error::NotAuthorized` — milestone was already validated (funds should go to success)

---

### `cancel_vault`

Allows the creator to cancel the vault and retrieve locked funds.

```rust
pub fn cancel_vault(env: Env, vault_id: u32, usdc_token: Address) -> Result<bool, Error>
```

**Parameters:**
- `vault_id`: ID of the vault to cancel
- `usdc_token`: Address of the USDC token contract

**Returns:** `Result<bool, Error>` — `true` on success

**Authorization:** Only the vault `creator` may call this.

**Errors:**
- `Error::VaultNotFound` — vault does not exist
- `Error::VaultNotActive` — vault is not in `Active` status

---

### `get_vault_state`

Retrieves the current state of a vault.

```rust
pub fn get_vault_state(env: Env, vault_id: u32) -> Option<ProductivityVault>
```

**Parameters:**
- `vault_id`: ID of the vault to query

**Returns:** `Option<ProductivityVault>` - Stored vault data when a record exists for that ID.

**Behavior:** Created vault records are not deleted during normal contract execution. Completed, failed, and cancelled vaults still return `Some(ProductivityVault)` with their terminal status. `None` therefore means the ID was never assigned (`vault_id >= vault_count()`) or storage was cleared outside the contract's normal lifecycle.

---

## Events

### `vault_created`

Emitted when a new vault is created.

**Topic:**
```
("vault_created", vault_id)
```

**Data:**
```rust
ProductivityVault {
    creator: Address,
    amount: i128,
    start_timestamp: u64,
    end_timestamp: u64,
    milestone_hash: BytesN<32>,
    verifier: Option<Address>,
    success_destination: Address,
    failure_destination: Address,
    status: VaultStatus::Active,
}
```

---

### `milestone_validated`

Emitted when a milestone is successfully validated.

**Topic:**
```
("milestone_validated", vault_id)
```

**Data:** `()` (empty tuple)

### `funds_released`

Emitted when funds are released to `success_destination`.

**Topic:** `("funds_released", vault_id)`  
**Data:** `i128` — amount transferred

---

### `funds_redirected`

Emitted when funds are redirected to `failure_destination`.

**Topic:** `("funds_redirected", vault_id)`  
**Data:** `i128` — amount transferred

---

### `vault_cancelled`

Emitted when a vault is cancelled by the creator.

**Topic:** `("vault_cancelled", vault_id)`  
**Data:** `()` (empty tuple)

---

```
                    ┌──────────────┐
                    │   CREATED    │
                    │              │
                    │ create_vault │
                    └──────┬───────┘
                           │
                           ▼
                    ┌──────────────┐
         ┌─────────│    ACTIVE    │─────────┐
         │         │              │         │
         │         └──────────────┘         │
         │                                    │
         ▼                                    ▼
┌─────────────────┐              ┌─────────────────────┐
│ validate_       │              │  redirect_funds     │
│ milestone()     │              │  (deadline passed)  │
└────────┬────────┘              └──────────┬──────────┘
         │                                   │
         ▼                                   ▼
┌─────────────────┐              ┌─────────────────────┐
│   COMPLETED    │              │      FAILED        │
│                │              │                     │
└─────────────────┘              └─────────────────────┘

         │
         ▼
┌─────────────────┐
│ cancel_vault()  │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│   CANCELLED    │
│                │
└─────────────────┘
```

---

## Security and Trust Model

This section outlines the security assumptions, trust model, and known limitations of the Disciplr Vault contract. It is intended for auditors, developers, and users to understand the risks and guarantees provided by the system.

### Trust Model

1. **Verifier Trust (Critical)**: When a `verifier` is designated (via `Some(Address)`), that address has **absolute power** to validate the milestone and cause funds to be released to the `success_destination` before the deadline. If the verifier is compromised or malicious, they can release funds prematurely or to a non-compliant recipient.
2. **Creator Power**: If no `verifier` is set (`None`), only the `creator` can validate the milestone. Additionally, the `creator` can cancel the vault at any time to reclaim funds, assuming the vault is still `Active`.
3. **Immutable Destinations**: Once a vault is created, the `success_destination` and `failure_destination` are immutable. This prevents redirection of funds after the vault is funded, assuming the core contract logic remains secure.

### Security Assumptions

1. **Stellar Ledger Integrity**: We assume the underlying Stellar blockchain and Soroban runtime correctly enforce authorization (`require_auth`) and maintain state integrity.
2. **Ledger Timestamp**: The contract relies on `env.ledger().timestamp()` for all time-based logic (start/end windows). We assume ledger timestamps are reasonably accurate and monotonic as per Stellar network consensus.
3. **Token Contract Behavior**: The contract interacts with a USDC token contract (standard Soroban token interface). We assume the token contract is honest and follows the expected transfer behavior.

### Known Limitations & Risks

1. **USDC Token Address Consistency**: The `usdc_token` address is **not stored** in the vault data. Instead, it is passed as an argument to methods like `release_funds`, `redirect_funds`, and `cancel_vault`.
   > [!WARNING]
   > There is a risk that a caller provides a different token address than the one used during vault creation. Users should verify the token contract used in interactions matches the intended asset.
2. **CEI Pattern Violations**: Some methods perform token transfers **before** updating the internal vault status. While Soroban's atomicity mitigates some traditional reentrancy risks, following the Checks-Effects-Interactions (CEI) pattern more strictly is a recommended enhancement for future versions.
3. **No Administrative Overrides**: There is no "admin" or "owner" role with the power to rescue funds from a stalled vault (e.g., if a verifier loses their key and the deadline is far in the future). Funds are strictly bound by the `end_timestamp` and authorization rules.
4. **Lack of Reentrancy Guards**: The contract does not currently implement explicit reentrancy guards, relying instead on the synchronous and atomic nature of Soroban contract calls.

### Recommendations for Integration

- **Off-chain Verification**: The `milestone_hash` should represent a clear, legally or technically binding document that both creator and verifier agree upon.
- **Multisig Verifiers**: For high-value vaults, we highly recommend using a multisig address (G-address or contract-based account) as the `verifier`.

---

## Usage Examples

### Example 1: Create a Milestone-Based Funding Vault

A project owner wants to lock 1000 USDC for a bug bounty program with a 30-day deadline.

```rust
// Parameters
let creator: Address = Address::from_string("GA7..."); // Creator wallet
let amount: i128 = 1000 * 10_000_000; // 1000 USDC in stroops
let start_timestamp: u64 = 1704067200; // Jan 1, 2024 00:00:00 UTC
let end_timestamp: u64 = 1706640000;    // Jan 30, 2024 00:00:00 UTC (30 days)

// Hash of milestone requirements (off-chain document)
let milestone_hash: BytesN<32> = BytesN::from_array(&env, &[
    0x4d, 0x69, 0x6c, 0x65, 0x73, 0x74, 0x6f, 0x6e,
    0x65, 0x5f, 0x72, 0x65, 0x71, 0x75, 0x69, 0x72,
    0x65, 0x6d, 0x65, 0x6e, 0x74, 0x73, 0x5f, 0x68,
    0x61, 0x73, 0x68, 0x5f, 0x65, 0x78, 0x61, 0x6d
]);

let verifier: Option<Address> = Some(Address::from_string("GB7..."));
let success_destination: Address = Address::from_string("GC7..."); // Project wallet
let failure_destination: Address = Address::from_string("GD7..."); // Funder wallet

// Create vault
let vault_id = DisciplrVaultClient::new(&env, &contract_address)
    .create_vault(
        &creator,
        &amount,
        &start_timestamp,
        &end_timestamp,
        &milestone_hash,
        &verifier,
        &success_destination,
        &failure_destination,
    );
// vault_id = 0
```

### Example 2: Validate Milestone and Release Funds

The verifier validates that milestone requirements were met and releases funds.

```rust
let verifier: Address = Address::from_string("GB7..."); // Designated verifier

let result = DisciplrVaultClient::new(&env, &contract_address)
    .with_source_account(&verifier)
    .validate_milestone(&vault_id);
// result = true

// Funds now transferred to success_destination
// Vault status changed to Completed
```

### Example 3: Handle Deadline Without Validation

After the deadline passes without milestone validation, funds are redirected.

```rust
// Assume end_timestamp has passed and no validation occurred

let result = DisciplrVaultClient::new(&env, &contract_address)
    .redirect_funds(&vault_id);
// result = true

// Funds transferred to failure_destination
// Vault status changed to Failed
```

### Example 4: Cancel Vault Before Deadline

Creator decides to cancel the vault before the deadline.

```rust
let creator: Address = Address::from_string("GA7..."); // Original creator

let result = DisciplrVaultClient::new(&env, &contract_address)
    .with_source_account(&creator)
    .cancel_vault(&vault_id);
// result = true

// Funds returned to creator
// Vault status changed to Cancelled
```

### Example 5: Query Vault State

Check the current state of a vault.

```rust
let vault_state = DisciplrVaultClient::new(&env, &contract_address)
    .get_vault_state(&vault_id);

// Returns Some(ProductivityVault) or None
match vault_state {
    Some(vault) => {
        // Access vault fields
        let current_status = vault.status;
        let amount_locked = vault.amount;
    }
    None => {
        // Vault not found or not initialized
    }
}
```

---

## Testing

Run the test suite to verify contract functionality:

```bash
cargo test
```

Expected output should include tests for:
- Vault creation with valid parameters
- Vault creation authorization
- Event emission on vault creation
- Milestone validation logic
- Fund release and redirect logic
- Vault cancellation
- State retrieval

---

## File Structure

```
disciplr-contracts/
├── src/
│   └── lib.rs           # DisciplrVault contract implementation
├── Cargo.toml           # Project dependencies
├── README.md            # Project overview
└── vesting.md           # This documentation
```

---

## Related Documentation

- [Soroban SDK Documentation](https://developers.stellar.org/docs/smart-contracts)
- [Stellar Smart Contracts Guide](https://developers.stellar.org/docs/smart-contracts/getting-started)
- [Token Interface (Soroban)](https://developers.stellar.org/docs/tokens)

---

## Changelog

| Version | Changes |
|---------|---------|
| 0.1.0 | Initial release with basic vault structure |
| 0.2.0 | All user-facing paths return `Result<T, Error>` — no panics in vault API. Added `Error` enum with 9 documented codes. Removed stale panic-based test file. Added `test_validate_milestone_rejects_non_active_statuses`. Resolved merge conflict in security section. Updated method signatures and error table in docs. |
