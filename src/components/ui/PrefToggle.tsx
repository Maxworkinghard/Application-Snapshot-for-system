/** 偏好设置共用的开关控件，样式来自 app.css 的 toggle-switch-btn */
export function PrefToggle({
  value,
  onChange,
  label,
  disabled = false,
}: {
  value: boolean;
  onChange: (next: boolean) => void;
  label: string;
  disabled?: boolean;
}) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={value}
      aria-label={label}
      disabled={disabled}
      className={`toggle-switch-btn ${value ? "on" : ""}`}
      style={disabled ? { opacity: 0.45, cursor: "not-allowed" } : undefined}
      onClick={() => {
        if (!disabled) onChange(!value);
      }}
    >
      <span className="toggle-thumb" />
    </button>
  );
}
