import { describe, expect, it } from "vitest";
import {
  INITIAL_SESSION,
  applyDone,
  applyError,
  applyProgress,
  dismissError,
  failLocal,
  parseSeed,
  requestCancel,
  startSession,
  toggleFavorite,
  type GenImage,
} from "./genSession";

const generating = () => startSession(INITIAL_SESSION, "job-1", 4);
const img = (id: string, path: string, favorite = false): GenImage => ({ id, path, favorite });

describe("startSession", () => {
  it("generating으로 전이하고 요청 장수를 셀 수로 기록한다", () => {
    const s = generating();
    expect(s.phase).toBe("generating");
    expect(s.jobId).toBe("job-1");
    expect(s.cells).toBe(4);
    expect(s.progress).toBe(0);
  });

  it("이전 결과 이미지·시드는 유지하고 notice/error는 초기화한다", () => {
    const prev = {
      ...INITIAL_SESSION,
      images: [img("g0", "images/a.png")],
      seed: 42,
      notice: "지난 고지",
      error: { code: "E_X", message: "x" },
    };
    const s = startSession(prev, "job-2", 2);
    expect(s.images).toEqual([img("g0", "images/a.png")]);
    expect(s.seed).toBe(42);
    expect(s.notice).toBeNull();
    expect(s.error).toBeNull();
  });
});

describe("applyProgress", () => {
  it("진행률과 고지를 반영한다", () => {
    let s = applyProgress(generating(), { jobId: "job-1", progress: 0.5 });
    expect(s.progress).toBe(0.5);
    s = applyProgress(s, {
      jobId: "job-1",
      progress: 0.6,
      notice: "메모리가 부족해 크기를 낮췄어요.",
    });
    expect(s.notice).toBe("메모리가 부족해 크기를 낮췄어요.");
    // notice 없는 후속 이벤트가 고지를 지우면 안 됨
    s = applyProgress(s, { jobId: "job-1", progress: 0.7 });
    expect(s.notice).toBe("메모리가 부족해 크기를 낮췄어요.");
  });

  it("다른 잡의 이벤트는 무시한다", () => {
    const s = generating();
    expect(applyProgress(s, { jobId: "other", progress: 0.9 })).toBe(s);
    expect(applyProgress(INITIAL_SESSION, { jobId: "job-1", progress: 0.9 })).toBe(INITIAL_SESSION);
  });
});

describe("applyDone", () => {
  it("idle로 복귀하고 결과 이미지(id 매칭)·시드를 기록한다", () => {
    const s = applyDone(generating(), {
      jobId: "job-1",
      generationIds: ["g1", "g2"],
      imagePaths: ["images/1.png", "images/2.png"],
      seed: 1234,
    });
    expect(s.phase).toBe("idle");
    expect(s.jobId).toBeNull();
    expect(s.images).toEqual([img("g1", "images/1.png"), img("g2", "images/2.png")]);
    expect(s.seed).toBe(1234);
    expect(s.cells).toBe(0);
  });

  it("폴백 고지는 완료 후에도 유지한다", () => {
    let s = applyProgress(generating(), { jobId: "job-1", progress: 0.3, notice: "폴백 고지" });
    s = applyDone(s, { jobId: "job-1", generationIds: [], imagePaths: [], seed: 1 });
    expect(s.notice).toBe("폴백 고지");
  });

  it("다른 잡의 완료는 무시한다", () => {
    const s = generating();
    expect(applyDone(s, { jobId: "other", generationIds: [], imagePaths: [], seed: 1 })).toBe(s);
  });
});

describe("applyError", () => {
  it("error 상태로 전이하고 AppError를 보관한다", () => {
    const s = applyError(generating(), {
      jobId: "job-1",
      error: { code: "E_ENGINE", message: "엔진이 응답하지 않아요." },
    });
    expect(s.phase).toBe("error");
    expect(s.error?.code).toBe("E_ENGINE");
  });

  it("E_CANCELED는 에러가 아니라 조용한 idle 복귀 (이전 결과 유지)", () => {
    const withImages = { ...generating(), images: [img("g0", "images/a.png")] };
    const s = applyError(requestCancel(withImages), {
      jobId: "job-1",
      error: { code: "E_CANCELED", message: "취소되었어요." },
    });
    expect(s.phase).toBe("idle");
    expect(s.error).toBeNull();
    expect(s.cancelRequested).toBe(false);
    expect(s.images).toEqual([img("g0", "images/a.png")]);
  });

  it("다른 잡의 에러는 무시한다", () => {
    const s = generating();
    expect(applyError(s, { jobId: "other", error: { code: "E_X", message: "x" } })).toBe(s);
  });
});

describe("toggleFavorite", () => {
  const done = () =>
    applyDone(generating(), {
      jobId: "job-1",
      generationIds: ["g1", "g2"],
      imagePaths: ["images/1.png", "images/2.png"],
      seed: 7,
    });

  it("해당 id만 토글한다", () => {
    const s = toggleFavorite(done(), "g2");
    expect(s.images).toEqual([img("g1", "images/1.png"), img("g2", "images/2.png", true)]);
  });

  it("두 번 토글하면 원상복구 (실패 롤백 경로)", () => {
    const s = toggleFavorite(toggleFavorite(done(), "g1"), "g1");
    expect(s.images[0].favorite).toBe(false);
  });

  it("없는 id는 아무것도 바꾸지 않는다", () => {
    const s = done();
    expect(toggleFavorite(s, "nope").images).toEqual(s.images);
  });
});

describe("requestCancel / dismissError / failLocal", () => {
  it("requestCancel은 generating에서만 동작한다", () => {
    expect(requestCancel(generating()).cancelRequested).toBe(true);
    expect(requestCancel(INITIAL_SESSION)).toBe(INITIAL_SESSION);
  });

  it("failLocal은 invoke 거부를 error 상태로 만든다", () => {
    const s = failLocal(generating(), { code: "E_STATE", message: "엔진 상태를 찾을 수 없어요." });
    expect(s.phase).toBe("error");
    expect(s.jobId).toBeNull();
  });

  it("dismissError는 error에서 idle로 복귀한다", () => {
    const err = failLocal(generating(), { code: "E_X", message: "x" });
    const s = dismissError(err);
    expect(s.phase).toBe("idle");
    expect(s.error).toBeNull();
    expect(dismissError(INITIAL_SESSION)).toBe(INITIAL_SESSION);
  });
});

describe("parseSeed", () => {
  it("빈칸은 랜덤(undefined)", () => {
    expect(parseSeed("")).toBeUndefined();
    expect(parseSeed("  ")).toBeUndefined();
  });

  it("정수 문자열을 파싱한다", () => {
    expect(parseSeed("42")).toBe(42);
    expect(parseSeed(" -7 ")).toBe(-7);
  });

  it("유효하지 않은 입력은 null", () => {
    expect(parseSeed("abc")).toBeNull();
    expect(parseSeed("1.5")).toBeNull();
    expect(parseSeed("9007199254740993")).toBeNull(); // safe integer 초과
  });
});
