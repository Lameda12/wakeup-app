import { site } from "@/lib/site";
import { cleanSource } from "@/lib/source";
import { getWaitlistCount } from "@/lib/supabase";
import { WaitlistForm } from "./waitlist-form";

export const dynamic = "force-dynamic";

const steps = [
  { trigger: "eyes close for 2.5s" },
  { trigger: "head down at your phone" },
  { trigger: "no typing for 90s", note: "dismiss only by typing a phrase with your eyes open." },
];

export default async function Home({
  searchParams,
}: {
  searchParams: Promise<{ ref?: string | string[] }>;
}) {
  const { ref } = await searchParams;
  const source = cleanSource(Array.isArray(ref) ? ref[0] : ref);
  const count = await getWaitlistCount();

  return (
    <main className="mx-auto flex min-h-dvh max-w-2xl flex-col px-5 py-8 sm:px-8 sm:py-12">
      <p className="text-xs text-neutral-500">~/wakeup</p>

      <section className="flex flex-1 flex-col justify-center py-10">
        <h1 className="text-[1.75rem] leading-[1.15] font-bold tracking-tight text-neutral-50 sm:text-5xl">
          {site.headline}
          <span className="cursor ml-1 inline-block h-[0.9em] w-[0.5em] translate-y-[0.12em] bg-alarm" aria-hidden />
        </h1>

        <p className="mt-5 max-w-xl text-sm leading-relaxed text-neutral-400 sm:text-base">{site.subhead}</p>

        <div className="mt-8">
          <WaitlistForm source={source} />

          {site.stripeLink && (
            <a
              href={site.stripeLink}
              className="mt-4 inline-block text-sm text-neutral-400 underline decoration-neutral-700 underline-offset-4 transition-colors hover:text-alarm hover:decoration-alarm"
            >
              or lock in early access for $9
            </a>
          )}

          {!!count && (
            <p className="mt-6 flex items-center gap-2 text-sm text-neutral-400">
              <span className="relative flex size-2" aria-hidden>
                <span className="absolute inline-flex size-full animate-ping rounded-full bg-alarm opacity-60 motion-reduce:animate-none" />
                <span className="relative inline-flex size-2 rounded-full bg-alarm" />
              </span>
              <span>
                <span className="text-neutral-100 tabular-nums">{count.toLocaleString("en-US")}</span>{" "}
                {count === 1 ? "person can't" : "people can't"} stay awake either
              </span>
            </p>
          )}
        </div>

        <ol className="mt-12 space-y-3 border-t border-neutral-900 pt-6 text-sm">
          {steps.map((step, i) => (
            <li key={step.trigger} className="grid grid-cols-[2rem_1fr] gap-x-2">
              <span className="text-neutral-600">{String(i + 1).padStart(2, "0")}</span>
              <span className="text-neutral-300">
                {step.trigger} <span className="text-alarm">-&gt; alarm</span>
                {step.note && <span className="mt-1 block text-neutral-500">{step.note}</span>}
              </span>
            </li>
          ))}
        </ol>
      </section>

      <footer className="text-xs text-neutral-500">
        built in public by{" "}
        <a
          href={`https://x.com/${site.xHandle}`}
          target="_blank"
          rel="noopener noreferrer"
          className="text-neutral-300 transition-colors hover:text-alarm"
        >
          @{site.xHandle}
        </a>
      </footer>
    </main>
  );
}
