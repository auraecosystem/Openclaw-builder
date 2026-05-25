import {
  buildBrokerConversationTarget,
  normalizeBrokerPlatformId,
  parseBrokerConversationTarget,
  type BrokerConversationTarget,
  type BrokerConversationType,
} from "openclaw/plugin-sdk/channel-broker";
import { normalizeKnownChannelBrokerPlatformId } from "./platforms.js";
import type { ResolvedChannelBrokerAccount } from "./types.js";

const CONVERSATION_TYPE_PREFIXES = new Set(["direct", "group", "channel", "thread"]);
const DIRECT_CONVERSATION_TYPE_ALIASES = new Set(["direct", "dm", "user"]);

function normalizeConversationType(raw: string | undefined): BrokerConversationType | undefined {
  if (!raw) {
    return undefined;
  }
  const normalized = raw.trim().toLowerCase();
  return CONVERSATION_TYPE_PREFIXES.has(normalized)
    ? (normalized as BrokerConversationType)
    : undefined;
}

export function normalizeBrokerTarget(raw: string): string | undefined {
  const trimmed = raw.trim();
  if (!trimmed) {
    return undefined;
  }
  try {
    const parsed = parseBrokerConversationTarget(trimmed);
    return buildBrokerConversationTarget(parsed);
  } catch {
    return undefined;
  }
}

function chatTypeFromConversationType(
  conversationType: BrokerConversationType | undefined,
): "direct" | "group" | "channel" | undefined {
  if (conversationType === "direct" || conversationType === "group") {
    return conversationType;
  }
  if (conversationType === "channel" || conversationType === "thread") {
    return "channel";
  }
  return undefined;
}

function inferTelegramChatTypeFromConversation(params: {
  conversationId: string;
  threadId?: string;
}): "direct" | "group" | "channel" | undefined {
  if (params.threadId) {
    return "channel";
  }
  if (/^\d+$/.test(params.conversationId)) {
    return "direct";
  }
  if (/^-\d+$/.test(params.conversationId)) {
    return "group";
  }
  return undefined;
}

export function inferChannelBrokerTargetChatType(
  rawTarget: string,
): "direct" | "group" | "channel" | undefined {
  try {
    const parsed = parseBrokerConversationTarget(rawTarget);
    const parsedType = chatTypeFromConversationType(parsed.conversationType);
    if (parsedType) {
      return parsedType;
    }
    const brokerPrefixed = parsed.platform === "broker" || parsed.platform === "channel-broker";
    const rawConversationId =
      brokerPrefixed && parsed.conversationId.includes(":")
        ? parsed.conversationId.slice(parsed.conversationId.indexOf(":") + 1)
        : parsed.conversationId;
    const [rawType] = rawConversationId.split(":", 1);
    const type = rawType?.trim().toLowerCase();
    if (DIRECT_CONVERSATION_TYPE_ALIASES.has(type ?? "")) {
      return "direct";
    }
    if (type === "group") {
      return "group";
    }
    if (type === "channel" || type === "thread") {
      return "channel";
    }
    const rawPlatform =
      brokerPrefixed && parsed.conversationId.includes(":")
        ? parsed.conversationId.slice(0, parsed.conversationId.indexOf(":"))
        : parsed.platform;
    if (normalizeBrokerPlatformId(rawPlatform) === "telegram") {
      const telegramConversation = parseTelegramTopicConversation(rawConversationId);
      const telegramType = inferTelegramChatTypeFromConversation(telegramConversation);
      if (telegramType) {
        return telegramType;
      }
    }
    return "channel";
  } catch {
    return undefined;
  }
}

function parseTelegramTopicConversation(rawConversationId: string): {
  conversationId: string;
  threadId?: string;
  conversationType?: BrokerConversationType;
} {
  const topicMatch = /^(.*):topic:([^:]+)$/i.exec(rawConversationId);
  if (topicMatch?.[1] && topicMatch[2]) {
    return {
      conversationId: topicMatch[1],
      threadId: topicMatch[2],
    };
  }
  const numericTopicMatch = /^(-?\d+):(\d+)$/.exec(rawConversationId);
  if (numericTopicMatch?.[1] && numericTopicMatch[2]) {
    return {
      conversationId: numericTopicMatch[1],
      threadId: numericTopicMatch[2],
    };
  }
  return { conversationId: rawConversationId };
}

