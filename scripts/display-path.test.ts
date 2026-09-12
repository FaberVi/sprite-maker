import { describe, expect, test } from "bun:test";
import { displayPath, projectNameFromPath } from "../src/lib/display-path";

describe("display path helpers", () => {
  test("strips the Windows extended-length prefix", () => {
    expect(displayPath("\\\\?\\C:\\Users\\Utente\\Desktop\\test")).toBe("C:\\Users\\Utente\\Desktop\\test");
  });

  test("reads the project folder name from Windows paths", () => {
    expect(projectNameFromPath("\\\\?\\C:\\Users\\Utente\\Desktop\\test")).toBe("test");
    expect(projectNameFromPath("/home/user/projects/demo")).toBe("demo");
  });
});
