import { Icon, type IconName } from "../../components/Icon";
import { toWispError } from "../../services/errors";
import { host } from "../../services/host";
import type { PowerAction } from "../../services/types";
import { toast } from "../../state/toast";
import { useConfirm } from "../notch/actions";

const ACTIONS: { action: PowerAction; label: string; icon: IconName; confirm?: string }[] = [
  { action: "lock", label: "Lock", icon: "lock" },
  { action: "sleep", label: "Sleep", icon: "sleep", confirm: "Put the PC to sleep now?" },
  {
    action: "restart",
    label: "Restart",
    icon: "restart",
    confirm: "Restart Windows in 10 seconds? Unsaved work may be lost.",
  },
  {
    action: "shutdown",
    label: "Shut down",
    icon: "power",
    confirm: "Shut down Windows in 10 seconds? Unsaved work may be lost.",
  },
];

async function run(action: PowerAction) {
  try {
    await host().powerAction(action);
    if (action === "restart" || action === "shutdown")
      toast("Starting in 10 seconds — press Cancel to stop", "warning", 8000);
    if (action === "cancel") toast("Cancelled", "success");
  } catch (e) {
    toast(toWispError(e).message, "warning", 3500);
  }
}

export default function PowerTab() {
  return (
    <div className="power">
      <div className="power-grid">
        {ACTIONS.map((a) => (
          <button
            key={a.action}
            className={`power-btn power-${a.action}`}
            onClick={() =>
              a.confirm
                ? useConfirm.getState().ask(a.confirm, a.label, () => run(a.action))
                : void run(a.action)
            }
          >
            <Icon name={a.icon} size={24} />
            <span>{a.label}</span>
          </button>
        ))}
      </div>
      <div className="row">
        <button className="btn btn-small" onClick={() => void run("cancel")}>
          Cancel a pending restart / shut down
        </button>
        <button className="btn btn-small btn-ghost" onClick={() => void host().setTucked(true)}>
          <Icon name="chevron-up" size={14} /> Tuck Wisp away
        </button>
        <button className="btn btn-small btn-ghost" onClick={() => void host().quit()}>
          Quit Wisp
        </button>
      </div>
    </div>
  );
}
