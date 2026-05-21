import fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { afterEach, describe, expect, it, vi } from "vitest";
import { repairCodexRolloutMissingCustomToolOutputs } from "./transcript-repair.js";

const tempDirs: string[] = [];

afterEach(async () => {
  vi.restoreAllMocks();
  for (const dir of tempDirs.splice(0)) {
    await fs.rm(dir, { recursive: true, force: true });
  }
});

async function createRolloutFile(lines: readonly Record<string, unknown>[]): Promise<string> {
  const dir = await fs.mkdtemp(path.join(os.tmpdir(), "openclaw-codex-repair-"));
  tempDirs.push(dir);
  const file = path.join(dir, "rollout-2026-05-20T21-29-59-thread-1.jsonl");
  await fs.writeFile(file, lines.map((line) => JSON.stringify(line)).join("\n") + "\n");
  return file;
}

async function readRecords(file: string): Promise<Array<Record<string, unknown>>> {
  const raw = await fs.readFile(file, "utf8");
  return raw
    .trim()
    .split("\n")
    .filter(Boolean)
    .map((line) => JSON.parse(line) as Record<string, unknown>);
}

function responseItem(payload: Record<string, unknown>): Record<string, unknown> {
  return {
    timestamp: "2026-05-20T21:31:38.500Z",
    type: "response_item",
    payload,
  };
}

describe("repairCodexRolloutMissingCustomToolOutputs", () => {
  it("inserts a synthetic custom tool output after an orphaned custom tool call", async () => {
    const file = await createRolloutFile([
      responseItem({
        type: "custom_tool_call",
        status: "completed",
        call_id: "call_missing",
        name: "exec",
        input: "date",
      }),
      responseItem({ type: "message", role: "assistant", content: "later item" }),
    ]);

    await expect(repairCodexRolloutMissingCustomToolOutputs([file])).resolves.toEqual({
      scannedFiles: 1,
      repairedFiles: 1,
      insertedOutputs: 1,
    });

    const records = await readRecords(file);
    expect(records).toHaveLength(3);
    expect((records[1].payload as Record<string, unknown>).type).toBe("custom_tool_call_output");
    expect((records[1].payload as Record<string, unknown>).call_id).toBe("call_missing");
    expect(JSON.stringify(records[1])).toContain("interrupted by OpenClaw");
    expect((records[2].payload as Record<string, unknown>).type).toBe("message");
  });

  it("leaves already paired custom tool calls unchanged", async () => {
    const file = await createRolloutFile([
      responseItem({
        type: "custom_tool_call",
        status: "completed",
        call_id: "call_ok",
        name: "exec",
        input: "date",
      }),
      responseItem({
        type: "custom_tool_call_output",
        call_id: "call_ok",
        output: [{ type: "input_text", text: "done" }],
      }),
    ]);
    const before = await fs.readFile(file, "utf8");

    await expect(repairCodexRolloutMissingCustomToolOutputs([file])).resolves.toEqual({
      scannedFiles: 1,
      repairedFiles: 0,
      insertedOutputs: 0,
    });

    await expect(fs.readFile(file, "utf8")).resolves.toBe(before);
  });

  it("stream-scans large clean rollouts without whole-file reads or rewrites", async () => {
    const lines: Record<string, unknown>[] = [];
    for (let index = 0; index < 5_000; index += 1) {
      lines.push(
        responseItem({
          type: "message",
          role: "assistant",
          content: `clean history ${index}`,
        }),
      );
    }
    lines.push(
      responseItem({
        type: "custom_tool_call",
        status: "completed",
        call_id: "call_large_ok",
        name: "exec",
        input: "date",
      }),
      responseItem({
        type: "custom_tool_call_output",
        call_id: "call_large_ok",
        output: [{ type: "input_text", text: "done" }],
      }),
    );
    const file = await createRolloutFile(lines);
    const beforeStat = await fs.stat(file);
    const readFileSpy = vi.spyOn(fs, "readFile");
    const renameSpy = vi.spyOn(fs, "rename");

    await expect(repairCodexRolloutMissingCustomToolOutputs([file])).resolves.toEqual({
      scannedFiles: 1,
      repairedFiles: 0,
      insertedOutputs: 0,
    });

    expect(readFileSpy).not.toHaveBeenCalled();
    expect(renameSpy).not.toHaveBeenCalled();
    await expect(fs.stat(file)).resolves.toMatchObject({
      size: beforeStat.size,
      mtimeMs: beforeStat.mtimeMs,
    });
  });
});
