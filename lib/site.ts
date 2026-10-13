export const site = {
  name: "wakeup",
  title: "wakeup: stop falling asleep at your desk",
  headline: "your mac won't let you fall asleep on it anymore.",
  subhead:
    "webcam + keyboard tracking that catches you dozing, zoning out, or scrolling your phone, then gets loud until you're actually back.",
  signer: "andi",
  xHandle: process.env.NEXT_PUBLIC_X_HANDLE || "lameda12",
  stripeLink: process.env.NEXT_PUBLIC_STRIPE_LINK,
  url:
    process.env.NEXT_PUBLIC_SITE_URL ||
    (process.env.VERCEL_PROJECT_PRODUCTION_URL
      ? `https://${process.env.VERCEL_PROJECT_PRODUCTION_URL}`
      : "http://localhost:3000"),
};
