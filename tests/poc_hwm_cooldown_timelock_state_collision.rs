//! PoC / regression — the #242 cooldown-increase timelock fields collide with the
//! PERC-313 high-water-mark fields in `StakePool._reserved`.
//!
//! ── The bug ──────────────────────────────────────────────────────────────────
//! PERC-313 HWM occupies `_reserved[10..32]`:
//!   [10]    hwm_enabled
//!   [11..13] hwm_floor_bps
//!   [16..24] epoch_high_water_tvl
//!   [24..32] hwm_last_epoch
//!
//! The #242 cooldown-increase timelock stores its two fields in the SAME region:
//!   [10..18] pending_cooldown_slots     (overlaps hwm_enabled, hwm_floor_bps, and the
//!                                         low bytes of epoch_high_water_tvl)
//!   [18..26] cooldown_proposed_at_slot  (overlaps epoch_high_water_tvl and the low bytes
//!                                         of hwm_last_epoch)
//!
//! The #242 doc-comment claims `[10..32]` is "previously-free", but PERC-313 reserved and
//! uses that whole region. As a result the two features corrupt each other's state:
//!   * Proposing a cooldown increase silently disables/garbles HWM (the withdrawal-drain
//!     floor — an LP protection).
//!   * `refresh_hwm` (invoked on EVERY deposit/withdraw while HWM is enabled) corrupts a
//!     pending cooldown proposal's value and timer.
//!
//! This is the same class of defect the codebase previously flagged CRITICAL for the
//! byte-9 `market_resolved`/`hwm_enabled` collision. This test asserts the independence
//! invariant using the program's real accessors; it fails on the pre-fix layout that
//! packed both features into `_reserved` — revert the dedicated fields and it goes red
//! again.

use bytemuck::Zeroable;
use percolator_stake::state::StakePool;

#[test]
fn hwm_and_cooldown_timelock_state_must_be_independent() {
    let mut pool = StakePool::zeroed();
    pool.set_discriminator();

    // Admin enables HWM with a real floor and seeds a water mark (as refresh_hwm would).
    pool.set_hwm_enabled(true);
    pool.set_hwm_floor_bps(5_000);
    pool.set_epoch_high_water_tvl(1_000_000);
    pool.set_hwm_last_epoch(42);

    // Admin proposes a cooldown increase (#242 two-phase timelock).
    pool.set_pending_cooldown_slots(500_000);
    pool.set_cooldown_proposed_at_slot(123_456);

    // The cooldown proposal must not have destroyed HWM state — the features are independent.
    assert!(pool.hwm_enabled(), "cooldown proposal disabled HWM (byte 10 collision)");
    assert_eq!(
        pool.hwm_floor_bps(),
        5_000,
        "cooldown proposal corrupted HWM floor (bytes 11-12 collision)"
    );
    assert_eq!(
        pool.epoch_high_water_tvl(),
        1_000_000,
        "cooldown proposal corrupted HWM water mark (bytes 16-23 collision)"
    );
    assert_eq!(
        pool.hwm_last_epoch(),
        42,
        "cooldown proposal corrupted HWM epoch (bytes 24-25 collision)"
    );

    // And the cooldown proposal itself must be intact.
    assert_eq!(pool.pending_cooldown_slots(), 500_000);
    assert_eq!(pool.cooldown_proposed_at_slot(), 123_456);
}

#[test]
fn hwm_refresh_must_not_corrupt_a_pending_cooldown_proposal() {
    let mut pool = StakePool::zeroed();
    pool.set_discriminator();

    // A cooldown increase is pending.
    pool.set_pending_cooldown_slots(500_000);
    pool.set_cooldown_proposed_at_slot(123_456);

    // HWM is enabled; a deposit/withdraw triggers refresh_hwm, which writes
    // epoch_high_water_tvl[16..24] and hwm_last_epoch[24..32] — overlapping the
    // cooldown proposal bytes.
    pool.set_hwm_enabled(true);
    pool.set_hwm_floor_bps(5_000);
    pool.refresh_hwm(43, 2_000_000);

    assert_eq!(
        pool.pending_cooldown_slots(),
        500_000,
        "HWM refresh corrupted pending_cooldown_slots"
    );
    assert_eq!(
        pool.cooldown_proposed_at_slot(),
        123_456,
        "HWM refresh corrupted cooldown_proposed_at_slot (timelock timer)"
    );
}
