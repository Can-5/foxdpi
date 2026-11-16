import { cn } from "@/utils/cn";

interface Option {
  value: string;
  label: string;
}

interface SegmentedControlProps {
  value: string;
  onValueChange: (value: string) => void;
  options: Option[];
  className?: string;
}

export function SegmentedControl({ value, onValueChange, options, className }: SegmentedControlProps) {
  return (
    <div
      role="group"
      aria-label="Bypass stratejisi seçimi"
      className={cn(
        "inline-flex h-10 w-full overflow-hidden rounded-lg bg-gray-100 p-1 text-fox-700",
        className,
      )}
    >
      {options.map((option) => (
        <button
          key={option.value}
          type="button"
          role="radio"
          aria-checked={value === option.value}
          onClick={() => onValueChange(option.value)}
          className={cn(
            "flex w-full items-center justify-center rounded-md px-3 text-sm font-medium transition",
            "focus-ring",
            value === option.value
              ? "bg-white shadow-sm ring-1 ring-fox-400"
              : "hover:bg-gray-200",
          )}
        >
          {option.label}
        </button>
      ))}
    </div>
  );
}
