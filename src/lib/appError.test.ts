import { describe, expect, it } from "vitest";
import { isAppError } from "./appError";

describe("isAppError", () => {
  it("code/message를 가진 객체를 AppError로 식별한다", () => {
    expect(isAppError({ code: "E_ENGINE", message: "엔진이 응답하지 않아요" })).toBe(true);
    expect(isAppError({ code: "E_X", message: "오류", detail: "raw" })).toBe(true);
  });

  it("형태가 다르면 false를 반환한다", () => {
    expect(isAppError(null)).toBe(false);
    expect(isAppError(undefined)).toBe(false);
    expect(isAppError("문자열")).toBe(false);
    expect(isAppError({ code: 1, message: "x" })).toBe(false);
    expect(isAppError({ message: "code 없음" })).toBe(false);
  });
});
