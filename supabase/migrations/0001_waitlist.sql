create table if not exists public.waitlist (
  id uuid primary key default gen_random_uuid(),
  email text unique not null,
  source text,
  created_at timestamptz not null default now()
);

-- RLS on with no policies: anon/authenticated keys can't read or write.
-- The server inserts with the service role key, which bypasses RLS.
alter table public.waitlist enable row level security;
