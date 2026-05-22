import { describe, expect, it } from "vitest";
import {
  buildCodexComputerUseMenuPayload,
  buildCodexFastMenuPayload,
  buildCodexPermissionsMenuPayload,
  buildCodexPluginNamePickerEmptyText,
  buildCodexPluginNamePickerPayload,
  buildCodexPluginsMenuPayload,
  buildCodexTopPickerPayload,
  parseCodexCallbackData,
} from "./codex-picker-buttons.js";

describe("parseCodexCallbackData", () => {
  it("parses the top picker callback", () => {
    expect(parseCodexCallbackData("cdx_top")).toEqual({ type: "top" });
  });
  it("parses each sub-menu opener", () => {
    expect(parseCodexCallbackData("cdx_plugins_menu")).toEqual({ type: "plugins_menu" });
    expect(parseCodexCallbackData("cdx_perm_menu")).toEqual({ type: "perm_menu" });
    expect(parseCodexCallbackData("cdx_fast_menu")).toEqual({ type: "fast_menu" });
    expect(parseCodexCallbackData("cdx_cuse_menu")).toEqual({ type: "cuse_menu" });
  });
  it("parses each dynamic plugin name picker callback", () => {
    expect(parseCodexCallbackData("cdx_picker_enable")).toEqual({ type: "picker", verb: "enable" });
    expect(parseCodexCallbackData("cdx_picker_disable")).toEqual({
      type: "picker",
      verb: "disable",
    });
  });
  it("returns null for unsupported plugin verbs (toggle / remove / add not picker-exposed)", () => {
    expect(parseCodexCallbackData("cdx_picker_toggle")).toBeNull();
    expect(parseCodexCallbackData("cdx_picker_remove")).toBeNull();
    expect(parseCodexCallbackData("cdx_picker_add")).toBeNull();
  });
  it("returns null for unknown callbacks (so tgcmd: and mdl_* fall through)", () => {
    expect(parseCodexCallbackData("tgcmd:/codex permissions yolo")).toBeNull();
    expect(parseCodexCallbackData("mdl_prov")).toBeNull();
    expect(parseCodexCallbackData("cdx_picker_unknown")).toBeNull();
    expect(parseCodexCallbackData("cdx_unknown")).toBeNull();
    expect(parseCodexCallbackData("")).toBeNull();
  });
});

describe("buildCodexTopPickerPayload", () => {
  it("returns four buttons in a 2x2 grid: plugins/permissions/account/help", () => {
    const payload = buildCodexTopPickerPayload();
    expect(payload.inline_keyboard).toEqual([
      // Row 1: sub-menu openers (cdx_* for in-place edit on Telegram)
      [
        { text: "plugins", callback_data: "cdx_plugins_menu" },
        { text: "permissions", callback_data: "cdx_perm_menu" },
      ],
      // Row 2: leaf actions (tgcmd: so they fire as chat commands and reply as new messages)
      [
        { text: "account", callback_data: "tgcmd:/codex account" },
        { text: "help", callback_data: "tgcmd:/codex help" },
      ],
    ]);
    expect(payload.text).toContain("Codex commands");
    expect(payload.text).toContain("/codex account");
    expect(payload.text).toContain("/codex help");
    expect(payload.text).toContain("/status, /fast, /help, /stop, /models");
    // Mentions that help reveals the rest of the typeable surface.
    expect(payload.text).toContain("Tap 'help'");
  });
});

describe("buildCodexPluginsMenuPayload", () => {
  it("returns 4 sub-action buttons plus a back row (list + enable + disable + help)", () => {
    const payload = buildCodexPluginsMenuPayload();
    const labels = payload.inline_keyboard.map((row) => row[0]?.text);
    // computer-use is an intentional omission: it auto-populates and appears
    // in the dynamic enable/disable pickers like any other plugin.
    expect(labels).toEqual(["list", "enable", "disable", "help", "← back"]);
    // Navigation buttons use cdx_* (in-place edit).
    expect(payload.inline_keyboard[1]?.[0]?.callback_data).toBe("cdx_picker_enable");
    // Leaf actions (list, help) use tgcmd: so they fire as commands.
    expect(payload.inline_keyboard[0]?.[0]?.callback_data).toBe("tgcmd:/codex plugins list");
    expect(payload.inline_keyboard[3]?.[0]?.callback_data).toBe("tgcmd:/codex plugins help");
    // Back returns to the top picker.
    expect(payload.inline_keyboard[4]?.[0]?.callback_data).toBe("cdx_top");
  });
});

