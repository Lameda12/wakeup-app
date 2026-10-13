import { useState } from "react";
import { Calibrate } from "./Calibrate";
import { finishOnboarding, hideWindow, testAlarm } from "./bridge";

type Step = "welcome" | "calibrate" | "finish";

/** First-run setup, or just the calibration part when `mode` is "calibrate". */
export function Onboarding({ mode }: { mode: "onboarding" | "calibrate" }) {
  const [step, setStep] = useState<Step>(mode === "calibrate" ? "calibrate" : "welcome");
  const [launchAtLogin, setLaunchAtLogin] = useState(true);
  const [error, setError] = useState<string | null>(null);

  async function finish() {
    const r = await finishOnboarding(launchAtLogin);
    if (!r.ok) setError(r.error);
  }

  return (
    <main className="panel">
      <header className="panel-head">
        <span className="brand">~/wakeup</span>
        {mode === "onboarding" && (
          <span className="dim">
            {["welcome", "calibrate", "finish"].indexOf(step) + 1} / 3
          </span>
        )}
      </header>

      {step === "welcome" && (
        <section className="panel-step">
          <h1>your mac won't let you fall asleep on it anymore.</h1>
          <ol className="how">
            <li>
              eyes closed for 2.5s <span className="bad">-&gt; alarm</span>
            </li>
            <li>
              head down at your phone <span className="bad">-&gt; alarm</span>
            </li>
            <li>
              no typing for 90s <span className="bad">-&gt; "still here?"</span>, then alarm
            </li>
          </ol>
          <p className="fine">
            the camera only turns on when you stop typing. video never leaves this mac and is never saved.
          </p>
          <div className="actions">
            <button className="btn btn-primary" onClick={() => setStep("calibrate")}>
              set it up
            </button>
          </div>
        </section>
      )}

      {step === "calibrate" && (
        <Calibrate
          doneLabel={mode === "calibrate" ? "done" : "continue"}
          onDone={() => (mode === "calibrate" ? hideWindow() : setStep("finish"))}
        />
      )}

      {step === "finish" && (
        <section className="panel-step">
          <h2>you're set.</h2>
          <p className="lede">
            wakeup lives in the menu bar (the eye). pause it there, or open settings to change timings and active
            hours.
          </p>
          <label className="check">
            <input type="checkbox" checked={launchAtLogin} onChange={(e) => setLaunchAtLogin(e.target.checked)} />
            open wakeup when I log in
          </label>
          <p className="fine">want to see what you're signing up for? the test alarm is loud.</p>
          {error && <p className="fine bad">{error}</p>}
          <div className="actions">
            <button className="btn" onClick={() => testAlarm()}>
              test the alarm
            </button>
            <button className="btn btn-primary" onClick={finish}>
              start watching
            </button>
          </div>
        </section>
      )}
    </main>
  );
}
