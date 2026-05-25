import { describe, expect, it } from "vitest";
import { createDeepSeekTextFilter, parseDsmlToolCalls } from "./deepseek-text-filter.js";

function filteredText(chunks: readonly string[]) {
  const filter = createDeepSeekTextFilter();
  return [...chunks.flatMap((chunk) => filter.push(chunk)), ...filter.flush()].join("");
}

describe("createDeepSeekTextFilter", () => {
  it.each([
    {
      name: "tool_use_error in visible text",
      chunks: [
        "before <｜DSML｜tool_use_error><tool_name>write</tool_name></｜DSML｜tool_use_error> after",
      ],
      expected: "before  after",
    },
    {
      name: "split open token",
      chunks: ["before ", "<｜DS", "ML｜tool_calls>body</｜DSML｜tool_calls>", " after"],
      expected: "before  after",
    },
    {
      name: "singular tool_call close",
      chunks: ["<|DSML|tool_call>read</|DSML|tool_call> visible"],
      expected: " visible",
    },
    {
      name: "singular open plural close",
      chunks: ["<|DS", "ML|tool_call>read\n", "</|DSML|tool_calls>"],
      expected: "",
    },
    {
      name: "unterminated block",
      chunks: ["visible <｜DSML｜tool_calls>partial body, no close"],
      expected: "visible ",
    },
    {
      name: "multiple blocks",
      chunks: [
        "a<｜DSML｜tool_use_error>x</｜DSML｜tool_use_error>b<｜DSML｜function_calls>y</｜DSML｜function_calls>c",
      ],
      expected: "abc",
    },
  ])("drops DSML: $name", ({ chunks, expected }) => {
    const text = filteredText(chunks);
    expect(text).toBe(expected);
    expect(text).not.toContain("DSML");
  });

  it("holds a partial open token until it can classify it", () => {
    const filter = createDeepSeekTextFilter();
    const mid = filter.push("safe text<｜DSM");
    expect(mid.join("")).toBe("safe text");

    const all = [
      ...mid,
      ...filter.push("L｜tool_calls>body</｜DSML｜tool_calls> done"),
      ...filter.flush(),
    ];
    expect(all.join("")).toBe("safe text done");
  });

  it("emits normal short text immediately", () => {
    const filter = createDeepSeekTextFilter();
    expect(filter.push("hello")).toEqual(["hello"]);
    expect(filter.flush()).toEqual([]);
  });

  it("captures DSML tool call content for recovery", () => {
    const filter = createDeepSeekTextFilter();
    filter.push(
      'before <｜DSML｜tool_calls><｜DSML｜invoke name="session_status"><｜DSML｜parameter name="sessionKey" string="true">current</｜DSML｜parameter></｜DSML｜invoke></｜DSML｜tool_calls> after',
    );
    filter.flush();
    const calls = filter.recoveredToolCalls();
    expect(calls).toHaveLength(1);
    expect(calls[0].name).toBe("session_status");
    expect(calls[0].arguments).toEqual({ sessionKey: "current" });
  });

  it("recovers tool calls from streamed DSML chunks", () => {
    const filter = createDeepSeekTextFilter();
    filter.push("<｜DSML｜tool_calls><｜DSML｜invo");
    filter.push('ke name="read_file"><｜DSML｜parameter name="path"');
    filter.push(
      ' string="true">/tmp/test.ts</｜DSML｜parameter></｜DSML｜invoke></｜DSML｜tool_calls>',
    );
    filter.flush();
    const calls = filter.recoveredToolCalls();
    expect(calls).toHaveLength(1);
    expect(calls[0].name).toBe("read_file");
    expect(calls[0].arguments).toEqual({ path: "/tmp/test.ts" });
  });

  it("returns no recovered calls when no DSML tool markup is present", () => {
    const filter = createDeepSeekTextFilter();
    filter.push("just normal text without DSML");
    filter.flush();
    expect(filter.recoveredToolCalls()).toEqual([]);
  });

  it("recovers multiple tool calls from a single DSML block", () => {
    const filter = createDeepSeekTextFilter();
    filter.push(
      '<｜DSML｜tool_calls><｜DSML｜invoke name="tool_a"><｜DSML｜parameter name="x" string="true">1</｜DSML｜parameter></｜DSML｜invoke><｜DSML｜invoke name="tool_b"><｜DSML｜parameter name="y" string="true">2</｜DSML｜parameter></｜DSML｜invoke></｜DSML｜tool_calls>',
    );
    filter.flush();
    const calls = filter.recoveredToolCalls();
    expect(calls).toHaveLength(2);
    expect(calls[0].name).toBe("tool_a");
    expect(calls[1].name).toBe("tool_b");
  });
});

describe("parseDsmlToolCalls", () => {
  it("parses a single invoke with string parameter", () => {
    const raw =
      '<｜DSML｜invoke name="session_status"><｜DSML｜parameter name="sessionKey" string="true">current</｜DSML｜parameter></｜DSML｜invoke>';
    const calls = parseDsmlToolCalls(raw);
    expect(calls).toEqual([{ name: "session_status", arguments: { sessionKey: "current" } }]);
  });

  it("parses numeric parameter values as numbers", () => {
    const raw =
      '<｜DSML｜invoke name="set_value"><｜DSML｜parameter name="count">42</｜DSML｜parameter></｜DSML｜invoke>';
    const calls = parseDsmlToolCalls(raw);
    expect(calls[0].arguments).toEqual({ count: 42 });
  });

  it("handles ASCII pipe delimiters", () => {
    const raw =
      '<|DSML|invoke name="tool"><|DSML|parameter name="key">val</|DSML|parameter></|DSML|invoke>';
    const calls = parseDsmlToolCalls(raw);
    expect(calls).toEqual([{ name: "tool", arguments: { key: "val" } }]);
  });

  it("returns empty array for non-invoke DSML content", () => {
    expect(parseDsmlToolCalls("<tool_name>write</tool_name>")).toEqual([]);
  });

  it("handles invoke with no parameters", () => {
    const raw = '<｜DSML｜invoke name="no_args"></｜DSML｜invoke>';
    const calls = parseDsmlToolCalls(raw);
    expect(calls).toEqual([{ name: "no_args", arguments: {} }]);
  });
});
