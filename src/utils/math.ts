/**
 * MNMX Mathematical Primitives
 *
 * Pure-function arithmetic for AMM swap computation, slippage estimation,
 * concentrated-liquidity math, and basis-point conversions.  All token
 * amounts use native BigInt to avoid precision loss.
 */

const BPS_DENOMINATOR = 10_000n;
const Q64 = 1n << 64n;

// ── Basis-Point Helpers ─────────────────────────────────────────────

/** Convert basis points (integer) to a decimal fraction. */
export function bpsToDecimal(bps: number): number {
  return bps / 10_000;
}

/** Convert a decimal fraction to basis points (rounded). */
export function decimalToBps(dec: number): number {
  return Math.round(dec * 10_000);
}

// ── Constant-Product AMM ────────────────────────────────────────────
