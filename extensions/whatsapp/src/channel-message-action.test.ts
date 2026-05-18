import { beforeEach, describe, expect, it, vi } from "vitest";
import { handleWhatsAppMessageAction } from "./channel-message-action.js";
import type { OpenClawConfig } from "./runtime-api.js";

const hoisted = vi.hoisted(() => ({
  handleWhatsAppAction: vi.fn(async () => ({ content: [{ type: "text", text: '{"ok":true}' }] })),
  handleWhatsAppReactAction: vi.fn(async () => ({
    content: [{ type: "text", text: '{"ok":true}' }],
  })),
}));

vi.mock("./channel-message-action.runtime.js", async () => {
  return {
    handleWhatsAppAction: hoisted.handleWhatsAppAction,
    normalizeWhatsAppTarget: (value?: string | null) => {
      const raw = (value ?? "").trim();
      if (!raw) {
        return null;
      }
      const stripped = raw.replace(/^whatsapp:/, "");
      if (stripped.endsWith("@g.us")) {
        return stripped;
      }
      return stripped.startsWith("+") ? stripped : `+${stripped.replace(/^\+/, "")}`;
    },
    readStringParam: (params: Record<string, unknown>, key: string) => {
      const value = params[key];
      return typeof value === "string" && value.trim() ? value : undefined;
    },
  };
});

vi.mock("./channel-react-action.js", async () => {
  return {
    handleWhatsAppReactAction: hoisted.handleWhatsAppReactAction,
  };
});

describe("handleWhatsAppMessageAction", () => {
  const baseCfg = {
    channels: { whatsapp: { actions: { sendMessage: true }, allowFrom: ["*"] } },
  } as OpenClawConfig;

  beforeEach(() => {
    hoisted.handleWhatsAppAction.mockClear();
    hoisted.handleWhatsAppReactAction.mockClear();
  });

  it("delegates reactions to the existing reaction handler", async () => {
    await handleWhatsAppMessageAction({
      action: "react",
      params: { to: "+1555", messageId: "msg-1", emoji: "👍" },
      cfg: baseCfg,
      accountId: "default",
    });

    expect(hoisted.handleWhatsAppReactAction).toHaveBeenCalledWith({
      action: "react",
      params: { to: "+1555", messageId: "msg-1", emoji: "👍" },
      cfg: baseCfg,
      accountId: "default",
    });
    expect(hoisted.handleWhatsAppAction).not.toHaveBeenCalled();
  });

  it("routes list replies through the WhatsApp action runtime", async () => {
    await handleWhatsAppMessageAction({
      action: "list-reply",
      params: {
        to: "+1555",
        selectedRowId: "2ª via",
        title: "2ª via",
      },
      cfg: baseCfg,
      accountId: "default",
    });

    expect(hoisted.handleWhatsAppAction).toHaveBeenCalledWith(
      {
        action: "list-reply",
        to: "+1555",
        selectedRowId: "2ª via",
        title: "2ª via",
        accountId: "default",
      },
      baseCfg,
    );
  });

  it("quotes the current inbound list message when replying in the same chat", async () => {
    await handleWhatsAppMessageAction({
      action: "list-reply",
      params: {
        to: "+1555",
        rowId: "2ª via",
        title: "2ª via",
      },
      cfg: baseCfg,
      accountId: "default",
      toolContext: {
        currentChannelProvider: "whatsapp",
        currentChannelId: "whatsapp:+1555",
        currentMessageId: "list-msg-1",
      },
    });

    expect(hoisted.handleWhatsAppAction).toHaveBeenCalledWith(
      {
        action: "list-reply",
        to: "+1555",
        rowId: "2ª via",
        title: "2ª via",
        accountId: "default",
        messageId: "list-msg-1",
      },
      baseCfg,
    );
  });

  it("does not quote a different current chat", async () => {
    await handleWhatsAppMessageAction({
      action: "list-reply",
      params: {
        to: "+9999",
        rowId: "2ª via",
        title: "2ª via",
      },
      cfg: baseCfg,
      accountId: "default",
      toolContext: {
        currentChannelProvider: "whatsapp",
        currentChannelId: "whatsapp:+1555",
        currentMessageId: "list-msg-1",
      },
    });

    expect(hoisted.handleWhatsAppAction).toHaveBeenCalledWith(
      {
        action: "list-reply",
        to: "+9999",
        rowId: "2ª via",
        title: "2ª via",
        accountId: "default",
      },
      baseCfg,
    );
  });
});
