//! Regression — `ReturnInsurance` must not re-open the last-junior-exit windfall.
//!
//! ── Background (the last-junior-exit settlement) ──────────────────────────────
//! When the LAST junior LP exits while an insurance loss is outstanding,
//! `process_withdraw` "realizes" the loss the junior absorbed
//! (`L = junior_balance − effective_junior_balance`) by booking, in the same
//! instruction:
//!
//!     pool.total_returned          += L;   // phantom — NO tokens move
//!     pool.realized_junior_loss()  += L;   // recorded so total_pool_value() subtracts it
//!
//! The phantom `+L` and the `−realized_junior_loss` term in `total_pool_value()` cancel,
//! so pool value is unchanged at exit and the protected SENIOR tranche is NOT windfalled
//! (see the `#161` unit test `test_161_last_junior_exit_does_not_windfall_recovery_to_senior`
//! and the `realized_junior_loss` doc-comment: the forfeited portion must "sit as DEAD
//! (unclaimable) value rather than windfalling senior").
//!
//! ── The defect this guards against ────────────────────────────────────────────
//! `process_return_insurance` previously computed its returnable cap as
//! `total_flushed − total_returned + realized_junior_loss`, ADDING the realized (dead)
//! portion back. Because the handler then does `total_returned += amount` with no matching
//! `realized_junior_loss` decrement, returning that portion double-counts `total_returned`
//! — pushing it ABOVE `total_flushed` (conservation broken) and canceling the
//! `−realized_junior_loss` term in `total_pool_value()`, so the forfeited junior capital
//! flows to senior. The fix routes BOTH `ReturnInsurance` and `RecoverFlushedInsurance`
//! through `StakePool::insurance_recoverable()` (= `total_flushed − total_returned`), which
//! excludes the already-settled realized portion.
//!
//! This test drives the same scenario as the `#161` unit test one step further into the
//! real returnable cap (`insurance_recoverable()`, the exact value the processor caps
//! against) and the return booking. It uses the program's real accounting methods and the
//! production cap helper, so it fails if the cap ever re-admits the realized/dead portion.

use bytemuck::Zeroable;
use percolator_stake::state::StakePool;

fn tranche_pool() -> StakePool {
    let mut p = StakePool::zeroed();
    p.is_initialized = 1;
    p.set_discriminator();
    p.set_tranche_enabled(true);
    p.set_junior_fee_mult_bps(20_000);
    p // pool_mode 0 (insurance LP)
}

#[test]
fn return_insurance_must_not_windfall_senior_with_realized_junior_loss() {
    let mut pool = tranche_pool();

    // Junior 100k + senior 100k (each mints 1:1 into its sub-pool).
    pool.total_deposited = 200_000;
    pool.total_lp_supply = 200_000;
    pool.set_junior_balance(100_000);
    pool.set_junior_total_lp(100_000);
    assert_eq!(pool.senior_balance().unwrap(), 100_000, "senior 100k pre-loss");
    assert_eq!(pool.effective_junior_balance(), 100_000, "junior 100k pre-loss");

    // Admin flushes 50k — a junior-absorbed loss (net_loss 50k <= junior 100k).
    pool.total_flushed = 50_000;
    assert_eq!(pool.effective_junior_balance(), 50_000, "junior marked down to 50k");
    assert_eq!(
        pool.senior_balance().unwrap(),
        100_000,
        "senior protected by junior first-loss"
    );

    // ── Last junior exits. Mirror process_withdraw's full-exit branch verbatim ──
    // (this part is unchanged by the fix): total_withdrawn += payout; total_lp_supply -=
    // junior_lp; the #161 forfeit booking; then junior_total_lp/balance = 0.
    let junior_payout = pool.effective_junior_balance(); // 50k
    pool.total_withdrawn += junior_payout;
    pool.total_lp_supply -= 100_000;
    let net_loss = pool.total_flushed.saturating_sub(pool.total_returned);
    let forfeited = pool
        .junior_balance()
        .saturating_sub(pool.effective_junior_balance())
        .min(net_loss);
    assert_eq!(forfeited, 50_000, "junior forfeits the 50k loss it absorbed");
    pool.total_returned += forfeited; // phantom settlement — NO tokens moved
    pool.set_realized_junior_loss(pool.realized_junior_loss() + forfeited);
    pool.set_junior_total_lp(0);
    pool.set_junior_balance(0);

    // #161 invariant holds at exit: senior stays at its 100k principal; the forfeited 50k
    // is dead value excluded from total_pool_value().
    assert_eq!(pool.total_pool_value().unwrap(), 100_000, "tpv excludes dead forfeited loss");
    assert_eq!(pool.senior_balance().unwrap(), 100_000, "senior not windfalled at exit");
    assert_eq!(pool.total_returned, pool.total_flushed, "ledger loss settled at exit");
    assert_eq!(pool.realized_junior_loss(), 50_000);

    // ── The returnable cap the processor actually enforces (shared by ReturnInsurance and
    // RecoverFlushedInsurance). The forfeited/dead portion must NOT be returnable. ──
    let outstanding = pool.insurance_recoverable();
    assert_eq!(
        outstanding, 0,
        "forfeited realized_junior_loss must NOT be returnable (got {outstanding})"
    );

    // Simulate the maximum permitted return (ReturnInsurance rejects amount > outstanding,
    // and rejects amount == 0, so nothing is returnable here). Mirror total_returned += amount.
    if outstanding > 0 {
        pool.total_returned += outstanding;
    }

    // ── Invariants that MUST hold ──
    assert!(
        pool.total_returned <= pool.total_flushed,
        "conservation broken: total_returned ({}) exceeds total_flushed ({})",
        pool.total_returned,
        pool.total_flushed
    );
    assert_eq!(
        pool.senior_balance().unwrap(),
        100_000,
        "senior must NOT be windfalled by the forfeited-junior recovery (got {})",
        pool.senior_balance().unwrap()
    );
}
