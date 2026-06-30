//! Buyback event emission.
//!
//! `percolator-stake` had no prior structured-event convention (only `msg!`),
//! so this module establishes one for the two buyback events. Each event is
//! emitted as a single `sol_log_data` chunk:
//!
//! ```text
//!   [8-byte event discriminator][field bytes ...]
//! ```
//!
//! Off-chain consumers base64-decode the `Program data:` log line, match the
//! leading 8-byte discriminator to select the event type, and decode the
//! REMAINING bytes. The staged SDK decoders (`decodeBuybackTriggered` /
//! `decodeLiquidityLocked`) consume exactly that field section — no
//! discriminator, no length framing, no other envelope.
//!
//! Scalars are little-endian; pubkeys are raw 32 bytes. The field order/types
//! match the SDK decoders byte-for-byte (INTEGRATION.md
//! `## dcccrypto/percolator-stake` step 7). Layout is append-only from here:
//! fields may be added at the tail, never reordered or removed.

use solana_program::{log::sol_log_data, pubkey::Pubkey};

/// Event discriminator for `BuybackTriggered` ("BBTRIGv1").
pub const BUYBACK_TRIGGERED_DISCRIMINATOR: [u8; 8] = *b"BBTRIGv1";
/// Event discriminator for `LiquidityLocked` ("BBLOCKv1").
pub const LIQUIDITY_LOCKED_DISCRIMINATOR: [u8; 8] = *b"BBLOCKv1";

/// Byte length of the `BuybackTriggered` field section (no discriminator) —
/// matches the SDK `BUYBACK_TRIGGERED_BYTE_LENGTH`.
pub const BUYBACK_TRIGGERED_DATA_LEN: usize = 8 + 32 + 8 + 8 + 8 + 16 + 32;
/// Byte length of the `LiquidityLocked` field section (no discriminator) —
/// matches the SDK `LIQUIDITY_LOCKED_BYTE_LENGTH`.
pub const LIQUIDITY_LOCKED_DATA_LEN: usize = 32 + 8 + 8 + 8 + 8 + 8 + 32 + 16;

// Compile-time locks: the field-section lengths must equal the SDK decoders'
// pinned wire sizes (INTEGRATION.md step 7). A field added/removed here without
// updating the SDK fails the build, not only `cargo test`.
const _: () = assert!(BUYBACK_TRIGGERED_DATA_LEN == 112);
const _: () = assert!(LIQUIDITY_LOCKED_DATA_LEN == 120);

/// `BuybackTriggered` — emitted after `trigger_buyback` reserves a slice.
/// Field order/types match the SDK `decodeBuybackTriggered` byte-for-byte.
#[derive(Debug, Clone, Copy)]
pub struct BuybackTriggered {
    /// Solana Clock `unix_timestamp` at trigger landing.
    pub timestamp: i64,
    /// The market's bound buyback token mint.
    pub token_mint: Pubkey,
    /// `BuybackTreasury` balance before this event (collateral base units).
    pub treasury_balance_before: u64,
    /// Amount credited to stakers by the reserve-first step (collateral base units).
    pub reserve_topup: u64,
    /// Amount reserved for the round-trip (collateral base units).
    pub slice: u64,
    /// Q-format exposure at trigger (this market's exposure; observability only).
    pub market_exposure: u128,
    /// The `BuybackTreasury` account holding the reserved slice.
    pub buyback_treasury: Pubkey,
}

impl BuybackTriggered {
    /// Serialize to `[8-byte discriminator][field bytes]`. The field section
    /// (`bytes[8..]`) is exactly what the SDK decoder consumes.
    pub fn serialize(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(8 + BUYBACK_TRIGGERED_DATA_LEN);
        buf.extend_from_slice(&BUYBACK_TRIGGERED_DISCRIMINATOR);
        buf.extend_from_slice(&self.timestamp.to_le_bytes());
        buf.extend_from_slice(self.token_mint.as_ref());
        buf.extend_from_slice(&self.treasury_balance_before.to_le_bytes());
        buf.extend_from_slice(&self.reserve_topup.to_le_bytes());
        buf.extend_from_slice(&self.slice.to_le_bytes());
        buf.extend_from_slice(&self.market_exposure.to_le_bytes());
        buf.extend_from_slice(self.buyback_treasury.as_ref());
        buf
    }

    /// Emit the event via `sol_log_data`.
    pub fn emit(&self) {
        let data = self.serialize();
        sol_log_data(&[data.as_slice()]);
    }
}

/// `LiquidityLocked` — emitted after `settle_buyback` validates the round-trip.
/// Field order/types match the SDK `decodeLiquidityLocked` byte-for-byte.
#[derive(Debug, Clone, Copy)]
pub struct LiquidityLocked {
    /// The market's bound buyback token mint.
    pub token_mint: Pubkey,
    /// Original slice in collateral base units.
    pub slice: u64,
    /// Pair-asset base units from the convert leg (the slice itself when no conversion).
    pub pair_acquired: u64,
    /// Buyback token purchased on the bound pool (base units).
    pub token_bought: u64,
    /// Pair-asset base units paired with the bought token for add-LP.
    pub pair_paired: u64,
    /// Token-2022 LP tokens destroyed.
    pub lp_tokens_burned: u64,
    /// The market's bound pool — equals `BuybackConfig.pool` post-validation.
    pub pool_pubkey: Pubkey,
    /// `token_bought * 10^12 / pair_paired` (Q12 ratio); `u128::MAX` when
    /// `pair_paired` was 0 (the SDK's `REALIZED_TOKEN_PER_PAIR_SENTINEL`).
    pub realized_token_per_pair: u128,
}

