# USDC Integration and Trust Model

This document records the external trust assumptions for using production USDC with the Disciplr vault contract.

## Scope

The current contract stores vault metadata and enforces local authorization and timing rules. It does not yet move production USDC on-chain. When token transfer logic is added, the safety of a vault will depend on both:

- The Disciplr contract logic in this repository.
- The administrative powers of the USDC asset issuer and any admin roles attached to the deployed Stellar asset contract.

## Core Trust Assumptions

### 1. Disciplr does not control USDC monetary policy

Disciplr is an application-layer escrow workflow. It does not mint, redeem, freeze, claw back, or upgrade the external USDC asset. Those powers remain with the USDC issuer / administrator defined by the production asset.

Implication for integrators:
- A correctly functioning Disciplr contract cannot remove issuer-level powers from the underlying USDC asset.
- Contract users still inherit the operational and governance risk of the production USDC asset.

### 2. The issuer / admin can be more powerful than the escrow contract

On Stellar, the asset issuer and related administrative controls may be able to pause distribution, freeze balances, enforce blacklisting, or otherwise affect transferability depending on the asset configuration and issuer policy.

Implication for integrators:
- A vault reaching `Completed`, `Failed`, or `Cancelled` does not guarantee settlement if the asset itself is frozen or administratively restricted.
- Incident response must treat issuer-side actions as a separate failure domain from Disciplr contract bugs.

### 3. Asset upgrades or admin changes are a security event

If production USDC is migrated to a new issuer, wrapped representation, or upgraded Stellar asset contract with different admin keys or policy, the integration assumptions in this repository must be re-reviewed before new vaults are created.

Implication for operators:
- Treat asset address changes, issuer account rotations, admin-key rotations, and policy changes as release-blocking events.
- Re-validate the production asset identifier and admin model before deployment and after any announced migration.

## Required Deployment Checks

Before using production USDC, operators should confirm:

1. The exact Stellar asset or Stellar Asset Contract address being integrated.
2. The issuer account and any admin-capable roles attached to that asset.
3. Whether the asset can be frozen, clawed back, blacklisted, or paused.
4. Whether Circle or another issuer can migrate users to a new asset representation.
5. Whether off-chain redemption delays or compliance actions can block settlement.

## Contract-Level Security Notes

The contract can only guarantee the following local properties:

- The creator must authorize vault creation.
- `create_vault` rejects zero amounts and invalid time windows.
- `validate_milestone` only succeeds for an active vault before the deadline.
- `validate_milestone` requires the configured verifier when present, otherwise the creator.
- `cancel_vault` and `redirect_funds` only operate on active vaults.

The contract cannot guarantee:

- That the integrated USDC asset will remain transferable.
- That issuer/admin policies will stay unchanged after deployment.
- That off-chain redemption or compliance workflows will settle instantly.

## Reviewer Notes

Auditors and integrators should review this file together with:

- [README.md](README.md)
- [vesting.md](vesting.md)
- [src/lib.rs](src/lib.rs)

## Primary References

- Stellar Docs: https://developers.stellar.org/docs/tokens/how-to-issue-an-asset
- Circle USDC risk factors: https://www.circle.com/legal/usdc-risk-factors
