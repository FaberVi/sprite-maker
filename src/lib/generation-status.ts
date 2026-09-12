import type { ActiveChatRequest } from "$lib/chat-generation-finalize";
import { reportsGenerationFailure, reportsGenerationWarning } from "$lib/message-generations";

export type GenerationActivityLevel = "info" | "warning" | "error";

export type GenerationActivityEntry = {
  text: string;
  level: GenerationActivityLevel;
  at: number;
};

export type GenerationStage = {
  id: string;
  label: string;
};

export type GenerationProgress = {
  title: string;
  stages: GenerationStage[];
  currentIndex: number;
};

export function activityLevelForLine(line: string): GenerationActivityLevel {
  const lower = line.toLowerCase();
  if (reportsGenerationFailure(line)) return "error";
  if (
    /^error:/i.test(line)
    || /\bfailed\b/.test(lower)
    || lower.includes("could not")
    || lower.includes("unable to")
    || lower.includes("output stream error")
  ) {
    return "error";
  }
  if (
    reportsGenerationWarning(line)
    || lower.includes("generation_warning:")
    || /\bwarning\b/.test(lower)
    || lower.includes("retrying")
    || lower.includes("too static")
    || lower.includes("need a cleaner")
    || lower.includes("needs a cleaner")
  ) {
    return "warning";
  }
  return "info";
}

export function toActivityEntries(lines: string[], level?: GenerationActivityLevel): GenerationActivityEntry[] {
  const at = Date.now();
  return lines.map(text => ({
    text,
    level: level ?? activityLevelForLine(text),
    at,
  }));
}

export function sanitizeActivityMessage(text: string): string {
  return text
    .replace(/^GENERATION_FAILED:\s*/i, "")
    .replace(/^GENERATION_WARNING:\s*/i, "")
    .trim();
}

export function latestGenerationIssue(
  activity: GenerationActivityEntry[],
): { level: "warning" | "error"; message: string } | undefined {
  for (let index = activity.length - 1; index >= 0; index -= 1) {
    const entry = activity[index];
    if (entry.level === "error" || entry.level === "warning") {
      return {
        level: entry.level,
        message: sanitizeActivityMessage(entry.text),
      };
    }
  }
  return undefined;
}

function matchesAny(patterns: RegExp[], value: string): boolean {
  return patterns.some(pattern => pattern.test(value));
}

/** Map the active request and activity log to a user-visible phase. */
export function resolveGenerationProgress(
  request: Pick<ActiveChatRequest, "id" | "phase" | "polishMode"> | undefined,
  activity: GenerationActivityEntry[],
  sending: boolean,
): GenerationProgress {
  const latest = activity.at(-1)?.text ?? "";

  if (sending && !request) {
    return {
      title: "Starting generation",
      stages: [
        { id: "start", label: "Start" },
        { id: "work", label: "Generate" },
        { id: "finish", label: "Register" },
      ],
      currentIndex: 0,
    };
  }

  if (request?.phase === "master") {
    const currentIndex = matchesAny([/registering/i], latest)
      ? 2
      : matchesAny([/master saved|rig animation|building native rig/i], latest)
        ? 1
        : 0;
    return {
      title: "Generating source master",
      stages: [
        { id: "master", label: "Master image" },
        { id: "rig", label: "Rig animation" },
        { id: "register", label: "Register" },
      ],
      currentIndex,
    };
  }

  if (request?.id.startsWith("native-rig-")) {
    const polishHandoff = request.polishMode === "ai-polish" || request.polishMode === "full-redraw";
    if (polishHandoff && matchesAny([/polish|redraw|provider/i], latest)) {
      return {
        title: request.polishMode === "full-redraw" ? "Full redraw pass" : "AI polish pass",
        stages: [
          { id: "rig", label: "Rig frames" },
          { id: "polish", label: "Polish" },
          { id: "register", label: "Register" },
        ],
        currentIndex: matchesAny([/registering/i], latest) ? 2 : 1,
      };
    }
    const currentIndex = matchesAny([/render|saving rig|registering/i], latest)
      ? 1
      : matchesAny([/analyzing|suggesting/i], latest)
        ? 0
        : 1;
    return {
      title: "Building native rig animation",
      stages: [
        { id: "analyze", label: "Analyze" },
        { id: "render", label: "Render frames" },
        { id: "register", label: "Register" },
      ],
      currentIndex,
    };
  }

  const currentIndex = matchesAny([/registering|check|inspect|quality|validat/i], latest)
    ? 2
    : matchesAny([/render|frame|draw|generat|writing|image|running command/i], latest)
      ? 1
      : 0;
  return {
    title: currentIndex === 0
      ? "Planning with AI"
      : currentIndex === 1
        ? "Generating assets"
        : "Finalizing output",
    stages: [
      { id: "plan", label: "Plan" },
      { id: "generate", label: "Generate" },
      { id: "finish", label: "Finalize" },
    ],
    currentIndex,
  };
}
