# wakeup waitlist

Single-page waitlist for wakeup. Next.js 15 (App Router) + Tailwind v4 + Supabase + Resend, deployed on Vercel.

## Setup

1. **Supabase**: run `supabase/migrations/0001_waitlist.sql` in the SQL editor. RLS is on with no policies, so only the service role key (server-side) can touch the table.
2. **Resend**: verify a sending domain, then set `RESEND_FROM` to an address on it.
3. **Env vars** (Vercel project settings, see `.env.example`):
   - `SUPABASE_URL`, `SUPABASE_SERVICE_ROLE_KEY`
   - `RESEND_API_KEY`, `RESEND_FROM`
   - `NEXT_PUBLIC_STRIPE_LINK`, `NEXT_PUBLIC_X_HANDLE` (inlined at build time, redeploy after changing)
   - `NEXT_PUBLIC_SITE_URL` (optional, defaults to the Vercel production URL; used for absolute OG image URLs)
4. **Vercel Analytics**: enable it in the project's Analytics tab.

## Traffic sources

Share links with `?ref=`: `https://yoursite.com/?ref=x`, `?ref=reddit`, `?ref=discord`. Without `ref`, the referring host (e.g. `t.co`) is stored instead.

```sql
select source, count(*) from waitlist group by source order by 2 desc;
```

## Dev

```bash
npm i
cp .env.example .env.local
npm run dev
```
