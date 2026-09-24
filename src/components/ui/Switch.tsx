/** 开关。没有图标、没有光晕：开是一块墨，关是一圈线 */
export function Switch({
  value,
  onChange,
  label,
  disabled = false,
  id,
}: {
  value: boolean;
  onChange: (next: boolean) => void;
  label: string;
  disabled?: boolean;
  id?: string;
}) {
  return (
    <button
      id={id}
      type="button"
      role="switch"
      aria-checked={value}
      aria-label={label}
      disabled={disabled}
      className={`switch ${value ? "is-on" : ""}`}
      onClick={() => onChange(!value)}
    >
      <span className="switch-knob" />
    </button>
  );
}
