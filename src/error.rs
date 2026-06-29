use crate::buyback::BuybackBlocker;
use solana_program::program_error::ProgramError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum StakeError {
    /// Pool already initialized for this slab
    AlreadyInitialized = 0,
    /// Pool not initialized
    NotInitialized = 1,
    /// Unauthorized — not admin
    Unauthorized = 2,
    /// Cooldown period not elapsed
    CooldownNotElapsed = 3,
    /// Insufficient LP tokens
    InsufficientLpTokens = 4,
    /// Zero amount
    ZeroAmount = 5,
    /// Arithmetic overflow
    Overflow = 6,
    /// Invalid mint — LP mint mismatch
    InvalidMint = 7,
    /// Market is resolved — no new deposits
    MarketResolved = 8,
    /// Deposit cap exceeded
    DepositCapExceeded = 9,
    /// Invalid PDA derivation
    InvalidPda = 10,
    /// Deprecated (was AdminAlreadyTransferred) — code kept for stable numbering
    _DeprecatedAdminAlreadyTransferred = 11,
    /// Deprecated (was AdminNotTransferred) — code kept for stable numbering
    _DeprecatedAdminNotTransferred = 12,
    /// Insufficient vault balance for withdrawal
    InsufficientVaultBalance = 13,
    /// Invalid percolator program ID
    InvalidPercolatorProgram = 14,
    /// CPI to percolator failed
    CpiFailed = 15,
    /// Invalid account ownership
    InvalidAccount = 16,
    /// Pool mode mismatch (e.g., AccrueFees on insurance pool)
    InvalidPoolMode = 17,
    /// Withdrawal blocked: would breach HWM floor
    WithdrawalBelowHwmFloor = 18,
    /// Tranches not enabled on this pool
    TrancheNotEnabled = 19,
    /// Junior tranche has insufficient balance for this operation
    JuniorBalanceInsufficient = 20,
    /// Wrong tranche — deposit PDA already belongs to a different tranche
    WrongTranche = 21,
    /// S-4: A deposit would mint zero LP shares (amount too small relative to
    /// share price, or degenerate pool state). Rejected explicitly so a deposit
    /// can never silently mint 0 LP while collateral is transferred in. Distinct
    /// from ZeroAmount (which means the requested amount itself was 0).
    ZeroSharesMinted = 22,
    /// Two-step admin rotation: no pending admin proposal exists (or it was
    /// cancelled), so AcceptAdmin has nothing to accept.
    NoPendingAdmin = 23,
    /// Junior tranche deposits are paused while an insurance loss is outstanding
    /// (total_flushed > total_returned). A junior depositing during an open claim
    /// would inherit a pre-existing loss it was never exposed to (and the mirror
    /// case could snipe the recovery). Deposits resume once insurance is returned.
    InsuranceLossOutstanding = 24,
    /// #242 timelock: a `cooldown_slots` INCREASE must go through the two-phase
    /// timelock (ProposeCooldownIncrease → wait TIMELOCK_SLOTS → CommitCooldownIncrease),
    /// not the immediate UpdateConfig path. A decrease or unchanged value is still
    /// allowed via UpdateConfig.
    CooldownIncreaseRequiresTimelock = 25,
    /// #242 timelock: CommitCooldownIncrease was called before TIMELOCK_SLOTS had
    /// elapsed since the proposal. LP holders are still inside their exit window.
    TimelockNotElapsed = 26,
    /// #242 timelock: CommitCooldownIncrease / CancelCooldownIncrease with no active
    /// proposal (cooldown_proposed_at_slot == 0).
    NoPendingCooldownProposal = 27,

    // ── Buyback gate-failure reasons (codes 28..34) ──────────────────────────
    // One per math-crate `BuybackBlocker` variant, in the same canonical order.
    // These are the on-chain Custom codes the keeper/SDK pin; the math crate's
    // own `BuybackBlocker` discriminants (0..6) are NOT the wire codes, and the
    // base is 28 here — NOT an Anchor 6000-style offset.
    /// trigger_buyback: less than the 24h cooldown since the last trigger.
    BuybackCooldownActive = 28,
    /// trigger_buyback: the BuybackTreasury balance is at or below its floor.
    BuybackBelowTreasuryFloor = 29,
    /// trigger_buyback: the market is paying a haircut on positive PnL (stress).
    BuybackHaircutsActive = 30,
    /// trigger_buyback: the market is otherwise distressed and auto-paused.
    BuybackAutoPausedUnderStress = 31,
    /// trigger_buyback: a reserve-first staker top-up is owed/in-flight; the
    /// buyback yields to it this slot.
    BuybackReserveTopUpPending = 32,
    /// trigger_buyback: the market has zero live exposure (not a real market).
    BuybackExposureBelowMinimum = 33,
    /// trigger_buyback: a checked_* arithmetic op failed (fail-closed bucket).
    BuybackMathOverflow = 34,
}

