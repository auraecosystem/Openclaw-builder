const DSML_KINDS = ["tool_use_error", "tool_calls", "tool_call", "function_calls"] as const;
const DSML_BARS = ["|", "｜"] as const;

const DSML_OPEN_TOKENS = DSML_BARS.flatMap((bar) =>
  DSML_KINDS.map((kind) => `<${bar}DSML${bar}${kind}>`),
);
const DSML_CLOSE_TOKENS = DSML_BARS.flatMap((bar) =>
  DSML_KINDS.map((kind) => `</${bar}DSML${bar}${kind}>`),
);
const MAX_OPEN_TOKEN_LEN = Math.max(...DSML_OPEN_TOKENS.map((token) => token.length));
const MAX_CLOSE_TOKEN_LEN = Math.max(...DSML_CLOSE_TOKENS.map((token) => token.length));

export interface DsmlToolCall {
  name: string;
  arguments: Record<string, unknown>;
}

export interface DeepSeekTextFilter {
  push(chunk: string): string[];
  flush(): string[];
  /** Returns tool calls recovered from DSML markup that was stripped from visible text. */
  recoveredToolCalls(): DsmlToolCall[];
}

export function createDeepSeekTextFilter(): DeepSeekTextFilter {
  let buffer = "";
  let insideDsml = false;
  let dsmlCapture = "";
  const capturedBlocks: string[] = [];

  const consume = (final: boolean): string[] => {
    const output: string[] = [];
    const emit = (text: string) => {
      if (text) {
        output.push(text);
      }
    };

    while (buffer) {
      if (insideDsml) {
        const close = findEarliestToken(buffer, DSML_CLOSE_TOKENS);
        if (close) {
          dsmlCapture += buffer.slice(0, close.index);
          capturedBlocks.push(dsmlCapture);
          dsmlCapture = "";
          buffer = buffer.slice(close.index + close.token.length);
          insideDsml = false;
          continue;
        }
        const keep = final ? 0 : Math.min(buffer.length, MAX_CLOSE_TOKEN_LEN - 1);
        dsmlCapture += buffer.slice(0, buffer.length - keep);
        buffer = buffer.slice(buffer.length - keep);
        if (final) {
          if (dsmlCapture) {
            capturedBlocks.push(dsmlCapture);
            dsmlCapture = "";
          }
          insideDsml = false;
        }
        return output;
      }

      const open = findEarliestToken(buffer, DSML_OPEN_TOKENS);
      if (open) {
        emit(buffer.slice(0, open.index));
        buffer = buffer.slice(open.index + open.token.length);
        insideDsml = true;
        dsmlCapture = "";
        continue;
      }

      if (final) {
        emit(buffer);
        buffer = "";
        return output;
      }

      const keep = longestDsmlOpenPrefixSuffixLength(buffer);
      const emitLength = buffer.length - keep;
      if (emitLength <= 0) {
        return output;
      }
      emit(buffer.slice(0, emitLength));
      buffer = buffer.slice(emitLength);
      return output;
    }
    return output;
  };

  return {
    push(chunk: string) {
      buffer += chunk;
      return consume(false);
    },
    flush() {
      return consume(true);
    },
    recoveredToolCalls() {
      const toolCalls: DsmlToolCall[] = [];
      for (const block of capturedBlocks) {
        toolCalls.push(...parseDsmlToolCalls(block));
      }
      return toolCalls;
    },
  };
}

/**
 * Parse DSML tool call markup into structured tool calls.
 *
 * The markup uses both `|` and `｜` (fullwidth) bar delimiters:
 *   <｜DSML｜invoke name="tool_name">
 *     <｜DSML｜parameter name="key" string="true">value</｜DSML｜parameter>
 *   </｜DSML｜invoke>
 */
export function parseDsmlToolCalls(raw: string): DsmlToolCall[] {
  const calls: DsmlToolCall[] = [];
  const invokePattern =
    /<[|｜]DSML[|｜]invoke\s+name="([^"]+)"[^>]*>([\s\S]*?)<\/[|｜]DSML[|｜]invoke>/g;
  const paramPattern =
    /<[|｜]DSML[|｜]parameter\s+name="([^"]+)"([^>]*)>([\s\S]*?)<\/[|｜]DSML[|｜]parameter>/g;

  let invokeMatch: RegExpExecArray | null;
  while ((invokeMatch = invokePattern.exec(raw)) !== null) {
    const name = invokeMatch[1];
    const body = invokeMatch[2];
    const args: Record<string, unknown> = {};

    let paramMatch: RegExpExecArray | null;
    paramPattern.lastIndex = 0;
    while ((paramMatch = paramPattern.exec(body)) !== null) {
      const paramName = paramMatch[1];
      const paramAttrs = paramMatch[2];
      const paramValue = paramMatch[3].trim();
      if (/\bstring="true"/.test(paramAttrs)) {
        args[paramName] = paramValue;
      } else {
        try {
          args[paramName] = JSON.parse(paramValue);
        } catch {
          args[paramName] = paramValue;
        }
      }
    }

    if (name) {
      calls.push({ name, arguments: args });
    }
  }

  return calls;
}

function findEarliestToken(text: string, tokens: readonly string[]) {
  let best: { index: number; token: string } | null = null;
  for (const token of tokens) {
    const index = text.indexOf(token);
    if (index !== -1 && (!best || index < best.index)) {
      best = { index, token };
    }
  }
  return best;
}

function longestDsmlOpenPrefixSuffixLength(text: string) {
  const maxLength = Math.min(text.length, MAX_OPEN_TOKEN_LEN - 1);
  for (let length = maxLength; length > 0; length--) {
    const suffix = text.slice(text.length - length);
    if (DSML_OPEN_TOKENS.some((token) => token.startsWith(suffix))) {
      return length;
    }
  }
  return 0;
}
