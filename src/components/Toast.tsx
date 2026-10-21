import { useToast } from "../state/toast";
import { Icon } from "./Icon";

export function ToastHost() {
  const t = useToast((s) => s.toast);
  return (
    <div className="toast-region" role="status" aria-live="polite">
      {t && (
        <div key={t.id} className={`toast toast-${t.tone}`}>
          {t.tone === "success" && <Icon name="check" size={16} />}
          {t.tone === "warning" && <Icon name="warning" size={16} />}
          <span>{t.text}</span>
        </div>
      )}
    </div>
  );
}
