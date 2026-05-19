import type { Bot } from "grammy";
import type { Message } from "grammy/types";
import type {
  DmPolicy,
  OpenClawConfig,
  ReplyToMode,
  TelegramAccountConfig,
} from "openclaw/plugin-sdk/config-contracts";
import { buildAgentSessionKey } from "openclaw/plugin-sdk/routing";
import { createSubsystemLogger, danger, logVerbose } from "openclaw/plugin-sdk/runtime-env";
import type { RuntimeEnv } from "openclaw/plugin-sdk/runtime-env";
import { normalizeLowercaseStringOrEmpty } from "openclaw/plugin-sdk/string-coerce-runtime";
import { expandTelegramAllowFromWithAccessGroups } from "./access-groups.js";
import { resolveTelegramAccount, type ResolvedTelegramAccount } from "./accounts.js";
import { shouldRequestTelegramGuestUpdates } from "./allowed-updates.js";
import { isSenderAllowed, normalizeAllowFrom } from "./bot-access.js";
import type { TelegramBotDeps } from "./bot-deps.js";
import {
  buildTelegramMessageContext,
  type BuildTelegramMessageContextParams,
} from "./bot-message-context.js";
import { dispatchTelegramMessage } from "./bot-message-dispatch.js";
import { resolveDefaultAgentId } from "./bot.agent.runtime.js";
import type { TelegramBotOptions } from "./bot.types.js";
import type { TelegramContext } from "./bot/types.js";

const DEFAULT_GUEST_FALLBACK_TEXT = "I could not produce a visible answer. Please try again.";
const TELEGRAM_GUEST_MESSAGE_TEXT_LIMIT = 4096;
const GUEST_PROMPT_PREFIX =
  "Telegram Guest Mode query. Return exactly one visible final answer. " +
  "Do not use message(action=send), Telegram send tools, or source-channel delivery tools. " +
  "Do not intentionally stay silent; if you cannot complete the request, explain briefly.";
const GUEST_RESULT_TITLE = "OpenClaw";

type TelegramGuestMessage = Message & {
  guest_query_id?: string;
};

type TelegramGuestContext = TelegramContext & {
  update?: {
    guest_message?: TelegramGuestMessage;
  };
};

type TelegramAnswerGuestQueryPayload = {
  guest_query_id: string;
  result: {
    type: "article";
    id: string;
    title: string;
    input_message_content: {
      message_text: string;
    };
    description?: string;
  };
};

type TelegramGuestApi = {
  raw?: {
    answerGuestQuery?: (payload: TelegramAnswerGuestQueryPayload) => Promise<unknown>;
  };
};

type RegisterTelegramGuestHandlersParams = {
  cfg: OpenClawConfig;
  bot: Bot;
  account: ResolvedTelegramAccount;
  telegramCfg: TelegramAccountConfig;
  historyLimit: number;
  groupHistories: BuildTelegramMessageContextParams["groupHistories"];
  dmPolicy: DmPolicy;
  allowFrom?: Array<string | number>;
  allowFromOverride?: Array<string | number>;
  logger: BuildTelegramMessageContextParams["logger"];
  resolveGroupActivation: BuildTelegramMessageContextParams["resolveGroupActivation"];
  loadFreshConfig: () => OpenClawConfig;
  runtime: RuntimeEnv;
  replyToMode: ReplyToMode;
  textLimit: number;
  opts: Pick<TelegramBotOptions, "token">;
  telegramDeps: TelegramBotDeps;
};

function isGuestModeEnabled(params: { telegramCfg: TelegramAccountConfig }): boolean {
  return shouldRequestTelegramGuestUpdates({
    guest: params.telegramCfg.guest,
  });
}

function resolveFreshTelegramConfig(params: {
  cfg: OpenClawConfig;
  account: ResolvedTelegramAccount;
  fallback: TelegramAccountConfig;
}): TelegramAccountConfig {
  try {
    return resolveTelegramAccount({
      cfg: params.cfg,
      accountId: params.account.accountId,
    }).config;
  } catch (error) {
    logVerbose(
      `telegram guest: failed to load fresh config for account ${params.account.accountId}; using startup snapshot: ${String(error)}`,
    );
    return params.fallback;
  }
}

