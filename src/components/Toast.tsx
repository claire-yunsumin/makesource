import { useEffect } from "react";

interface ToastProps {
  message: string;
  tone?: "error" | "warn" | "success";
  /** 자동 닫힘까지 ms (기본 4000) */
  duration?: number;
  onClose: () => void;
}

const TONE_CLASS = {
  error: "bg-error",
  warn: "bg-warn",
  success: "bg-success",
} as const;

/** 우하단 토스트 (04 §5). 표시 후 duration 뒤 자동으로 onClose를 호출한다. */
export default function Toast({ message, tone = "error", duration = 4000, onClose }: ToastProps) {
  useEffect(() => {
    const timer = window.setTimeout(onClose, duration);
    return () => window.clearTimeout(timer);
  }, [duration, onClose]);

  return (
    <div
      role={tone === "error" ? "alert" : "status"}
      className={`fixed bottom-6 right-6 z-50 max-w-sm rounded-md px-4 py-3 text-sm text-white shadow-card ${TONE_CLASS[tone]}`}
    >
      {message}
    </div>
  );
}