impl From<StakeError> for ProgramError {
    fn from(e: StakeError) -> Self {
        ProgramError::Custom(e as u32)
    }
}

/// Map a math-crate buyback gate failure to its on-chain error. Callers in the
/// trigger handler use `.map_err(StakeError::from)?`, which surfaces as
/// `ProgramError::Custom(28..34)` — the codes the keeper/SDK pin.
impl From<BuybackBlocker> for StakeError {
    fn from(b: BuybackBlocker) -> Self {
        match b {
            BuybackBlocker::CooldownActive => StakeError::BuybackCooldownActive,
            BuybackBlocker::BelowTreasuryFloor => StakeError::BuybackBelowTreasuryFloor,
            BuybackBlocker::HaircutsActive => StakeError::BuybackHaircutsActive,
            BuybackBlocker::AutoPausedUnderStress => StakeError::BuybackAutoPausedUnderStress,
            BuybackBlocker::ReserveTopUpPending => StakeError::BuybackReserveTopUpPending,
            BuybackBlocker::ExposureBelowMinimum => StakeError::BuybackExposureBelowMinimum,
            BuybackBlocker::MathOverflow => StakeError::BuybackMathOverflow,
        }
    }
}

// Compile-time lock on the buyback gate-failure error codes (base 28). The
// keeper and SDK pin these Custom codes; a reorder that shifts them fails the
// build, not only `cargo test`.
const _: () = {
    assert!(StakeError::BuybackCooldownActive as u32 == 28);
    assert!(StakeError::BuybackBelowTreasuryFloor as u32 == 29);
    assert!(StakeError::BuybackHaircutsActive as u32 == 30);
    assert!(StakeError::BuybackAutoPausedUnderStress as u32 == 31);
    assert!(StakeError::BuybackReserveTopUpPending as u32 == 32);
    assert!(StakeError::BuybackExposureBelowMinimum as u32 == 33);
    assert!(StakeError::BuybackMathOverflow as u32 == 34);
};

