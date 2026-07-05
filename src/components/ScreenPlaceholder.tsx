interface ScreenPlaceholderProps {
  title: string;
  description: string;
}

/** T0.2 골격용 화면 플레이스홀더. 각 화면의 실제 UI는 해당 태스크에서 대체한다. */
export default function ScreenPlaceholder({ title, description }: ScreenPlaceholderProps) {
  return (
    <section className="flex h-full flex-col items-center justify-center gap-2 p-8 text-center">
      <h1 className="text-xl font-semibold text-text">{title}</h1>
      <p className="text-sm text-text-sub">{description}</p>
    </section>
  );
}
