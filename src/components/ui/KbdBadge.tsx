import React from "react";

interface KbdBadgeProps {
  shortcut: string | null;
  className?: string;
  isRecording?: boolean;
  size?: "sm" | "md";
}

export const KbdBadge: React.FC<KbdBadgeProps> = ({
  shortcut,
  className = "",
  isRecording = false,
  size = "md",
}) => {
  if (isRecording) {
    return (
      <span className={`kbd-badge-recording size-${size} ${className}`}>
        <span className="kbd-pulsing-dot" />
        <span>请按下快捷键组合...</span>
      </span>
    );
  }

  if (!shortcut) {
    return <span className={`kbd-badge-empty size-${size} ${className}`}>未绑定</span>;
  }

  const keys = shortcut.split("+").map((k) => k.trim());

  return (
    <div className={`kbd-badge-container size-${size} ${className}`}>
      {keys.map((key, index) => {
        let displayKey = key;
        if (key.toLowerCase() === "commandorcontrol" || key.toLowerCase() === "ctrl") displayKey = "Ctrl";
        if (key.toLowerCase() === "alt") displayKey = "Alt";
        if (key.toLowerCase() === "shift") displayKey = "Shift";
        if (key.toLowerCase() === "enter" || key === "↵") displayKey = "↵ Enter";

        return (
          <React.Fragment key={index}>
            <kbd className={`kbd-key size-${size}`}>{displayKey}</kbd>
            {index < keys.length - 1 && (
              <span className="kbd-separator" aria-hidden="true">
                +
              </span>
            )}
          </React.Fragment>
        );
      })}
    </div>
  );
};
