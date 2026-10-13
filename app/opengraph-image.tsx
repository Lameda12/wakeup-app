import { readFile } from "node:fs/promises";
import { join } from "node:path";
import { ImageResponse } from "next/og";
import { site } from "@/lib/site";

export const alt = site.headline;
export const size = { width: 1200, height: 630 };
export const contentType = "image/png";

export default async function OpengraphImage() {
  const font = await readFile(join(process.cwd(), "assets/JetBrainsMono-Bold.woff"));

  return new ImageResponse(
    (
      <div
        style={{
          width: "100%",
          height: "100%",
          display: "flex",
          flexDirection: "column",
          justifyContent: "space-between",
          background: "#000",
          padding: "72px 80px",
          fontFamily: "JetBrains Mono",
        }}
      >
        <div style={{ display: "flex", fontSize: 26, color: "#525252" }}>~/wakeup</div>
        <div style={{ display: "flex", fontSize: 68, lineHeight: 1.15, color: "#ff3b30", letterSpacing: -1 }}>
          {site.headline}
        </div>
        <div style={{ display: "flex", fontSize: 26, color: "#737373" }}>macOS · waitlist open</div>
      </div>
    ),
    { ...size, fonts: [{ name: "JetBrains Mono", data: font, weight: 700, style: "normal" }] },
  );
}
