import type { FastMode } from "../shared/string-coerce.js";

export const formatFastModeLabel = (mode: FastMode): string | null => {
  if (mode === "auto") {
    return "Fast:auto";
  }
  if (!mode) {
    return null;
  }
  return "Fast";
};
