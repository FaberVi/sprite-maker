import { describe, expect, test } from "bun:test";
import {
  activityLevelForLine,
  latestGenerationIssue,
  resolveGenerationProgress,
  toActivityEntries,
} from "../src/lib/generation-status";

describe("generation status", () => {
  test("classifies warnings and errors", () => {
    expect(activityLevelForLine("Analyzing sprite fit for native rigging")).toBe("info");
    expect(activityLevelForLine("First rig pass was too static — retrying with simplified motion")).toBe("warning");
    expect(activityLevelForLine("GENERATION_FAILED: provider rejected the image")).toBe("error");
    expect(activityLevelForLine("Output stream error: connection reset")).toBe("error");
  });

  test("resolves master phase stages", () => {
    const progress = resolveGenerationProgress(
      { id: "req-1", phase: "master", polishMode: "rig" },
      toActivityEntries(["Master saved — building native rig animation"]),
      false,
    );
    expect(progress.title).toBe("Generating source master");
    expect(progress.currentIndex).toBe(1);
  });

  test("surfaces the latest issue in the activity log", () => {
    const issue = latestGenerationIssue(toActivityEntries([
      "Suggesting joint points and pose frames",
      "Sprite silhouette is noisy and may need a cleaner master",
      "Saving rig and rendering frames deterministically",
    ]));
    expect(issue?.level).toBe("warning");
    expect(issue?.message).toContain("cleaner master");
  });
});
