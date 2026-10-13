import { useEffect, useRef, useState } from "react";
import {
  getAlarm,
  onAlarm,
  onEyes,
  submitPhrase,
  typingStarted,
  type AlarmInfo,
  type AlarmReason,
  type Eyes,
} from "./bridge";

const REASONS: Record<AlarmReason, string> = {
  EyesClosed: "your eyes were closed",
  HeadDown: "your head was down at your phone",
  Idle: "you zoned out and missed the nudge",
  LostWhileDrowsy: "your head went down on the desk",
};

function useElapsed(since: number | undefined): string {
  const [now, setNow] = useState(Date.now());
  useEffect(() => {
    const id = setInterval(() => setNow(Date.now()), 250);
    return () => clearInterval(id);
  }, []);
  if (since === undefined) return "0:00";
  const s = Math.max(0, Math.floor((now - since) / 1000));
  return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, "0")}`;
}

/** Length of the prefix of `typed` that matches `phrase` (case-insensitive). */
function matchedPrefix(typed: string, phrase: string): number {
  let i = 0;
  while (i < typed.length && i < phrase.length && typed[i].toLowerCase() === phrase[i].toLowerCase()) i++;
  return i;
}

export function Alarm({ primary }: { primary: boolean }) {
  const [info, setInfo] = useState<AlarmInfo | null>(null);
  const [eyes, setEyes] = useState<Eyes>(null);
  const [typed, setTyped] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const input = useRef<HTMLInputElement>(null);
  const elapsed = useElapsed(info?.startedAtMs);

  useEffect(() => {
    getAlarm().then(setInfo);
    const offAlarm = onAlarm((next) => {
      setInfo(next);
      setTyped("");
      setError(null);
    });
    const offEyes = onEyes(setEyes);
    return () => {
      offAlarm();
      offEyes();
    };
  }, []);

  // Keep the cursor in the box: clicking around shouldn't strand the user.
  useEffect(() => {
    if (!primary) return;
    const refocus = () => input.current?.focus();
    refocus();
    window.addEventListener("focus", refocus);
    document.addEventListener("click", refocus);
    return () => {
      window.removeEventListener("focus", refocus);
      document.removeEventListener("click", refocus);
    };
  }, [primary, info]);

  if (!info) return <main className="alarm" />;

  const phrase = info.phrase;
  const ok = matchedPrefix(typed, phrase);
  const wrong = typed.length > ok;

  async function change(value: string) {
    if (!typed && value) typingStarted();
    setTyped(value);
    setError(null);
    if (value.trim().toLowerCase() === phrase.toLowerCase() && !busy) {
      setBusy(true);
      const result = await submitPhrase(value);
      setBusy(false);
      if (!result.ok) {
        setError(result.error);
        setTyped("");
      }
    }
  }

  return (
    <main className="alarm">
      <div className="alarm-frame" aria-hidden />
      <section className="alarm-body">
        <p className="alarm-meta">
          ~/wakeup <span className="dim">· ringing {elapsed}</span>
        </p>
        <h1 className="alarm-title">wake up.</h1>
        <p className="alarm-reason">{REASONS[info.reason]}.</p>

        {primary ? (
          <>
            <p className="alarm-instruction">type this, eyes open, to stop it:</p>
            <p className="phrase" aria-label={phrase}>
              <span className="phrase-ok">{phrase.slice(0, ok)}</span>
              {wrong ? <span className="phrase-bad">{phrase.slice(ok, ok + 1) || " "}</span> : null}
              <span className="phrase-todo">{phrase.slice(ok + (wrong ? 1 : 0))}</span>
            </p>
            <label className="prompt">
              <span className="caret" aria-hidden>
                &gt;
              </span>
              <input
                ref={input}
                value={typed}
                onChange={(e) => change(e.target.value)}
                spellCheck={false}
                autoComplete="off"
                autoCorrect="off"
                autoCapitalize="off"
                aria-label="dismiss phrase"
                disabled={busy}
              />
            </label>
            <p className="status" role="status">
              {error ? (
                <span className="bad">{error}</span>
              ) : (
                <EyesIndicator eyes={eyes} />
              )}
            </p>
          </>
        ) : (
          <p className="alarm-instruction">type the phrase on your main screen.</p>
        )}
      </section>
    </main>
  );
}

function EyesIndicator({ eyes }: { eyes: Eyes }) {
  if (eyes === null) return <span className="dim">camera can't see your face. move into view.</span>;
  return eyes === "open" ? (
    <span>
      <span className="dot dot-ok" /> eyes open
    </span>
  ) : (
    <span className="bad">
      <span className="dot dot-bad" /> eyes closed. open them.
    </span>
  );
}
