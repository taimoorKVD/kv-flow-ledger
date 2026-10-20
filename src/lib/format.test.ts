import { describe, expect, it } from "vitest";
import { compactTokens, dur, margin, money } from "./format";

describe("format helpers", () => {
  it("formats money from minor units", () => {
    expect(money(12345)).toBe("$123.45");
    expect(money(-50)).toBe("-$0.50");
    expect(money(150050, "PKR")).toBe("Rs 1,500.50");
  });

  it("formats duration", () => {
    expect(dur(3_600_000)).toBe("1h 0m");
  });

  it("shows unavailable tokens", () => {
    expect(compactTokens(null, false)).toBe("Unavailable");
    expect(compactTokens(1500, true)).toBe("2K");
  });

  it("margin n/a", () => {
    expect(margin(null)).toBe("N/A");
    expect(margin(12.34)).toBe("12.3%");
  });
});
