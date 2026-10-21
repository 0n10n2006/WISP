import { forwardRef, useEffect, useImperativeHandle, useRef, type KeyboardEvent } from "react";
import { Icon } from "./Icon";

export interface PromptBoxHandle {
  focus(): void;
}

interface Props {
  value: string;
  onChange(v: string): void;
  onSubmit(): void;
  onCancel(): void;
  onEscape(): void;
  busy: boolean;
  disabled?: boolean;
  placeholder?: string;
}

const MAX_ROWS_PX = 140;

export const PromptBox = forwardRef<PromptBoxHandle, Props>(function PromptBox(
  { value, onChange, onSubmit, onCancel, onEscape, busy, disabled, placeholder = "Ask Wisp" },
  ref,
) {
  const ta = useRef<HTMLTextAreaElement>(null);
  useImperativeHandle(ref, () => ({ focus: () => ta.current?.focus() }));

  useEffect(() => {
    const el = ta.current;
    if (!el) return;
    el.style.height = "auto";
    el.style.height = `${Math.min(el.scrollHeight, MAX_ROWS_PX)}px`;
  }, [value]);

  const onKeyDown = (e: KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === "Enter" && !e.shiftKey && !e.nativeEvent.isComposing) {
      e.preventDefault();
      if (!busy) onSubmit();
    } else if (e.key === "Escape") {
      e.preventDefault();
      if (busy) onCancel();
      else onEscape();
    }
  };

  return (
    <div className="prompt">
      <label htmlFor="wisp-prompt" className="sr-only">
        Ask Wisp
      </label>
      <textarea
        id="wisp-prompt"
        ref={ta}
        rows={1}
        value={value}
        placeholder={placeholder}
        disabled={disabled}
        onChange={(e) => onChange(e.target.value)}
        onKeyDown={onKeyDown}
        maxLength={32000}
        spellCheck
      />
      {busy ? (
        <button
          className="icon-btn icon-btn-solid stop"
          onClick={onCancel}
          aria-label="Stop answer"
        >
          <Icon name="stop" size={16} />
        </button>
      ) : (
        <button
          className="icon-btn icon-btn-solid"
          onClick={onSubmit}
          disabled={disabled || !value.trim()}
          aria-label="Send"
        >
          <Icon name="send" size={16} />
        </button>
      )}
    </div>
  );
});
