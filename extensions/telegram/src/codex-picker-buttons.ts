/**
 * Telegram in-place navigation for the /codex picker tree.
 *
 * Mirrors the `model-buttons.ts` pattern: extensions/telegram owns its own
 * builders for the picker UI it renders, and the bot's callback_query handler
 * dispatches these via `editMessageText` so the picker morphs in place rather
 * than spawning a new bot reply per tap.
 *
 * The codex plugin's own picker builders (extensions/codex/src/command-handlers.ts)
 * stay the source of truth for slash-command output and for the channel-agnostic
 * `MessagePresentation` blocks. The structure mirrored here is intentionally
 * narrow: only the navigation surface (top picker, sub-menus, back buttons,
 * dynamic plugin name pickers) needs in-place rendering. Leaf actions (e.g.
 * tapping a specific plugin to enable, or tapping `yolo` under permissions)
 * keep the `tgcmd:` callback_data so they fire as real chat commands and
 * produce a confirmation reply.
 *
 * Callback_data scheme (Telegram caps each callback_data at 64 bytes):
 * - `cdx_top`                    top-level picker
 * - `cdx_plugins_menu`           plugins sub-menu (list/enable/disable/.../computer-use)
 * - `cdx_perm_menu`              permissions sub-menu (default/yolo/status)
 * - `cdx_more_menu`              aggregated more sub-menu (models/account/threads/...)
 * - `cdx_fast_menu`              fast-mode sub-menu (typeable; not linked from top)
 * - `cdx_cuse_menu`              computer-use sub-menu (status/install)
 * - `cdx_picker_enable`          dynamic picker of currently-disabled plugins
 * - `cdx_picker_disable`         dynamic picker of currently-enabled plugins
 */

export type ButtonRow = Array<{ text: string; callback_data: string }>;

export type CodexPluginEntry = {
  enabled?: boolean;
  marketplaceName?: string;
  pluginName?: string;
};

export type ParsedCodexCallback =
  | { type: "top" }
  | { type: "plugins_menu" }
  | { type: "perm_menu" }
  | { type: "fast_menu" }
  | { type: "cuse_menu" }
  | { type: "picker"; verb: "enable" | "disable" };

export type CodexCallbackPayload = {
  text: string;
  inline_keyboard: ButtonRow[];
};

const CALLBACK_PREFIX = {
  top: "cdx_top",
  pluginsMenu: "cdx_plugins_menu",
  permMenu: "cdx_perm_menu",
  fastMenu: "cdx_fast_menu",
  cuseMenu: "cdx_cuse_menu",
  pickerPrefix: "cdx_picker_",
} as const;

/** Parse a Telegram callback_data string into a structured codex navigation event. */
export function parseCodexCallbackData(data: string): ParsedCodexCallback | null {
  const trimmed = data.trim();
  switch (trimmed) {
    case CALLBACK_PREFIX.top:
      return { type: "top" };
    case CALLBACK_PREFIX.pluginsMenu:
      return { type: "plugins_menu" };
    case CALLBACK_PREFIX.permMenu:
      return { type: "perm_menu" };
    case CALLBACK_PREFIX.fastMenu:
      return { type: "fast_menu" };
    case CALLBACK_PREFIX.cuseMenu:
      return { type: "cuse_menu" };
  }
  if (trimmed.startsWith(CALLBACK_PREFIX.pickerPrefix)) {
    const verb = trimmed.slice(CALLBACK_PREFIX.pickerPrefix.length);
    if (verb === "enable" || verb === "disable") {
      return { type: "picker", verb };
    }
  }
  return null;
}

/**
 * Top-level /codex picker. Four buttons in a 2x2 grid:
 *   Row 1: [plugins]   [permissions]
 *   Row 2: [account]   [help]
 *
 * - `plugins`, `permissions`: open sub-menus via in-place edit (cdx_*).
 * - `account`: leaf, fires `/codex account` (auth info as new message).
 * - `help`: leaf, fires `/codex help` (codex-scoped help listing every verb).
 *
 * `/codex help` is NOT a dupe of openclaw's `/help` — the latter lists openclaw
 * top-level slash commands; the former lists every codex verb (the typeable
 * surface we deliberately keep out of the picker).
 *
 * Picker omissions (typeable only):
 *   - dupes of openclaw top-level commands: /status, /fast, /stop, /models
 *   - debug / power-user verbs: threads, mcp, binding, detach, skills
 *   - free-form-arg or heavier verbs: resume, bind, steer, model, diagnostics,
 *     compact, review, computer-use status/install
 */
export function buildCodexTopPickerPayload(): CodexCallbackPayload {
  return {
    text:
      "Codex commands. Pick a category or type:\n\n" +
      "1. /codex plugins menu\n" +
      "2. /codex permissions menu\n" +
      "3. /codex account\n" +
      "4. /codex help\n\n" +
      "Tap 'help' for the full list of typeable verbs (threads, mcp, binding, " +
      "detach, skills, resume, bind, steer, model, diagnostics, compact, review, " +
      "computer-use).\n\n" +
      "Top-level shortcuts cover everyday operations: /status, /fast, /help, /stop, /models.",
    inline_keyboard: [
      [
        { text: "plugins", callback_data: CALLBACK_PREFIX.pluginsMenu },
        { text: "permissions", callback_data: CALLBACK_PREFIX.permMenu },
      ],
      [
        { text: "account", callback_data: "tgcmd:/codex account" },
        { text: "help", callback_data: "tgcmd:/codex help" },
      ],
    ],
  };
}

