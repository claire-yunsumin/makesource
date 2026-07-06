import { describe, expect, it, vi } from "vitest";
import { resolveComparisonSeed } from "./abCompare";

describe("resolveComparisonSeed", () => {
  it("uses the parsed seed as-is when input is a valid integer", () => {
    expect(resolveComparisonSeed("42", () => 999)).toBe(42);
  });

  it("calls fallbackSeed exactly once and shares that value when input is blank", () => {
    const fallback = vi.fn(() => 777);
    expect(resolveComparisonSeed("", fallback)).toBe(777);
    expect(fallback).toHaveBeenCalledTimes(1);
  });

  it("returns null for non-numeric input instead of guessing", () => {
    expect(resolveComparisonSeed("abc", () => 1)).toBeNull();
  });
});
