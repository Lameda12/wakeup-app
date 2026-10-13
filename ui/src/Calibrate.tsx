import { useEffect, useRef, useState } from "react";
import {
  calibrationCancel,
  calibrationFinish,
  calibrationPhase,
  calibrationStart,
  chime,
  onCalibFrame,
  type CalibFrame,
  type CalibrationResult,
  type Phase,
} from "./bridge";

const PHASE_SECS = 5;
const PHASES: { phase: Phase; title: string; hint: string }[] = [
  { phase: "open", title: "look at the screen", hint: "like you're working. blink normally." },
  { phase: "closed", title: "close your eyes", hint: "keep them closed until the next chime." },
  { phase: "down", title: "look down at your phone", hint: "or at your lap, like you're scrolling." },
];

type Stage =
  | { kind: "intro" }
  | { kind: "finding" }
  | { kind: "phase"; index: number; left: number }
  | { kind: "computing" }
  | { kind: "result"; result: CalibrationResult }
  | { kind: "error"; message: string };

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

export function Calibrate({ onDone, doneLabel }: { onDone: () => void; doneLabel: string }) {
  const [stage, setStage] = useState<Stage>({ kind: "intro" });
  const [frame, setFrame] = useState<CalibFrame>({ face: false, ear: null });
  const [waitedLong, setWaitedLong] = useState(false);
  const recent = useRef<boolean[]>([]);
  const running = useRef(false);
  /** Bumped to abandon an in-flight run (cancel, unmount, restart). */
  const generation = useRef(0);

  useEffect(() => {
    const off = onCalibFrame((f) => {
      setFrame(f);
      recent.current = [...recent.current.slice(-9), f.face];
    });
    return () => {
      off();
      cancel();
    };
  }, []);

  function cancel() {
    generation.current++;
    if (running.current) calibrationCancel();
    running.current = false;
  }

  async function run() {
    const gen = ++generation.current;
    const live = () => gen === generation.current;
    const started = await calibrationStart();
    if (!started.ok) return setStage({ kind: "error", message: started.error });
    running.current = true;
    setStage({ kind: "finding" });
    setWaitedLong(false);

    // Wait until the face is seen steadily before starting the clock.
    const t0 = Date.now();
    recent.current = [];
    while (recent.current.filter(Boolean).length < 6) {
      if (!live()) return;
      if (Date.now() - t0 > 8000) setWaitedLong(true);
      await sleep(150);
    }

    for (let index = 0; index < PHASES.length; index++) {
      if (!live()) return;
      await calibrationPhase(PHASES[index].phase);
      chime();
      for (let left = PHASE_SECS; left > 0; left--) {
        if (!live()) return;
        setStage({ kind: "phase", index, left });
        await sleep(1000);
      }
    }
    if (!live()) return;
    chime();
    setStage({ kind: "computing" });
    const done = await calibrationFinish();
    running.current = false;
    setStage(done.ok ? { kind: "result", result: done.value } : { kind: "error", message: done.error });
  }

  switch (stage.kind) {
    case "intro":
      return (
        <section className="panel-step">
          <h2>calibrate to your face</h2>
          <p className="lede">
            about 20 seconds. wakeup measures your eyes open, closed, and your head looking down, so the
            alarm fits you instead of an average face.
          </p>
          <p className="fine">sit how you normally work. turn the sound on: one step has your eyes closed.</p>
          <div className="actions">
            <button className="btn btn-primary" onClick={run}>
              start
            </button>
          </div>
        </section>
      );

    case "finding":
      return (
        <section className="panel-step">
          <h2>looking for your face…</h2>
          <FaceMeter frame={frame} />
          {waitedLong && (
            <p className="fine bad">
              still nothing. check that wakeup can use the camera: System Settings → Privacy &amp; Security →
              Camera. then try again.
            </p>
          )}
          <div className="actions">
            <button
              className="btn"
              onClick={() => {
                cancel();
                setStage({ kind: "intro" });
              }}
            >
              cancel
            </button>
          </div>
        </section>
      );

    case "phase": {
      const p = PHASES[stage.index];
      return (
        <section className="panel-step">
          <p className="step-count">
            {stage.index + 1} / {PHASES.length}
          </p>
          <h2 className="phase-title">{p.title}</h2>
          <p className="lede">{p.hint}</p>
          <p className="countdown">{stage.left}</p>
          <FaceMeter frame={frame} />
        </section>
      );
    }

    case "computing":
      return (
        <section className="panel-step">
          <h2>crunching…</h2>
        </section>
      );

    case "error":
      return (
        <section className="panel-step">
          <h2 className="bad">that didn't work</h2>
          <p className="lede">{stage.message}</p>
          <div className="actions">
            <button className="btn btn-primary" onClick={run}>
              try again
            </button>
          </div>
        </section>
      );

    case "result":
      return (
        <section className="panel-step">
          <h2>calibrated.</h2>
          <EarScale result={stage.result} />
          <p className="fine">
            {stage.result.pitch_down === null
              ? "head-down detection is off for now."
              : "head-down detection is on."}
          </p>
          {stage.result.warnings.map((w) => (
            <p key={w} className="fine warn">
              {w}
            </p>
          ))}
          <div className="actions">
            <button className="btn" onClick={run}>
              redo
            </button>
            <button className="btn btn-primary" onClick={onDone}>
              {doneLabel}
            </button>
          </div>
        </section>
      );
  }
}

function FaceMeter({ frame }: { frame: CalibFrame }) {
  const pct = frame.ear === null ? 0 : Math.min(100, (frame.ear / 0.4) * 100);
  return (
    <div className="face-meter">
      <span className={frame.face ? "" : "dim"}>
        <span className={`dot ${frame.face ? "dot-ok" : "dot-off"}`} /> {frame.face ? "face found" : "no face"}
      </span>
      <div className="meter" aria-label="eye openness">
        <div className="meter-fill" style={{ width: `${pct}%` }} />
      </div>
      <span className="dim">eyes</span>
    </div>
  );
}

/** Open vs closed readings on one axis, with the alarm line between them. */
function EarScale({ result }: { result: CalibrationResult }) {
  const x = (v: number) => `${Math.min(100, Math.max(0, (v / 0.4) * 100))}%`;
  return (
    <figure className="ear-scale">
      <div className="ear-track">
        <div className="ear-zone" style={{ width: x(result.ear_closed) }} />
        <span className="ear-mark ear-shut" style={{ left: x(result.ear_shut_typical) }} />
        <span className="ear-line" style={{ left: x(result.ear_closed) }} />
        <span className="ear-mark ear-open" style={{ left: x(result.ear_open_typical) }} />
      </div>
      <figcaption className="ear-legend">
        <span>
          <span className="dot dot-bad" /> closed {result.ear_shut_typical.toFixed(2)}
        </span>
        <span>alarm below {result.ear_closed.toFixed(2)}</span>
        <span>
          <span className="dot dot-ok" /> open {result.ear_open_typical.toFixed(2)}
        </span>
      </figcaption>
    </figure>
  );
}