function buildGuestSessionKey(params: {
  agentId: string;
  accountId: string;
  chatId: string | number;
  senderId?: string;
}): string {
  const senderPart = params.senderId ? `:sender:${params.senderId}` : "";
  return normalizeLowercaseStringOrEmpty(
    buildAgentSessionKey({
      agentId: params.agentId,
      channel: "telegram",
      accountId: params.accountId,
      dmScope: "per-account-channel-peer",
      peer: {
        kind: "direct",
        id: `guest-from-group:${params.chatId}${senderPart}`,
      },
    }),
  );
}

function buildGuestAnswerPayload(
  guestQueryId: string,
  text: string,
): TelegramAnswerGuestQueryPayload {
  const trimmed = text.trim();
  const messageText = trimmed.slice(0, TELEGRAM_GUEST_MESSAGE_TEXT_LIMIT);
  return {
    guest_query_id: guestQueryId,
    result: {
      type: "article",
      id: `openclaw-${Date.now().toString(36)}`,
      title: GUEST_RESULT_TITLE,
      input_message_content: {
        message_text: messageText,
      },
      ...(messageText ? { description: messageText.slice(0, 120) } : {}),
    },
  };
}

async function answerGuestQuery(params: {
  bot: Bot;
  guestQueryId: string;
  text: string;
}): Promise<boolean> {
  const api = params.bot.api as unknown as TelegramGuestApi;
  const answer = api.raw?.answerGuestQuery;
  if (typeof answer !== "function") {
    throw new Error("Telegram API client does not expose raw.answerGuestQuery");
  }
  await answer(buildGuestAnswerPayload(params.guestQueryId, params.text));
  return true;
}

