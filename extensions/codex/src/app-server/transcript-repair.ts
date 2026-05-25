import fs from "node:fs/promises";

export type CodexRolloutTranscriptRepairResult = {
  scannedFiles: number;
  repairedFiles: number;
  insertedOutputs: number;
};

type ParsedJsonLine = {
  raw: string;
  value?: Record<string, unknown>;
};

type MissingCustomToolCall = {
  lineIndex: number;
  callId: string;
  timestamp?: string;
};

const SYNTHETIC_TOOL_OUTPUT_TEXT =
  "[openclaw] custom tool call was interrupted by OpenClaw before a tool output was recorded; inserted synthetic output during transcript repair.";

export async function repairCodexRolloutMissingCustomToolOutputs(
  files: readonly string[],
): Promise<CodexRolloutTranscriptRepairResult> {
  const result: CodexRolloutTranscriptRepairResult = {
    scannedFiles: 0,
    repairedFiles: 0,
    insertedOutputs: 0,
  };
  const visited = new Set<string>();
  for (const file of files) {
    if (visited.has(file)) {
      continue;
    }
    visited.add(file);
    const repair = await repairCodexRolloutFile(file);
    if (repair.scanned) {
      result.scannedFiles += 1;
    }
    if (repair.insertedOutputs > 0) {
      result.repairedFiles += 1;
      result.insertedOutputs += repair.insertedOutputs;
    }
  }
  return result;
}

async function repairCodexRolloutFile(
  file: string,
): Promise<{ scanned: boolean; insertedOutputs: number }> {
  const scan = await scanCodexRolloutFile(file);
  if (!scan.scanned || scan.missingCalls.length === 0) {
    return { scanned: scan.scanned, insertedOutputs: 0 };
  }
  await rewriteCodexRolloutFileWithSyntheticOutputs(file, scan.missingCalls);
  return { scanned: true, insertedOutputs: scan.missingCalls.length };
}

async function scanCodexRolloutFile(
  file: string,
): Promise<{ scanned: boolean; missingCalls: MissingCustomToolCall[] }> {
  let handle: Awaited<ReturnType<typeof fs.open>>;
  try {
    handle = await fs.open(file, "r");
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code === "ENOENT") {
      return { scanned: false, missingCalls: [] };
    }
    throw error;
  }

  const outputCallIds = new Set<string>();
  const toolCalls: MissingCustomToolCall[] = [];
  try {
    let lineIndex = 0;
    for await (const rawLine of handle.readLines()) {
      const line = parseJsonLine(rawLine);
      const payload = readPayload(line.value);
      const type = readString(payload, "type");
      if (type === "custom_tool_call") {
        const callId = readString(payload, "call_id");
        if (callId) {
          toolCalls.push({
            lineIndex,
            callId,
            timestamp: readString(line.value, "timestamp"),
          });
        }
      } else if (type === "custom_tool_call_output") {
        const callId = readString(payload, "call_id");
        if (callId) {
          outputCallIds.add(callId);
        }
      }
      lineIndex += 1;
    }
  } finally {
    await handle.close();
  }

  return {
    scanned: true,
    missingCalls: toolCalls.filter((call) => !outputCallIds.has(call.callId)),
  };
}

async function rewriteCodexRolloutFileWithSyntheticOutputs(
  file: string,
  missingCalls: readonly MissingCustomToolCall[],
): Promise<void> {
  const missingByIndex = new Map<number, MissingCustomToolCall[]>();
  for (const call of missingCalls) {
    const calls = missingByIndex.get(call.lineIndex) ?? [];
    calls.push(call);
    missingByIndex.set(call.lineIndex, calls);
  }

  const tempFile = `${file}.openclaw-repair-${process.pid}-${Date.now()}.tmp`;
  let readHandle: Awaited<ReturnType<typeof fs.open>> | undefined;
  let writeHandle: Awaited<ReturnType<typeof fs.open>> | undefined;
  try {
    readHandle = await fs.open(file, "r");
    writeHandle = await fs.open(tempFile, "wx");
    let lineIndex = 0;
    for await (const rawLine of readHandle.readLines()) {
      await writeHandle.writeFile(`${rawLine}\n`, "utf8");
      for (const call of missingByIndex.get(lineIndex) ?? []) {
        await writeHandle.writeFile(
          `${JSON.stringify(buildSyntheticCustomToolOutput(call))}\n`,
          "utf8",
        );
      }
      lineIndex += 1;
    }
    await writeHandle.close();
    writeHandle = undefined;
    await readHandle.close();
    readHandle = undefined;
    await fs.rename(tempFile, file);
  } catch (error) {
    await writeHandle?.close().catch(() => undefined);
    await readHandle?.close().catch(() => undefined);
    await fs.rm(tempFile, { force: true }).catch(() => undefined);
    throw error;
  }
}

function parseJsonLine(raw: string): ParsedJsonLine {
  try {
    const value = JSON.parse(raw) as unknown;
    return {
      raw,
      value:
        value && typeof value === "object" && !Array.isArray(value)
          ? (value as Record<string, unknown>)
          : undefined,
    };
  } catch {
    return { raw };
  }
}

function buildSyntheticCustomToolOutput(call: {
  callId: string;
  timestamp?: string;
}): Record<string, unknown> {
  return {
    timestamp: call.timestamp ?? new Date().toISOString(),
    type: "response_item",
    payload: {
      type: "custom_tool_call_output",
      call_id: call.callId,
      output: [
        {
          type: "input_text",
          text: SYNTHETIC_TOOL_OUTPUT_TEXT,
        },
      ],
    },
  };
}

function readPayload(
  value: Record<string, unknown> | undefined,
): Record<string, unknown> | undefined {
  const payload = value?.payload;
  return payload && typeof payload === "object" && !Array.isArray(payload)
    ? (payload as Record<string, unknown>)
    : undefined;
}

function readString(value: Record<string, unknown> | undefined, key: string): string | undefined {
  const raw = value?.[key];
  return typeof raw === "string" && raw.length > 0 ? raw : undefined;
}
