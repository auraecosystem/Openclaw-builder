import { describe, expect, it } from "vitest";
import { loadWrapperResolutionParityFixtureCases } from "./exec-approvals-test-helpers.js";
import { resolveCommandResolutionFromArgv } from "./exec-approvals.js";

describe("exec approvals wrapper resolution parity fixture", () => {
  const fixtures = loadWrapperResolutionParityFixtureCases();

  it.each(fixtures)("matches wrapper fixture: $id", (fixture) => {
    const resolution = resolveCommandResolutionFromArgv(fixture.argv);
    expect(resolution?.execution.rawExecutable ?? null).toBe(fixture.expectedRawExecutable);
  });
});
