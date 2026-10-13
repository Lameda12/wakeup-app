import { useEffect, useState } from "react";
import { getNudge, onNudge, type NudgeInfo } from "./bridge";

export function Nudge() {
  const [info, setInfo] = useState<NudgeInfo | null>(null);
  const [now, setNow] = useState(Date.now());

  useEffect(() => {
    getNudge().then(setInfo);
    const off = onNudge(setInfo);
    const id = setInterval(() => setNow(Date.now()), 100);
    return () => {
      off();
      clearInterval(id);
    };
  }, []);

  if (!info) return <main className="nudge" />;
  const left = Math.max(0, info.timeoutSecs - (now - info.shownAtMs) / 1000);

  return (
    <main className="nudge">
      <p className="nudge-title">still here?</p>
      <p className="nudge-text">touch any key. alarm in {Math.ceil(left)}s.</p>
      <div className="nudge-bar">
        <div className="nudge-bar-fill" style={{ width: `${(left / info.timeoutSecs) * 100}%` }} />
      </div>
    </main>
  );
}
