import { API_CONSTANTS } from "grammy";

export type TelegramUpdateType = (typeof API_CONSTANTS.ALL_UPDATE_TYPES)[number];

export const DEFAULT_TELEGRAM_UPDATE_TYPES: ReadonlyArray<TelegramUpdateType> =
  API_CONSTANTS.DEFAULT_UPDATE_TYPES;

const TELEGRAM_GUEST_MESSAGE_UPDATE = "guest_message" as TelegramUpdateType;

export type TelegramGuestModeConfig = {
  enabled?: boolean;
};

export function shouldRequestTelegramGuestUpdates(params: {
  guest?: TelegramGuestModeConfig;
  includeGuest?: boolean;
}): boolean {
  if (typeof params.includeGuest === "boolean") {
    return params.includeGuest;
  }
  return params.guest?.enabled === true;
}

export function resolveTelegramAllowedUpdates(params?: {
  guest?: TelegramGuestModeConfig;
  includeGuest?: boolean;
}): ReadonlyArray<TelegramUpdateType> {
  const updates = [...DEFAULT_TELEGRAM_UPDATE_TYPES] as TelegramUpdateType[];
  if (!updates.includes("message_reaction")) {
    updates.push("message_reaction");
  }
  if (!updates.includes("channel_post")) {
    updates.push("channel_post");
  }
  if (
    shouldRequestTelegramGuestUpdates(params ?? {}) &&
    !updates.includes(TELEGRAM_GUEST_MESSAGE_UPDATE)
  ) {
    updates.push(TELEGRAM_GUEST_MESSAGE_UPDATE);
  }
  return updates;
}
