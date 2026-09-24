/**
 * 几个互斥选项：选中的是墨色加一道下划线，其余是灰字。
 * 取代原来的分段胶囊——胶囊的底色和边框在这里不带来任何信息。
 */
export function Choices<T extends string>({
  value,
  options,
  onChange,
  ariaLabel,
  className = "",
}: {
  value: T;
  options: Array<{ value: T; label: string }>;
  onChange: (next: T) => void;
  ariaLabel: string;
  className?: string;
}) {
  return (
    <div className={`choices ${className}`} role="radiogroup" aria-label={ariaLabel}>
      {options.map((option) => (
        <button
          key={option.value}
          type="button"
          role="radio"
          aria-checked={value === option.value}
          className={`choice ${value === option.value ? "is-on" : ""}`}
          onClick={() => onChange(option.value)}
        >
          {option.label}
        </button>
      ))}
    </div>
  );
}