/// Get user-friendly hint text for an error code.
/// Useful for off-chain clients and SDKs to provide actionable error guidance.
pub fn error_hint(code: u32) -> &'static str {
    match code {
        0 => "Pool already initialized — use a different slab address or check if InitPool was already called",
        1 => "Pool not initialized — call InitPool first to create the stake pool",
        2 => "Unauthorized — you must be the pool admin to perform this action",
        3 => "Cooldown not elapsed — wait for the cooldown period before withdrawing again",
        4 => "Insufficient LP tokens — you don't have enough LP tokens to burn",
        5 => "Zero amount — deposit and withdrawal amounts must be greater than zero",
        6 => "Arithmetic overflow — pool values exceeded u64 bounds, operation blocked",
        7 => "Invalid mint — LP mint doesn't match the pool's LP mint",
        8 => "Market is resolved — no new deposits allowed after resolution",
        9 => "Deposit cap exceeded — pool has reached its maximum deposit limit",
        10 => "Invalid PDA — account is not a valid PDA for the expected seed",
        11 => "Admin already transferred — transfer admin is a one-time operation",
        12 => "Admin not yet transferred — call TransferAdmin before performing admin operations",
        13 => "Insufficient vault balance — vault doesn't have enough collateral for this withdrawal",
        14 => "Invalid percolator program — percolator program ID doesn't match",
        15 => "CPI to percolator failed — the cross-program invoke to percolator failed",
        16 => "Invalid account — account is not owned by the expected program or is not writable",
        17 => "Pool mode mismatch — operation not valid for this pool's mode (e.g., AccrueFees on insurance pool)",
        18 => "Withdrawal blocked — would breach high-water mark floor protection",
        19 => "Tranches not enabled — senior/junior tranches are not enabled on this pool",
        20 => "Junior balance insufficient — junior tranche doesn't have enough balance for this operation",
        21 => "Wrong tranche — deposit already belongs to a different tranche",
        22 => "Zero shares minted — deposit amount too small to mint any LP at the current share price; increase the amount",
        23 => "No pending admin — there is no admin transfer to accept (propose one first, or it was cancelled)",
        24 => "Insurance loss outstanding — junior tranche deposits are paused until the flushed insurance is returned (total_flushed > total_returned)",
        28 => "Buyback cooldown active — less than 24h since the last buyback trigger for this market",
        29 => "Buyback below treasury floor — the buyback treasury is at or below its floor; nothing to spend",
        30 => "Buyback haircuts active — the market is paying a haircut on positive PnL; buyback paused under stress",
        31 => "Buyback auto-paused — the market is distressed; the fee accrues to the staker reserve instead",
        32 => "Buyback reserve top-up pending — stakers are credited toward the reserve target first this slot",
        33 => "Buyback exposure below minimum — the market has no live open interest to buy back against",
        34 => "Buyback math overflow — a checked arithmetic operation failed in the buyback gate",
        _ => "Unknown error — check the error code and pool state",
    }
}

#[cfg(test)]
mod tests {
    use super::{error_hint, StakeError};
    use crate::buyback::BuybackBlocker;
    use solana_program::program_error::ProgramError;

    #[test]
    fn buyback_blocker_maps_to_base_28_codes() {
        // The keeper/SDK pin these on-chain Custom codes. Base is 28 — NOT the
        // math crate's 0..6 BuybackBlocker discriminants, NOT an Anchor 6000 base.
        let cases = [
            (
                BuybackBlocker::CooldownActive,
                StakeError::BuybackCooldownActive,
                28u32,
            ),
            (
                BuybackBlocker::BelowTreasuryFloor,
                StakeError::BuybackBelowTreasuryFloor,
                29,
            ),
            (
                BuybackBlocker::HaircutsActive,
                StakeError::BuybackHaircutsActive,
                30,
            ),
            (
                BuybackBlocker::AutoPausedUnderStress,
                StakeError::BuybackAutoPausedUnderStress,
                31,
            ),
            (
                BuybackBlocker::ReserveTopUpPending,
                StakeError::BuybackReserveTopUpPending,
                32,
            ),
            (
                BuybackBlocker::ExposureBelowMinimum,
                StakeError::BuybackExposureBelowMinimum,
                33,
            ),
            (
                BuybackBlocker::MathOverflow,
                StakeError::BuybackMathOverflow,
                34,
            ),
        ];
        for (blocker, expected_err, code) in cases {
            assert_eq!(StakeError::from(blocker), expected_err);
            assert_eq!(expected_err as u32, code);
            assert_eq!(ProgramError::from(expected_err), ProgramError::Custom(code));
            assert_ne!(
                error_hint(code),
                "Unknown error — check the error code and pool state"
            );
        }
    }
}
