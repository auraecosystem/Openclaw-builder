import type { AgentToolResult } from "@earendil-works/pi-agent-core";
import {
  createActionGate,
  jsonResult,
  readReactionParams,
  readStringParam,
} from "openclaw/plugin-sdk/channel-actions";
import type { OpenClawConfig } from "openclaw/plugin-sdk/config-contracts";
import { resolveAuthorizedWhatsAppOutboundTarget } from "./action-runtime-target-auth.js";
import { resolveWhatsAppReactionLevel } from "./reaction-level.js";
import { sendListReplyWhatsApp, sendReactionWhatsApp } from "./send.js";
import { toWhatsappJid } from "./text-runtime.js";

export const whatsAppActionRuntime = {
  resolveAuthorizedWhatsAppOutboundTarget,
  sendListReplyWhatsApp,
  sendReactionWhatsApp,
};

export async function handleWhatsAppAction(
  params: Record<string, unknown>,
  cfg: OpenClawConfig,
): Promise<AgentToolResult<unknown>> {
  const action = readStringParam(params, "action", { required: true });
  const whatsAppConfig = cfg.channels?.whatsapp;
  const isActionEnabled = createActionGate(whatsAppConfig?.actions);

  if (action === "react") {
    const accountId = readStringParam(params, "accountId");
    if (!whatsAppConfig) {
      throw new Error("WhatsApp reactions are disabled.");
    }
    if (!isActionEnabled("reactions")) {
      throw new Error("WhatsApp reactions are disabled.");
    }
    const reactionLevelInfo = resolveWhatsAppReactionLevel({
      cfg,
      accountId: accountId ?? undefined,
    });
    if (!reactionLevelInfo.agentReactionsEnabled) {
      throw new Error(
        `WhatsApp agent reactions disabled (reactionLevel="${reactionLevelInfo.level}"). ` +
          `Set channels.whatsapp.reactionLevel to "minimal" or "extensive" to enable.`,
      );
    }
    const chatJid = readStringParam(params, "chatJid", { required: true });
    const messageId = readStringParam(params, "messageId", { required: true });
    const { emoji, remove, isEmpty } = readReactionParams(params, {
      removeErrorMessage: "Emoji is required to remove a WhatsApp reaction.",
    });
    const participant = readStringParam(params, "participant");
    const fromMeRaw = params.fromMe;
    const fromMe = typeof fromMeRaw === "boolean" ? fromMeRaw : undefined;

    // Resolve account + allowFrom via shared account logic so auth and routing stay aligned.
    const resolved = whatsAppActionRuntime.resolveAuthorizedWhatsAppOutboundTarget({
      cfg,
      chatJid,
      accountId,
      actionLabel: "reaction",
    });

    const resolvedEmoji = remove ? "" : emoji;
    await whatsAppActionRuntime.sendReactionWhatsApp(resolved.to, messageId, resolvedEmoji, {
      verbose: false,
      fromMe,
      participant: participant ?? undefined,
      accountId: resolved.accountId,
      cfg,
    });
    if (!remove && !isEmpty) {
      return jsonResult({ ok: true, added: emoji });
    }
    return jsonResult({ ok: true, removed: true });
  }

  if (action === "list-reply" || action === "listReply") {
    const accountId = readStringParam(params, "accountId");
    if (!whatsAppConfig) {
      throw new Error("WhatsApp list replies are disabled.");
    }
    if (!isActionEnabled("sendMessage")) {
      throw new Error("WhatsApp list replies are disabled.");
    }
    const to =
      readStringParam(params, "to") ??
      readStringParam(params, "chatJid") ??
      readStringParam(params, "chatId");
    if (!to) {
      throw new Error("WhatsApp list reply requires to or chatJid.");
    }
    const selectedRowId =
      readStringParam(params, "selectedRowId") ?? readStringParam(params, "rowId");
    if (!selectedRowId) {
      throw new Error("WhatsApp list reply requires selectedRowId or rowId.");
    }
    const title = readStringParam(params, "title") ?? selectedRowId;
    const description = readStringParam(params, "description");
    const messageId =
      readStringParam(params, "messageId") ?? readStringParam(params, "replyToId");
    const participant = readStringParam(params, "participant");
    const fromMeRaw = params.fromMe;
    const fromMe = typeof fromMeRaw === "boolean" ? fromMeRaw : false;

    const resolved = whatsAppActionRuntime.resolveAuthorizedWhatsAppOutboundTarget({
      cfg,
      chatJid: to,
      accountId,
      actionLabel: "list reply",
    });

    await whatsAppActionRuntime.sendListReplyWhatsApp(
      resolved.to,
      {
        title,
        selectedRowId,
        ...(description ? { description } : {}),
      },
      {
        verbose: false,
        accountId: resolved.accountId,
        cfg,
        ...(messageId
          ? {
              quotedMessageKey: {
                id: messageId,
                remoteJid: toWhatsappJid(resolved.to),
                fromMe,
                ...(participant ? { participant } : {}),
              },
            }
          : {}),
      },
    );
    return jsonResult({ ok: true, selectedRowId });
  }

  throw new Error(`Unsupported WhatsApp action: ${action}`);
}
