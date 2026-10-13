import { useEffect, useRef, useState } from "react";
import { getSettings, openCalibration, saveSettings, testAlarm, type Settings as S } from "./bridge";

const NUDGE_OPTIONS = [0, 45, 60, 90, 120, 180, 300];
const DAYS = ["M", "T", "W", "T", "F", "S", "S"];

export function Settings() {
  const [draft, setDraft] = useState<S | null>(null);
  const [status, setStatus] = useState<{ ok: boolean; text: string } | null>(null);
  const dirty = useRef(false);

  // Reload when the window comes back, e.g. after recalibrating.
  useEffect(() => {
    const load = () =>
      getSettings().then((r) => {
        if (r.ok) {
          dirty.current = false;
          setDraft(r.value);
        }
      });
    load();
    window.addEventListener("focus", load);
    return () => window.removeEventListener("focus", load);
  }, []);

  // Autosave shortly after the last change.
  useEffect(() => {
    if (!draft || !dirty.current) return;
    const id = setTimeout(async () => {
      const r = await saveSettings(draft);
      setStatus(r.ok ? { ok: true, text: "saved" } : { ok: false, text: r.error });
    }, 300);
    return () => clearTimeout(id);
  }, [draft]);

  if (!draft) return <main className="panel" />;

  const set = (patch: Partial<S>) => {
    dirty.current = true;
    setStatus(null);
    setDraft({ ...draft, ...patch });
  };
  const setHours = (patch: Partial<S["active_hours"]>) => set({ active_hours: { ...draft.active_hours, ...patch } });
  const toggleDay = (day: number) => {
    const days = draft.active_hours.days.includes(day)
      ? draft.active_hours.days.filter((d) => d !== day)
      : [...draft.active_hours.days, day].sort();
    setHours({ days });
  };
  const headDownOn = draft.calibrated && Math.abs(draft.pitch_down - draft.pitch_neutral) > 0.01;

  return (
    <main className="panel settings">
      <header className="panel-head">
        <span className="brand">~/wakeup/settings</span>
        <span className={status && !status.ok ? "bad" : "dim"} role="status">
          {status?.text ?? ""}
        </span>
      </header>

      <Section title="alarms">
        <Slider
          label="eyes closed for"
          value={draft.eyes_closed_for}
          min={1.5}
          max={5}
          step={0.5}
          unit="s"
          onChange={(v) => set({ eyes_closed_for: v })}
        />
        <Slider
          label="head down for"
          value={draft.head_down_for}
          min={1.5}
          max={6}
          step={0.5}
          unit="s"
          note={headDownOn ? undefined : "off until a calibration picks up your nod"}
          onChange={(v) => set({ head_down_for: v })}
        />
        <Row label='"still here?" after no typing' note="for reading or zoning out">
          <select value={draft.idle_nudge_after} onChange={(e) => set({ idle_nudge_after: Number(e.target.value) })}>
            {NUDGE_OPTIONS.map((s) => (
              <option key={s} value={s}>
                {s === 0 ? "never" : s < 120 ? `${s}s` : `${s / 60} min`}
              </option>
            ))}
          </select>
        </Row>
        <Row label="alarm if the nudge is ignored for">
          <select
            value={draft.nudge_timeout}
            disabled={draft.idle_nudge_after === 0}
            onChange={(e) => set({ nudge_timeout: Number(e.target.value) })}
          >
            {[5, 10, 15, 30].map((s) => (
              <option key={s} value={s}>
                {s}s
              </option>
            ))}
          </select>
        </Row>
      </Section>

      <Section title="camera">
        <Toggle
          label="only when I stop typing"
          note="camera light stays off while you type. off = always watching."
          checked={draft.camera_gate > 0}
          onChange={(on) => set({ camera_gate: on ? 8 : 0 })}
        />
      </Section>

      <Section title="when">
        <Toggle
          label="pause during calls"
          note="while any app is using the microphone"
          checked={draft.meeting_pause}
          onChange={(on) => set({ meeting_pause: on })}
        />
        <Toggle
          label="only during active hours"
          checked={draft.active_hours_enabled}
          onChange={(on) => set({ active_hours_enabled: on })}
        />
        {draft.active_hours_enabled && (
          <div className="hours">
            <input type="time" value={draft.active_hours.start} onChange={(e) => setHours({ start: e.target.value })} />
            <span className="dim">to</span>
            <input type="time" value={draft.active_hours.end} onChange={(e) => setHours({ end: e.target.value })} />
            <div className="days" role="group" aria-label="days">
              {DAYS.map((d, i) => (
                <button
                  key={i}
                  className={`day ${draft.active_hours.days.includes(i + 1) ? "day-on" : ""}`}
                  aria-pressed={draft.active_hours.days.includes(i + 1)}
                  onClick={() => toggleDay(i + 1)}
                >
                  {d}
                </button>
              ))}
            </div>
          </div>
        )}
      </Section>

      <Section title="system">
        <Toggle label="open at login" checked={draft.launch_at_login} onChange={(on) => set({ launch_at_login: on })} />
      </Section>

      <Section title="calibration">
        <p className="fine">
          {draft.calibrated
            ? `calibrated. eyes count as closed below ${draft.ear_closed.toFixed(2)}.`
            : "not calibrated: using average-face defaults, which can misfire."}
        </p>
        <div className="actions actions-left">
          <button className="btn" onClick={() => openCalibration()}>
            {draft.calibrated ? "recalibrate" : "calibrate now"}
          </button>
          <button className="btn" onClick={() => testAlarm()}>
            test alarm
          </button>
        </div>
      </Section>

      <p className="fine footer">video is processed on this mac and never saved or sent anywhere.</p>
    </main>
  );
}

function Section({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <section className="section">
      <h3>{title}</h3>
      {children}
    </section>
  );
}

function Row({ label, note, children }: { label: string; note?: string; children: React.ReactNode }) {
  return (
    <div className="row">
      <div>
        <div>{label}</div>
        {note && <div className="fine">{note}</div>}
      </div>
      {children}
    </div>
  );
}

function Slider(props: {
  label: string;
  value: number;
  min: number;
  max: number;
  step: number;
  unit: string;
  note?: string;
  onChange: (v: number) => void;
}) {
  return (
    <Row label={props.label} note={props.note}>
      <span className="slider">
        <input
          type="range"
          min={props.min}
          max={props.max}
          step={props.step}
          value={props.value}
          aria-label={props.label}
          onChange={(e) => props.onChange(Number(e.target.value))}
        />
        <span className="slider-value">
          {props.value}
          {props.unit}
        </span>
      </span>
    </Row>
  );
}

function Toggle(props: { label: string; note?: string; checked: boolean; onChange: (on: boolean) => void }) {
  return (
    <Row label={props.label} note={props.note}>
      <button
        role="switch"
        aria-checked={props.checked}
        aria-label={props.label}
        className={`switch ${props.checked ? "switch-on" : ""}`}
        onClick={() => props.onChange(!props.checked)}
      >
        <span className="switch-knob" />
      </button>
    </Row>
  );
}