/**
 * Plugins sub-menu. Includes computer-use as a built-in capability so the
 * user's "things I install on my agent" mental model maps cleanly.
 */
export function buildCodexPluginsMenuPayload(): CodexCallbackPayload {
  // `computer-use` is not a dedicated entry; it auto-populates and appears
  // in the dynamic enable/disable pickers like any other plugin.
  return {
    text: "Codex plugins. Pick a sub-action or type the command.",
    inline_keyboard: [
      [{ text: "list", callback_data: "tgcmd:/codex plugins list" }],
      [{ text: "enable", callback_data: `${CALLBACK_PREFIX.pickerPrefix}enable` }],
      [{ text: "disable", callback_data: `${CALLBACK_PREFIX.pickerPrefix}disable` }],
      [{ text: "help", callback_data: "tgcmd:/codex plugins help" }],
      [{ text: "← back", callback_data: CALLBACK_PREFIX.top }],
    ],
  };
}

/** Permissions sub-menu: default / yolo / status, plus back. */
export function buildCodexPermissionsMenuPayload(): CodexCallbackPayload {
  return {
    text: "Codex permissions. Pick a mode or type /codex permissions <mode>:",
    inline_keyboard: [
      [{ text: "default", callback_data: "tgcmd:/codex permissions default" }],
      [{ text: "yolo", callback_data: "tgcmd:/codex permissions yolo" }],
      [{ text: "status", callback_data: "tgcmd:/codex permissions status" }],
      [{ text: "← back", callback_data: CALLBACK_PREFIX.top }],
    ],
  };
}

/** Fast-mode sub-menu: on / off / status, plus back. */
export function buildCodexFastMenuPayload(): CodexCallbackPayload {
  return {
    text: "Codex fast mode. Pick a mode or type /codex fast <mode>:",
    inline_keyboard: [
      [{ text: "on", callback_data: "tgcmd:/codex fast on" }],
      [{ text: "off", callback_data: "tgcmd:/codex fast off" }],
      [{ text: "status", callback_data: "tgcmd:/codex fast status" }],
      [{ text: "← back", callback_data: CALLBACK_PREFIX.top }],
    ],
  };
}

/**
 * Computer-use sub-menu. Back goes to plugins menu (not top) because
 * computer-use is presented under plugins in the picker tree.
 */
export function buildCodexComputerUseMenuPayload(): CodexCallbackPayload {
  return {
    text:
      "Codex computer-use. Pick an action or type /codex computer-use <action>:\n\n" +
      "Flag-driven invocations (--source, --marketplace-path, --marketplace) are not in the " +
      "picker. Type '/codex computer-use' for the full surface.",
    inline_keyboard: [
      [{ text: "status", callback_data: "tgcmd:/codex computer-use status" }],
      [{ text: "install", callback_data: "tgcmd:/codex computer-use install" }],
      [{ text: "← back", callback_data: CALLBACK_PREFIX.pluginsMenu }],
    ],
  };
}

/**
 * Plugin name picker for enable/disable. The plugin list comes from the live
 * OpenClaw config (the same source that /codex plugins list reads). Buttons
 * fire as tgcmd: so the actual verb runs and replies with a confirmation; the
 * back button uses cdx_plugins_menu so navigation stays in-place.
 *
 * Returns null if there are no eligible plugins for the verb (caller should
 * fall back to a plain-text empty-state message via tgcmd).
 */
export function buildCodexPluginNamePickerPayload(
  verb: "enable" | "disable",
  plugins: Record<string, CodexPluginEntry>,
): CodexCallbackPayload | null {
  const keys = Object.keys(plugins).toSorted();
  const filtered = keys.filter((key) => {
    const enabled = plugins[key]?.enabled !== false;
    if (verb === "disable") {
      return enabled;
    }
    return !enabled;
  });

  if (filtered.length === 0) {
    return null;
  }

  const rows: ButtonRow[] = filtered.map((key) => {
    const entry = plugins[key] ?? {};
    const state = entry.enabled === false ? " (OFF)" : "";
    return [
      {
        text: `${key}${state}`,
        callback_data: `tgcmd:/codex plugins ${verb} ${key}`,
      },
    ];
  });
  rows.push([{ text: "← back", callback_data: CALLBACK_PREFIX.pluginsMenu }]);

  const verbLabel = verb;
  const lines = [
    `Pick a Codex sub-plugin to ${verbLabel}:`,
    "",
    ...filtered.map((key, i) => {
      const entry = plugins[key] ?? {};
      const state = entry.enabled === false ? "OFF" : "ON";
      const marketplace = entry.marketplaceName ?? "?";
      return `  ${i + 1}. ${key}  (${state}, ${marketplace})`;
    }),
  ];

  return {
    text: lines.join("\n"),
    inline_keyboard: rows,
  };
}

/** Empty-state text for the plugin name picker when no plugins are eligible. */
export function buildCodexPluginNamePickerEmptyText(verb: "enable" | "disable"): string {
  if (verb === "disable") {
    return "No enabled Codex sub-plugins to disable. Run '/codex plugins list' to see current state.";
  }
  return "No disabled Codex sub-plugins to enable. Run '/codex plugins list' to see current state.";
}
