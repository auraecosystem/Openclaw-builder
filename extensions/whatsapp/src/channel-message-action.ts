import { jsonResult } from "openclaw/plugin-sdk/channel-actions";
import {
  handleWhatsAppAction,
  normalizeWhatsAppTarget,
  readStringParam,
  resolveAuthorizedWhatsAppOutboundTarget,
  resolveWhatsAppAccount,
  resolveWhatsAppMediaMaxBytes,
  sendMessageWhatsApp,
} from "./channel-message-action.runtime.js";
import { handleWhatsAppReactAction } from "./channel-react-action.js";

const WHATSAPP_CHANNEL = "whatsapp" as const;

type WhatsAppMessageActionParams = {
  action: string;
  params: Record<string, unknown>;
  cfg: Parameters<typeof handleWhatsAppAction>[1];
  accountId?: string | null;
  requesterSenderId?: string | null;
  mediaAccess?: {
    localRoots?: readonly string[];
    readFile?: (filePath: string) => Promise<Buffer>;
  };
  mediaLocalRoots?: readonly string[];
  mediaReadFile?: (filePath: string) => Promise<Buffer>;
  toolContext?: {
    currentChannelId?: string | null;
    currentChannelProvider?: string | null;
    currentMessageId?: string | number | null;
  };
};

function readUploadFileMediaSource(args: Record<string, unknown>): string | undefined {
  return (
    readStringParam(args, "media", { trim: false }) ??
    readStringParam(args, "mediaUrl", { trim: false }) ??
    readStringParam(args, "filePath", { trim: false }) ??
    readStringParam(args, "path", { trim: false }) ??
    readStringParam(args, "fileUrl", { trim: false })
  );
}

function readUploadFileCaptionText(args: Record<string, unknown>): string {
  return (
    readStringParam(args, "message", { allowEmpty: true }) ??
    readStringParam(args, "content", { allowEmpty: true }) ??
    readStringParam(args, "caption", { allowEmpty: true }) ??
    ""
  );
}

function readBooleanParam(args: Record<string, unknown>, key: string): boolean | undefined {
  const value = args[key];
  if (typeof value === "boolean") {
    return value;
  }
  if (typeof value !== "string") {
    return undefined;
  }
  const normalized = value.trim().toLowerCase();
  if (normalized === "true") {
    return true;
  }
  if (normalized === "false") {
    return false;
  }
  return undefined;
}

function hasUploadFileBufferPayload(args: Record<string, unknown>): boolean {
  return readStringParam(args, "buffer", { trim: false }) !== undefined;
}

function extractBase64Payload(encoded: string): string {
  const match = /^data:[^;]+;base64,(.*)$/i.exec(encoded.trim());
  return match ? match[1] : encoded;
}

function estimateBase64DecodedBytes(encoded: string): number {
  const compact = extractBase64Payload(encoded).replace(/\s/g, "");
  if (!compact) {
    return 0;
  }
  const padding = compact.endsWith("==") ? 2 : compact.endsWith("=") ? 1 : 0;
  return Math.max(0, Math.floor((compact.length * 3) / 4) - padding);
}

function decodeUploadFileMediaPayload(params: {
  args: Record<string, unknown>;
  encoded: string;
  maxBytes?: number;
}):
  | {
      buffer: Buffer;
      contentType?: string;
      fileName?: string;
    }
  | undefined {
  if (params.maxBytes !== undefined) {
    const estimatedBytes = estimateBase64DecodedBytes(params.encoded);
    if (estimatedBytes > params.maxBytes) {
      throw new Error(
        `WhatsApp upload-file buffer exceeds configured media limit (${estimatedBytes} bytes > ${params.maxBytes} bytes).`,
      );
    }
  }
  const contentType =
    readStringParam(params.args, "contentType") ?? readStringParam(params.args, "mimeType");
  const fileName =
    readStringParam(params.args, "filename") ?? readStringParam(params.args, "fileName");
  const buffer = Buffer.from(extractBase64Payload(params.encoded), "base64");
  if (params.maxBytes !== undefined && buffer.byteLength > params.maxBytes) {
    throw new Error(
      `WhatsApp upload-file buffer exceeds configured media limit (${buffer.byteLength} bytes > ${params.maxBytes} bytes).`,
    );
  }
  return {
    buffer,
    ...(contentType ? { contentType } : {}),
    ...(fileName ? { fileName } : {}),
  };
}