export function registerTelegramGuestHandlers(params: RegisterTelegramGuestHandlersParams): void {
  const guestLog = createSubsystemLogger("gateway/channels/telegram/guest");
  const sessionRuntime = {
    ...(params.telegramDeps.buildChannelInboundEventContext
      ? { buildChannelInboundEventContext: params.telegramDeps.buildChannelInboundEventContext }
      : {}),
    ...(params.telegramDeps.readSessionUpdatedAt
      ? { readSessionUpdatedAt: params.telegramDeps.readSessionUpdatedAt }
      : {}),
    ...(params.telegramDeps.recordInboundSession
      ? { recordInboundSession: params.telegramDeps.recordInboundSession }
      : {}),
    ...(params.telegramDeps.resolveInboundLastRouteSessionKey
      ? { resolveInboundLastRouteSessionKey: params.telegramDeps.resolveInboundLastRouteSessionKey }
      : {}),
    ...(params.telegramDeps.resolvePinnedMainDmOwnerFromAllowlist
      ? {
          resolvePinnedMainDmOwnerFromAllowlist:
            params.telegramDeps.resolvePinnedMainDmOwnerFromAllowlist,
        }
      : {}),
    resolveStorePath: params.telegramDeps.resolveStorePath,
  };
  const contextRuntime = params.telegramDeps.recordChannelActivity
    ? { recordChannelActivity: params.telegramDeps.recordChannelActivity }
    : undefined;

  params.bot.use(async (ctx, next) => {
    const guestCtx = ctx as unknown as TelegramGuestContext;
    const msg = guestCtx.update?.guest_message;
    if (!msg) {
      await next();
      return;
    }
    const freshCfg = params.loadFreshConfig();
    const freshTelegramCfg = resolveFreshTelegramConfig({
      cfg: freshCfg,
      account: params.account,
      fallback: params.telegramCfg,
    });
    if (!isGuestModeEnabled({ telegramCfg: freshTelegramCfg })) {
      logVerbose("telegram guest: skipped guest_message because guest mode is disabled");
      return;
    }
    const guestQueryId = msg.guest_query_id?.trim();
    if (!guestQueryId) {
      guestLog.debug("telegram guest: skipped guest_message without guest_query_id");
      return;
    }

    const guestDmPolicy = freshTelegramCfg.dmPolicy ?? params.dmPolicy;
    if (guestDmPolicy === "disabled") {
      logVerbose("telegram guest: blocked guest_message because dmPolicy is disabled");
      return;
    }

    const fallbackText = freshTelegramCfg.guest?.fallbackText ?? DEFAULT_GUEST_FALLBACK_TEXT;
    const guestAllowFrom = params.allowFromOverride ?? freshTelegramCfg.allowFrom;
    let answered = false;
    const answerText = async (text: string) => {
      if (answered) {
        return true;
      }
      answered = await answerGuestQuery({
        bot: params.bot,
        guestQueryId,
        text,
      });
      return answered;
    };

    try {
      const chatId = msg.chat.id;
      const senderId = msg.from?.id != null ? String(msg.from.id) : "";
      const senderUsername = msg.from?.username ?? "";
      const effectiveGuestAllow = normalizeAllowFrom(
        await expandTelegramAllowFromWithAccessGroups({
          cfg: freshCfg,
          allowFrom: guestAllowFrom,
          accountId: params.account.accountId,
          senderId,
        }),
      );
      if (
        !effectiveGuestAllow.hasEntries ||
        !isSenderAllowed({
          allow: effectiveGuestAllow,
          senderId,
          senderUsername,
        })
      ) {
        logVerbose(
          `telegram guest: blocked guest_message from ${senderId || "unknown"} (allowFrom)`,
        );
        return;
      }
      const routeAgentId = resolveDefaultAgentId(freshCfg);
      const guestConversationId = `guest-from-group:${chatId}:sender:${senderId || "unknown"}`;
      const primaryCtx: TelegramContext = {
        message: msg,
        me: guestCtx.me,
        getFile:
          typeof guestCtx.getFile === "function"
            ? guestCtx.getFile.bind(guestCtx)
            : async () => ({}),
      };
      const context = await buildTelegramMessageContext({
        primaryCtx,
        allMedia: [],
        storeAllowFrom: [],
        options: {
          forceWasMentioned: true,
          sessionKeyOverride: buildGuestSessionKey({
            agentId: routeAgentId,
            accountId: params.account.accountId,
            chatId,
            senderId,
          }),
          routeAgentIdOverride: routeAgentId,
          messageIdOverride: `guest:${guestQueryId}`,
          systemPromptPrefix: GUEST_PROMPT_PREFIX,
          skipGroupBaseAccess: true,
          conversationKindOverride: "direct",
          fromOverride: `telegram:${guestConversationId}`,
          conversationIdOverride: guestConversationId,
          conversationLabelOverride: "Telegram Guest Mode",
          originatingToOverride: `telegram:guest:${guestQueryId}`,
          suppressUpdateLastRoute: true,
        },
        bot: params.bot,
        cfg: params.cfg,
        account: params.account,
        historyLimit: params.historyLimit,
        groupHistories: params.groupHistories,
        dmPolicy: guestDmPolicy,
        allowFrom: guestAllowFrom,
        groupAllowFrom: guestAllowFrom,
        ackReactionScope: "off",
        logger: params.logger,
        resolveGroupActivation: params.resolveGroupActivation,
        resolveGroupRequireMention: () => false,
        resolveTelegramGroupConfig: () => ({}),
        sendChatActionHandler: {
          sendChatAction: async () => undefined,
          isSuspended: () => false,
          reset: () => undefined,
        },
        loadFreshConfig: params.loadFreshConfig,
        runtime: contextRuntime,
        sessionRuntime,
        upsertPairingRequest: params.telegramDeps.upsertChannelPairingRequest,
      });
      if (!context) {
        await answerText(fallbackText);
        return;
      }
      await dispatchTelegramMessage({
        context: {
          ...context,
          sendTyping: async () => undefined,
          sendRecordVoice: async () => undefined,
          ackReactionPromise: null,
          reactionApi: null,
          removeAckAfterReply: false,
          statusReactionController: null,
        },
        bot: params.bot,
        cfg: params.cfg,
        runtime: params.runtime,
        replyToMode: params.replyToMode,
        streamMode: "off",
        textLimit: params.textLimit,
        telegramCfg: params.telegramCfg,
        telegramDeps: params.telegramDeps,
        opts: params.opts,
        sourceReplyDeliveryMode: "automatic",
        guestDelivery: { answerText },
      });
      if (!answered) {
        await answerText(fallbackText);
      }
    } catch (err) {
      params.runtime.error?.(danger(`telegram guest message processing failed: ${String(err)}`));
      if (!answered) {
        try {
          await answerText(fallbackText);
        } catch {}
      }
    }
  });
}