describe("buildCodexPermissionsMenuPayload", () => {
  it("returns 3 mode buttons plus back", () => {
    const payload = buildCodexPermissionsMenuPayload();
    const labels = payload.inline_keyboard.map((row) => row[0]?.text);
    expect(labels).toEqual(["default", "yolo", "status", "← back"]);
    expect(payload.inline_keyboard[1]?.[0]?.callback_data).toBe("tgcmd:/codex permissions yolo");
    expect(payload.inline_keyboard[3]?.[0]?.callback_data).toBe("cdx_top");
  });
});

describe("buildCodexFastMenuPayload", () => {
  it("returns on/off/status plus back", () => {
    const payload = buildCodexFastMenuPayload();
    expect(payload.inline_keyboard).toEqual([
      [{ text: "on", callback_data: "tgcmd:/codex fast on" }],
      [{ text: "off", callback_data: "tgcmd:/codex fast off" }],
      [{ text: "status", callback_data: "tgcmd:/codex fast status" }],
      [{ text: "← back", callback_data: "cdx_top" }],
    ]);
  });
});

describe("buildCodexComputerUseMenuPayload", () => {
  it("returns status/install with back routed to plugins menu (parent-aware)", () => {
    const payload = buildCodexComputerUseMenuPayload();
    expect(payload.inline_keyboard).toEqual([
      [{ text: "status", callback_data: "tgcmd:/codex computer-use status" }],
      [{ text: "install", callback_data: "tgcmd:/codex computer-use install" }],
      [{ text: "← back", callback_data: "cdx_plugins_menu" }],
    ]);
  });
});

describe("buildCodexPluginNamePickerPayload", () => {
  const plugins = {
    chrome: { enabled: true, marketplaceName: "openai-bundled", pluginName: "chrome" },
    "build-ios-apps": {
      enabled: false,
      marketplaceName: "openai-curated",
      pluginName: "build-ios-apps",
    },
    "hugging-face": {
      enabled: true,
      marketplaceName: "openai-curated",
      pluginName: "hugging-face",
    },
  };

  it("filters to enabled-only for disable", () => {
    const payload = buildCodexPluginNamePickerPayload("disable", plugins);
    expect(payload).not.toBeNull();
    const labels = payload?.inline_keyboard.map((row) => row[0]?.text);
    expect(labels).toEqual(["chrome", "hugging-face", "← back"]);
    expect(payload?.inline_keyboard[0]?.[0]?.callback_data).toBe(
      "tgcmd:/codex plugins disable chrome",
    );
    expect(payload?.inline_keyboard[2]?.[0]?.callback_data).toBe("cdx_plugins_menu");
  });

  it("filters to disabled-only for enable", () => {
    const payload = buildCodexPluginNamePickerPayload("enable", plugins);
    expect(payload).not.toBeNull();
    const labels = payload?.inline_keyboard.map((row) => row[0]?.text);
    expect(labels).toEqual(["build-ios-apps (OFF)", "← back"]);
  });

  it("returns null when there are no eligible plugins for the verb", () => {
    expect(buildCodexPluginNamePickerPayload("disable", {})).toBeNull();
    expect(
      buildCodexPluginNamePickerPayload("enable", {
        x: { enabled: true, marketplaceName: "m", pluginName: "p" },
      }),
    ).toBeNull();
  });
});

describe("buildCodexPluginNamePickerEmptyText", () => {
  it("returns verb-appropriate empty messages", () => {
    expect(buildCodexPluginNamePickerEmptyText("disable")).toContain("No enabled");
    expect(buildCodexPluginNamePickerEmptyText("enable")).toContain("No disabled");
  });
});