impl LiquidityLocked {
    /// Serialize to `[8-byte discriminator][field bytes]`. The field section
    /// (`bytes[8..]`) is exactly what the SDK decoder consumes.
    pub fn serialize(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(8 + LIQUIDITY_LOCKED_DATA_LEN);
        buf.extend_from_slice(&LIQUIDITY_LOCKED_DISCRIMINATOR);
        buf.extend_from_slice(self.token_mint.as_ref());
        buf.extend_from_slice(&self.slice.to_le_bytes());
        buf.extend_from_slice(&self.pair_acquired.to_le_bytes());
        buf.extend_from_slice(&self.token_bought.to_le_bytes());
        buf.extend_from_slice(&self.pair_paired.to_le_bytes());
        buf.extend_from_slice(&self.lp_tokens_burned.to_le_bytes());
        buf.extend_from_slice(self.pool_pubkey.as_ref());
        buf.extend_from_slice(&self.realized_token_per_pair.to_le_bytes());
        buf
    }

    /// Emit the event via `sol_log_data`.
    pub fn emit(&self) {
        let data = self.serialize();
        sol_log_data(&[data.as_slice()]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_discriminators_distinct() {
        // Distinct from each other and from the account discriminators in
        // state.rs (BBST_V1\0 / BBCF_V1\0), so a consumer can never confuse an
        // event log with an account or the two events with each other.
        assert_ne!(
            BUYBACK_TRIGGERED_DISCRIMINATOR,
            LIQUIDITY_LOCKED_DISCRIMINATOR
        );
    }

    #[test]
    fn buyback_triggered_byte_layout() {
        // Distinct per-field values land at the exact offsets the SDK
        // decodeBuybackTriggered reads (LE scalars, raw 32-byte pubkeys).
        let token_mint = Pubkey::new_unique();
        let buyback_treasury = Pubkey::new_unique();
        let ev = BuybackTriggered {
            timestamp: 0x0102_0304_0506_0708,
            token_mint,
            treasury_balance_before: 0x1111_1111_1111_1111,
            reserve_topup: 0x2222_2222_2222_2222,
            slice: 0x3333_3333_3333_3333,
            market_exposure: 0x4444_4444_4444_4444_5555_5555_5555_5555,
            buyback_treasury,
        };
        let b = ev.serialize();
        assert_eq!(b.len(), 8 + BUYBACK_TRIGGERED_DATA_LEN);
        assert_eq!(&b[0..8], &BUYBACK_TRIGGERED_DISCRIMINATOR);
        assert_eq!(&b[8..16], &0x0102_0304_0506_0708i64.to_le_bytes());
        assert_eq!(&b[16..48], token_mint.as_ref());
        assert_eq!(&b[48..56], &0x1111_1111_1111_1111u64.to_le_bytes());
        assert_eq!(&b[56..64], &0x2222_2222_2222_2222u64.to_le_bytes());
        assert_eq!(&b[64..72], &0x3333_3333_3333_3333u64.to_le_bytes());
        assert_eq!(
            &b[72..88],
            &0x4444_4444_4444_4444_5555_5555_5555_5555u128.to_le_bytes()
        );
        assert_eq!(&b[88..120], buyback_treasury.as_ref());
    }

    #[test]
    fn liquidity_locked_byte_layout() {
        let token_mint = Pubkey::new_unique();
        let pool_pubkey = Pubkey::new_unique();
        let ev = LiquidityLocked {
            token_mint,
            slice: 0x1111_1111_1111_1111,
            pair_acquired: 0x2222_2222_2222_2222,
            token_bought: 0x3333_3333_3333_3333,
            pair_paired: 0x4444_4444_4444_4444,
            lp_tokens_burned: 0x5555_5555_5555_5555,
            pool_pubkey,
            realized_token_per_pair: 0x6666_6666_6666_6666_7777_7777_7777_7777,
        };
        let b = ev.serialize();
        assert_eq!(b.len(), 8 + LIQUIDITY_LOCKED_DATA_LEN);
        assert_eq!(&b[0..8], &LIQUIDITY_LOCKED_DISCRIMINATOR);
        assert_eq!(&b[8..40], token_mint.as_ref());
        assert_eq!(&b[40..48], &0x1111_1111_1111_1111u64.to_le_bytes());
        assert_eq!(&b[48..56], &0x2222_2222_2222_2222u64.to_le_bytes());
        assert_eq!(&b[56..64], &0x3333_3333_3333_3333u64.to_le_bytes());
        assert_eq!(&b[64..72], &0x4444_4444_4444_4444u64.to_le_bytes());
        assert_eq!(&b[72..80], &0x5555_5555_5555_5555u64.to_le_bytes());
        assert_eq!(&b[80..112], pool_pubkey.as_ref());
        assert_eq!(
            &b[112..128],
            &0x6666_6666_6666_6666_7777_7777_7777_7777u128.to_le_bytes()
        );
    }
}
