"use client";

import { useActionState, useEffect, useState } from "react";
import { joinWaitlist, type JoinResult } from "./actions";
import { cleanSource } from "@/lib/source";

export function WaitlistForm({ source }: { source: string | null }) {
  const [state, action, pending] = useActionState<JoinResult | null, FormData>(
    (_prev, formData) => joinWaitlist(formData),
    null,
  );
  // No ?ref=: fall back to the referring site (t.co, reddit.com, ...).
  const [resolvedSource, setResolvedSource] = useState(source ?? "");
  useEffect(() => {
    if (source || !document.referrer) return;
    try {
      const host = new URL(document.referrer).hostname.replace(/^www\./, "");
      if (host !== location.hostname) setResolvedSource(cleanSource(host) ?? "");
    } catch {}
  }, [source]);

  const done = state?.ok === true;

  return (
    <form action={action} className="w-full">
      <div className="flex flex-col gap-3 sm:flex-row">
        <label htmlFor="email" className="sr-only">
          email
        </label>
        <div className="flex flex-1 items-center border border-neutral-800 bg-neutral-950 px-3 transition-colors focus-within:border-alarm">
          <span className="text-alarm select-none" aria-hidden>
            &gt;
          </span>
          <input
            id="email"
            name="email"
            type="email"
            required
            autoComplete="email"
            inputMode="email"
            placeholder="you@domain.com"
            disabled={done}
            className="w-full bg-transparent px-2 py-3 text-base text-neutral-100 placeholder:text-neutral-600 focus:outline-none disabled:opacity-60 sm:text-sm"
          />
        </div>

        {/* honeypot */}
        <div aria-hidden className="absolute -left-[9999px] h-0 w-0 overflow-hidden">
          <label htmlFor="company">company</label>
          <input id="company" name="company" type="text" tabIndex={-1} autoComplete="off" />
        </div>
        <input type="hidden" name="source" value={resolvedSource} />

        <button
          type="submit"
          disabled={pending || done}
          className={`border border-alarm px-5 py-3 text-sm font-bold whitespace-nowrap text-alarm transition-colors enabled:hover:bg-alarm enabled:hover:text-ink focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-alarm disabled:cursor-default ${done ? "pulse-once" : ""} ${pending ? "opacity-60" : ""}`}
        >
          {pending ? "joining..." : done ? "you're in" : "join the waitlist"}
        </button>
      </div>

      <p aria-live="polite" role="status" className="mt-3 min-h-5 text-sm">
        {state?.ok === true && (
          <span className="text-neutral-100">
            you&apos;re #{state.position.toLocaleString("en-US")}. i&apos;ll email you when it ships.
          </span>
        )}
        {state?.ok === false && <span className="text-alarm">{state.error}</span>}
      </p>
    </form>
  );
}