function parseDiscordConversation(rawConversationId: string): {
  conversationId: string;
  threadId?: string;
  conversationType?: BrokerConversationType;
} {
  const kindMatch = /^(user|dm|direct|channel|thread):(.+)$/i.exec(rawConversationId);
  const rawKind = kindMatch?.[1]?.toLowerCase();
  const id = kindMatch?.[2]?.trim();
  if (!rawKind || !id) {
    return { conversationId: rawConversationId };
  }
  const conversationType: BrokerConversationType =
    rawKind === "user" || rawKind === "dm"
      ? "direct"
      : rawKind === "thread"
        ? "thread"
        : rawKind === "direct"
          ? "direct"
          : "channel";
  return {
    conversationId: id,
    conversationType,
  };
}

function parseSlackConversation(rawConversationId: string): {
  conversationId: string;
  threadId?: string;
  conversationType?: BrokerConversationType;
} {
  const kindMatch = /^(user|dm|direct|channel):(.+)$/i.exec(rawConversationId);
  const rawKind = kindMatch?.[1]?.toLowerCase();
  const id = kindMatch?.[2]?.trim();
  if (!rawKind || !id) {
    return { conversationId: rawConversationId };
  }
  return {
    conversationId: id,
    conversationType: rawKind === "channel" ? "channel" : "direct",
  };
}

function parsePlatformConversation(params: { platform: string; rawConversationId: string }): {
  conversationId: string;
  threadId?: string;
  conversationType?: BrokerConversationType;
} {
  if (params.platform === "telegram") {
    return parseTelegramTopicConversation(params.rawConversationId);
  }
  if (params.platform === "discord") {
    return parseDiscordConversation(params.rawConversationId);
  }
  if (params.platform === "slack") {
    return parseSlackConversation(params.rawConversationId);
  }
  return { conversationId: params.rawConversationId };
}

export function parseChannelBrokerTarget(params: {
  rawTarget: string;
  account: ResolvedChannelBrokerAccount;
  threadId?: string | number | null;
}): BrokerConversationTarget {
  const parsed = parseBrokerConversationTarget(params.rawTarget);
  const brokerPrefixed = parsed.platform === "broker" || parsed.platform === "channel-broker";
  const brokerPrefixSeparator = brokerPrefixed ? parsed.conversationId.indexOf(":") : -1;
  const rawPlatform =
    brokerPrefixed && brokerPrefixSeparator > 0
      ? parsed.conversationId.slice(0, brokerPrefixSeparator)
      : brokerPrefixed && params.account.defaultPlatform
        ? params.account.defaultPlatform
        : parsed.platform;
  const rawConversationId =
    brokerPrefixed && brokerPrefixSeparator > 0
      ? parsed.conversationId.slice(brokerPrefixSeparator + 1)
      : brokerPrefixed && params.account.defaultPlatform
        ? parsed.conversationId
        : parsed.conversationId;
  const normalizedRawPlatform = normalizeBrokerPlatformId(rawPlatform);
  const platform =
    params.account.platformAliases[normalizedRawPlatform] ??
    normalizeKnownChannelBrokerPlatformId(rawPlatform);
  if (params.account.platforms.length > 0 && !params.account.platforms.includes(platform)) {
    throw new Error(
      `Channel broker provider ${params.account.providerId} does not support platform ${platform}.`,
    );
  }
  const colonParts = rawConversationId.split(":");
  const explicitType = normalizeConversationType(colonParts[0]);
  const platformConversation = parsePlatformConversation({
    platform,
    rawConversationId: explicitType ? colonParts.slice(1).join(":") : rawConversationId,
  });
  const threadId =
    params.threadId == null
      ? (parsed.threadId ?? platformConversation.threadId)
      : String(params.threadId).trim() || parsed.threadId || platformConversation.threadId;
  const inferredType =
    platform === "telegram"
      ? inferTelegramChatTypeFromConversation({
          conversationId: platformConversation.conversationId,
          ...(threadId ? { threadId } : {}),
        })
      : undefined;
  return {
    platform,
    conversationId: platformConversation.conversationId,
    conversationType:
      explicitType ??
      parsed.conversationType ??
      platformConversation.conversationType ??
      inferredType ??
      (threadId ? "channel" : undefined) ??
      params.account.defaultConversationType,
    ...(threadId ? { threadId } : {}),
  };
}

export function resolveBrokerOutboundTo(params: {
  to?: string | null;
  account: ResolvedChannelBrokerAccount;
}): string | undefined {
  const explicit = params.to?.trim();
  if (explicit) {
    return normalizeBrokerTarget(explicit);
  }
  return params.account.defaultTo;
}
