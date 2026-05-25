import { describe, expect, it } from "vitest";
import { formatFastModeLabel } from "./status-labels.js";

describe("formatFastModeLabel", () => {
  it("shows fast mode when enabled", () => {
    expect(formatFastModeLabel(true)).toBe("Fast");
  });

  it("shows auto fast mode", () => {
    expect(formatFastModeLabel("auto")).toBe("Fast:auto");
  });

  it("hides fast mode when disabled", () => {
    expect(formatFastModeLabel(false)).toBeNull();
  });
});
