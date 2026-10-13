import type { Metadata, Viewport } from "next";
import localFont from "next/font/local";
import { Analytics } from "@vercel/analytics/next";
import { site } from "@/lib/site";
import "./globals.css";

const jetbrains = localFont({
  src: [
    { path: "./fonts/JetBrainsMono-Regular.woff2", weight: "400", style: "normal" },
    { path: "./fonts/JetBrainsMono-Bold.woff2", weight: "700", style: "normal" },
  ],
  variable: "--font-jetbrains",
  display: "swap",
});

export const metadata: Metadata = {
  metadataBase: new URL(site.url),
  title: site.title,
  description: site.subhead,
  openGraph: {
    title: site.title,
    description: site.subhead,
    siteName: site.name,
    type: "website",
    url: "/",
  },
  twitter: {
    card: "summary_large_image",
    title: site.title,
    description: site.subhead,
    creator: `@${site.xHandle}`,
  },
};

export const viewport: Viewport = {
  themeColor: "#0a0a0a",
};

export default function RootLayout({ children }: { children: React.ReactNode }) {
  return (
    <html lang="en" className={jetbrains.variable}>
      <body className="bg-ink font-mono text-neutral-200">
        {children}
        <Analytics />
      </body>
    </html>
  );
}
