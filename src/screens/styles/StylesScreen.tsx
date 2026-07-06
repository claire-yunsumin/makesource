import { useCallback, useEffect, useState } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import { dataDir } from "@tauri-apps/api/path";
import Toast from "../../components/Toast";
import { isAppError, type AppError } from "../../lib/appError";
import { APP_DATA_DIR_NAME, joinImagePath } from "../../lib/imagePath";
import { useNavigate } from "react-router-dom";
import { styleDelete, stylesList, type Style } from "../../lib/tauri";
import { useGenerateStore } from "../generate/store";
import EssenceWizard from "./EssenceWizard";

/**
 * 스타일 관리 (04 §4.3, T4.2): 카드 그리드 + 에센스 마법사.
 * [생성에 사용]은 IP-Adapter 경로 연결(T4.3)에서, 정밀 학습(LoRA)은 M6에서.
 */
export default function StylesScreen() {
  const [styles, setStyles] = useState<Style[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<AppError | null>(null);
  const [dataRoot, setDataRoot] = useState<string | null>(null);
  const [wizardOpen, setWizardOpen] = useState(false);
  const [toast, setToast] = useState<{ message: string; tone: "error" | "success" } | null>(null);
  const navigate = useNavigate();

  useEffect(() => {
    dataDir()
      .then((dir) => setDataRoot(`${dir.replace(/\/$/, "")}/${APP_DATA_DIR_NAME}`))
      .catch(() => setDataRoot(null));
  }, []);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      setStyles(await stylesList());
    } catch (e) {
      setError(
        isAppError(e)
          ? e
          : { code: "E_UNKNOWN", message: "스타일을 불러오지 못했어요.", detail: String(e) },
      );
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  const handleDelete = useCallback(async (style: Style) => {
    if (!window.confirm(`'${style.name}' 스타일을 삭제할까요? 참조 이미지도 함께 지워져요.`)) {
      return;
    }
    try {
      await styleDelete(style.id);
      setStyles((prev) => prev.filter((s) => s.id !== style.id));
      setToast({ message: "스타일을 삭제했어요.", tone: "success" });
    } catch (e) {
      setToast({
        message: isAppError(e) ? e.message : "스타일을 삭제하지 못했어요.",
        tone: "error",
      });
    }
  }, []);

  return (
    <div className="h-full overflow-y-auto p-6">
      <div className="mb-4 flex items-center gap-3">
        <h1 className="text-base font-medium text-text">스타일</h1>
        <button
          type="button"
          onClick={() => setWizardOpen(true)}
          className="ease-out-ui rounded-md bg-primary px-3 py-1.5 text-xs font-medium text-white transition-colors duration-150 hover:bg-primary-hover"
        >
          + 새 스타일 (에센스 · 몇 분)
        </button>
        <span className="text-xs text-text-sub">정밀 학습(LoRA)은 준비 중이에요</span>
      </div>

      {error && (
        <div role="alert" className="mb-4 rounded-md border border-error bg-surface-2 px-3 py-2">
          <p className="text-sm text-error">{error.message}</p>
          <button
            type="button"
            onClick={() => void load()}
            className="mt-2 rounded-sm border border-border px-2 py-1 text-xs text-text-sub hover:bg-surface"
          >
            다시 시도
          </button>
        </div>
      )}

      {!loading && !error && styles.length === 0 ? (
        <div className="flex h-2/3 flex-col items-center justify-center gap-3 text-center">
          <span aria-hidden className="text-4xl text-text-sub">
            ◐
          </span>
          <p className="text-base font-medium text-text">아직 만든 스타일이 없어요</p>
          <p className="max-w-sm text-sm text-text-sub">
            브랜드 이미지를 몇 장 드롭하면 학습 없이 몇 분 만에 브랜드 톤 스타일을 만들 수 있어요.
          </p>
        </div>
      ) : (
        <div className="grid grid-cols-2 gap-4 md:grid-cols-3 xl:grid-cols-4">
          {styles.map((style) => {
            const thumbAbs = dataRoot && style.thumb ? joinImagePath(dataRoot, style.thumb) : null;
            return (
              <div key={style.id} className="overflow-hidden rounded-lg bg-surface-2 shadow-card">
                <div className="flex aspect-video items-center justify-center bg-surface">
                  {thumbAbs ? (
                    <img
                      src={convertFileSrc(thumbAbs)}
                      alt={style.name}
                      className="h-full w-full object-cover"
                    />
                  ) : (
                    <span aria-hidden className="text-2xl text-text-sub">
                      ◐
                    </span>
                  )}
                </div>
                <div className="space-y-2 p-3">
                  <div className="flex items-center justify-between gap-2">
                    <p className="truncate text-sm text-text">{style.name}</p>
                    <span className="shrink-0 rounded-sm border border-border px-1.5 py-0.5 text-[10px] text-text-sub">
                      {style.kind === "essence" ? "에센스" : "LoRA"}
                    </span>
                  </div>
                  {style.essencePrompt && (
                    <p className="line-clamp-2 text-xs text-text-sub">{style.essencePrompt}</p>
                  )}
                  <div className="flex gap-1">
                    <button
                      type="button"
                      onClick={() => {
                        // 생성 화면 스타일 선택에 반영하고 이동 (T4.3)
                        useGenerateStore.setState({ styleId: style.id });
                        navigate("/generate");
                      }}
                      className="ease-out-ui flex-1 rounded-md border border-border px-2 py-1 text-xs text-text transition-colors duration-150 hover:bg-surface"
                    >
                      생성에 사용
                    </button>
                    <button
                      type="button"
                      onClick={() => void handleDelete(style)}
                      className="ease-out-ui rounded-md border border-border px-2 py-1 text-xs text-text-sub transition-colors duration-150 hover:border-error hover:text-error"
                    >
                      삭제
                    </button>
                  </div>
                </div>
              </div>
            );
          })}
        </div>
      )}

      {wizardOpen && (
        <EssenceWizard
          onClose={() => setWizardOpen(false)}
          onSaved={() => {
            void load();
            setToast({ message: "스타일을 저장했어요.", tone: "success" });
          }}
        />
      )}
      {toast && <Toast message={toast.message} tone={toast.tone} onClose={() => setToast(null)} />}
    </div>
  );
}