async function handleWhatsAppUploadFileAction(params: WhatsAppMessageActionParams) {
  const mediaUrl = readUploadFileMediaSource(params.params);
  const encodedPayload = readStringParam(params.params, "buffer", { trim: false });
  if (!mediaUrl && !hasUploadFileBufferPayload(params.params)) {
    throw new Error(
      "WhatsApp upload-file requires media, mediaUrl, filePath, path, fileUrl, or buffer.",
    );
  }
  const to = readStringParam(params.params, "to", { required: true });
  const resolved = resolveAuthorizedWhatsAppOutboundTarget({
    cfg: params.cfg,
    chatJid: to,
    accountId: params.accountId ?? undefined,
    actionLabel: "upload-file",
  });
  const account = resolveWhatsAppAccount({
    cfg: params.cfg,
    accountId: resolved.accountId,
  });
  const mediaPayload = encodedPayload
    ? decodeUploadFileMediaPayload({
        args: params.params,
        encoded: encodedPayload,
        maxBytes: resolveWhatsAppMediaMaxBytes(account),
      })
    : undefined;
  const result = await sendMessageWhatsApp(resolved.to, readUploadFileCaptionText(params.params), {
    verbose: false,
    cfg: params.cfg,
    ...(mediaUrl && !mediaPayload ? { mediaUrl } : {}),
    ...(mediaPayload ? { mediaPayload } : {}),
    mediaAccess: params.mediaAccess,
    mediaLocalRoots: params.mediaLocalRoots,
    mediaReadFile: params.mediaReadFile,
    gifPlayback: readBooleanParam(params.params, "gifPlayback") ?? undefined,
    audioAsVoice:
      readBooleanParam(params.params, "asVoice") ??
      readBooleanParam(params.params, "audioAsVoice") ??
      undefined,
    forceDocument:
      readBooleanParam(params.params, "forceDocument") ??
      readBooleanParam(params.params, "asDocument") ??
      undefined,
    accountId: resolved.accountId,
  });
  return jsonResult({
    ok: true,
    channel: WHATSAPP_CHANNEL,
    action: "upload-file",
    messageId: result.messageId,
    toJid: result.toJid,
  });
}

export async function handleWhatsAppMessageAction(params: WhatsAppMessageActionParams) {
  if (params.action === "upload-file") {
    return await handleWhatsAppUploadFileAction(params);
  }
  if (params.action === "react") {
    return await handleWhatsAppReactAction(params);
  }
  if (params.action !== "list-reply") {
    throw new Error(`Action ${params.action} is not supported for provider ${WHATSAPP_CHANNEL}.`);
  }

  const isWhatsAppSource = params.toolContext?.currentChannelProvider === WHATSAPP_CHANNEL;
  const explicitTarget =
    readStringParam(params.params, "chatJid") ??
    readStringParam(params.params, "chatId") ??
    readStringParam(params.params, "to");
  const normalizedTarget = explicitTarget ? normalizeWhatsAppTarget(explicitTarget) : null;
  const normalizedCurrent =
    isWhatsAppSource && params.toolContext?.currentChannelId
      ? normalizeWhatsAppTarget(params.toolContext.currentChannelId)
      : null;
  const isCurrentChat =
    normalizedTarget != null && normalizedCurrent != null && normalizedTarget === normalizedCurrent;
  const messageId =
    readStringParam(params.params, "messageId") ??
    readStringParam(params.params, "replyToId") ??
    (isCurrentChat && params.toolContext?.currentMessageId != null
      ? String(params.toolContext.currentMessageId)
      : undefined);

  return await handleWhatsAppAction(
    {
      ...params.params,
      action: "list-reply",
      accountId: params.accountId ?? undefined,
      ...(messageId ? { messageId } : {}),
    },
    params.cfg,
  );
}
