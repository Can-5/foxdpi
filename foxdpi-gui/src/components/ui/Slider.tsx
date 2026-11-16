import { cn } from "@/utils/cn";

interface SliderProps {
  value?: number;
  onValueChange?: (value: number) => void;
  min?: number;
  max?: number;
  step?: number;
  label?: string;
  className?: string;
}

export function Slider({
  value = 0,
  onValueChange,
  min = 0,
  max = 100,
  step = 1,
  label,
  className,
}: SliderProps) {
  return (
    <div className={cn("flex flex-col gap-2", className)}>
      {label && (
        <span className="text-sm text-fox-500">{label}</span>
      )}
      <input
        type="range"
        min={min}
        max={max}
        step={step}
        value={value}
        onChange={(e) => onValueChange?.(Number(e.target.value))}
        className="h-2 w-full cursor-pointer appearance-none rounded-full bg-fox-200 accent-fox-600"
        aria-label={label ?? "Slider"}
      />
      <span className="text-sm text-fox-700 text-right">{value}</span>
    </div>
  );
}
