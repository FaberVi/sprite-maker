import { describe, expect, test } from "bun:test";
import { formatElapsedDuration } from "../src/lib/elapsed-duration";

describe("formatElapsedDuration", () => {
  test("formats sub-minute durations", () => {
    expect(formatElapsedDuration(0)).toBe("0:00");
    expect(formatElapsedDuration(5_400)).toBe("0:05");
    expect(formatElapsedDuration(59_999)).toBe("0:59");
  });

  test("formats minute durations", () => {
    expect(formatElapsedDuration(60_000)).toBe("1:00");
    expect(formatElapsedDuration(154_000)).toBe("2:34");
  });

  test("formats hour durations", () => {
    expect(formatElapsedDuration(3_661_000)).toBe("1:01:01");
  });
});
