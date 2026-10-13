"use server";

import { revalidatePath } from "next/cache";
import { Resend } from "resend";
import { z } from "zod";
import { site } from "@/lib/site";
import { cleanSource } from "@/lib/source";
import { getWaitlistCount, supabaseAdmin } from "@/lib/supabase";

export type JoinResult = { ok: true; position: number } | { ok: false; error: string };

const emailSchema = z.string().trim().toLowerCase().max(254).pipe(z.email());

const UNIQUE_VIOLATION = "23505";

export async function joinWaitlist(formData: FormData): Promise<JoinResult> {
  // Honeypot: real users never see this field. Pretend it worked so bots move on.
  const trap = formData.get("company");
  if (typeof trap === "string" && trap.length > 0) {
    return { ok: true, position: (await getWaitlistCount()) ?? 1 };
  }

  const parsed = emailSchema.safeParse(formData.get("email"));
  if (!parsed.success) return { ok: false, error: "that doesn't look like an email." };
  const email = parsed.data;
  const source = cleanSource(formData.get("source"));

  let isNew = true;
  try {
    const { error } = await supabaseAdmin().from("waitlist").insert({ email, source });
    if (error) {
      if (error.code !== UNIQUE_VIOLATION) throw error;
      isNew = false; // already on the list: same response, no leak
    }
  } catch (err) {
    console.error("[waitlist] insert failed", err);
    return { ok: false, error: "something broke on my end. try again in a sec." };
  }

  const position = (await getWaitlistCount()) ?? 1;

  if (isNew) {
    await sendConfirmation(email, position);
    revalidatePath("/");
  }

  return { ok: true, position };
}

async function sendConfirmation(to: string, position: number) {
  const apiKey = process.env.RESEND_API_KEY;
  const from = process.env.RESEND_FROM;
  if (!apiKey || !from) {
    console.error("[waitlist] RESEND_API_KEY / RESEND_FROM not set, skipping email");
    return;
  }
  try {
    const { error } = await new Resend(apiKey).emails.send({
      from,
      to,
      subject: "you're on the wakeup list",
      text: [
        `you're #${position} on the wakeup list.`,
        "i'll email you once when the mac app ships, nothing else.",
        "reply and tell me when you usually crash at your desk, it helps me tune the alarms.",
        "",
        `- ${site.signer}`,
      ].join("\n"),
    });
    if (error) throw error;
  } catch (err) {
    // Signup already landed; a failed email shouldn't fail the request.
    console.error("[waitlist] email failed", err);
  }
}
