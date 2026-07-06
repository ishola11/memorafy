-- Memorafy cloud schema baseline.
--
-- Consolidates the manually-run scripts in services/migrations/ (001–008)
-- into a single CLI-managed migration for Supabase branching. Written
-- idempotently: applying it to the existing production project (which
-- already has this schema) is a no-op, while fresh preview branches get
-- the complete schema. New schema changes go in new timestamped files in
-- this directory — never edit this one.

-- ── Tables ──────────────────────────────────────────────────────────────

CREATE TABLE IF NOT EXISTS public.devices (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  user_id UUID NOT NULL REFERENCES auth.users(id) ON DELETE CASCADE,
  name TEXT NOT NULL,
  platform TEXT NOT NULL CHECK (platform IN ('macos', 'windows')),
  device_key_pub BYTEA,
  last_seen_at TIMESTAMPTZ,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  revoked_at TIMESTAMPTZ
);

CREATE TABLE IF NOT EXISTS public.items (
  id UUID PRIMARY KEY,
  user_id UUID NOT NULL REFERENCES auth.users(id) ON DELETE CASCADE,
  kind TEXT NOT NULL DEFAULT 'history' CHECK (kind IN ('history', 'snippet')),
  content_type TEXT NOT NULL,
  display_title TEXT,
  preview_text TEXT,
  char_count INT,
  url TEXT,
  url_title TEXT,
  url_domain TEXT,
  code_language TEXT,
  line_count INT,
  blob_path TEXT,
  blob_size BIGINT,
  content_hash TEXT NOT NULL,
  plain_text TEXT,
  trigger TEXT,
  source_device_id UUID REFERENCES public.devices(id),
  is_pinned BOOLEAN NOT NULL DEFAULT false,
  is_favorited BOOLEAN NOT NULL DEFAULT false,
  encrypted BOOLEAN NOT NULL DEFAULT false,
  created_at TIMESTAMPTZ NOT NULL,
  updated_at TIMESTAMPTZ NOT NULL,
  deleted_at TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS idx_items_user_created ON public.items(user_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_items_user_pinned ON public.items(user_id) WHERE is_pinned;
-- Incremental pull filters on updated_at (client sync engine).
CREATE INDEX IF NOT EXISTS idx_items_user_updated ON public.items(user_id, updated_at);

CREATE TABLE IF NOT EXISTS public.collections (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  user_id UUID NOT NULL REFERENCES auth.users(id) ON DELETE CASCADE,
  name TEXT NOT NULL,
  color TEXT NOT NULL DEFAULT '#6366f1',
  icon TEXT,
  sort_order INT NOT NULL DEFAULT 0,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS public.item_collections (
  item_id UUID NOT NULL REFERENCES public.items(id) ON DELETE CASCADE,
  collection_id UUID NOT NULL REFERENCES public.collections(id) ON DELETE CASCADE,
  PRIMARY KEY (item_id, collection_id)
);

CREATE TABLE IF NOT EXISTS public.sync_events (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  user_id UUID NOT NULL REFERENCES auth.users(id) ON DELETE CASCADE,
  device_id UUID NOT NULL REFERENCES public.devices(id),
  entity_type TEXT NOT NULL,
  entity_id UUID NOT NULL,
  op TEXT NOT NULL,
  payload JSONB,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- ── Defaults & grants ───────────────────────────────────────────────────

ALTER TABLE public.devices ALTER COLUMN user_id SET DEFAULT auth.uid();
ALTER TABLE public.items ALTER COLUMN user_id SET DEFAULT auth.uid();
ALTER TABLE public.collections ALTER COLUMN user_id SET DEFAULT auth.uid();
ALTER TABLE public.sync_events ALTER COLUMN user_id SET DEFAULT auth.uid();

GRANT USAGE ON SCHEMA public TO anon, authenticated;
GRANT SELECT, INSERT, UPDATE, DELETE ON public.items TO authenticated;
GRANT SELECT, INSERT, UPDATE, DELETE ON public.collections TO authenticated;
GRANT SELECT, INSERT, UPDATE, DELETE ON public.item_collections TO authenticated;
GRANT SELECT, INSERT, UPDATE, DELETE ON public.sync_events TO authenticated;
-- Devices are written only through the register_device/touch_device RPCs
-- (SECURITY DEFINER) — direct writes stay revoked.
GRANT SELECT ON public.devices TO anon, authenticated;
REVOKE INSERT, UPDATE, DELETE ON public.devices FROM anon, authenticated;

-- ── Row Level Security ──────────────────────────────────────────────────

ALTER TABLE public.devices ENABLE ROW LEVEL SECURITY;
ALTER TABLE public.items ENABLE ROW LEVEL SECURITY;
ALTER TABLE public.collections ENABLE ROW LEVEL SECURITY;
ALTER TABLE public.item_collections ENABLE ROW LEVEL SECURITY;
ALTER TABLE public.sync_events ENABLE ROW LEVEL SECURITY;

-- Resolve the user id from the JWT; auth.uid() alone is sometimes NULL on
-- PostgREST upserts.
CREATE OR REPLACE FUNCTION public.current_user_id()
RETURNS uuid
LANGUAGE sql
STABLE
SET search_path = public
AS $$
  SELECT coalesce(
    auth.uid(),
    NULLIF(current_setting('request.jwt.claim.sub', true), '')::uuid,
    NULLIF((auth.jwt() ->> 'sub'), '')::uuid
  );
$$;

REVOKE ALL ON FUNCTION public.current_user_id() FROM PUBLIC;
GRANT EXECUTE ON FUNCTION public.current_user_id() TO anon, authenticated;

-- Recreate policies from scratch (DROP + CREATE is the idempotent form).
DROP POLICY IF EXISTS devices_own ON public.devices;
DROP POLICY IF EXISTS devices_select_own ON public.devices;
DROP POLICY IF EXISTS devices_insert_own ON public.devices;
DROP POLICY IF EXISTS devices_update_own ON public.devices;
DROP POLICY IF EXISTS devices_delete_own ON public.devices;
CREATE POLICY devices_select_own ON public.devices
  FOR SELECT USING (public.current_user_id() = user_id);

DROP POLICY IF EXISTS items_own ON public.items;
DROP POLICY IF EXISTS items_select_own ON public.items;
DROP POLICY IF EXISTS items_insert_own ON public.items;
DROP POLICY IF EXISTS items_update_own ON public.items;
DROP POLICY IF EXISTS items_delete_own ON public.items;
CREATE POLICY items_select_own ON public.items
  FOR SELECT TO authenticated USING (auth.uid() = user_id);
CREATE POLICY items_insert_own ON public.items
  FOR INSERT TO authenticated WITH CHECK (auth.uid() = user_id);
CREATE POLICY items_update_own ON public.items
  FOR UPDATE TO authenticated USING (auth.uid() = user_id) WITH CHECK (auth.uid() = user_id);
CREATE POLICY items_delete_own ON public.items
  FOR DELETE TO authenticated USING (auth.uid() = user_id);

DROP POLICY IF EXISTS collections_own ON public.collections;
DROP POLICY IF EXISTS collections_select_own ON public.collections;
DROP POLICY IF EXISTS collections_insert_own ON public.collections;
DROP POLICY IF EXISTS collections_update_own ON public.collections;
DROP POLICY IF EXISTS collections_delete_own ON public.collections;
CREATE POLICY collections_select_own ON public.collections
  FOR SELECT TO authenticated USING (auth.uid() = user_id);
CREATE POLICY collections_insert_own ON public.collections
  FOR INSERT TO authenticated WITH CHECK (auth.uid() = user_id);
CREATE POLICY collections_update_own ON public.collections
  FOR UPDATE TO authenticated USING (auth.uid() = user_id) WITH CHECK (auth.uid() = user_id);
CREATE POLICY collections_delete_own ON public.collections
  FOR DELETE TO authenticated USING (auth.uid() = user_id);

DROP POLICY IF EXISTS item_collections_own ON public.item_collections;
DROP POLICY IF EXISTS item_collections_select_own ON public.item_collections;
DROP POLICY IF EXISTS item_collections_insert_own ON public.item_collections;
DROP POLICY IF EXISTS item_collections_delete_own ON public.item_collections;
CREATE POLICY item_collections_select_own ON public.item_collections
  FOR SELECT TO authenticated
  USING (EXISTS (SELECT 1 FROM public.items i WHERE i.id = item_id AND i.user_id = auth.uid()));
CREATE POLICY item_collections_insert_own ON public.item_collections
  FOR INSERT TO authenticated
  WITH CHECK (
    EXISTS (SELECT 1 FROM public.items i WHERE i.id = item_id AND i.user_id = auth.uid())
    AND EXISTS (SELECT 1 FROM public.collections c WHERE c.id = collection_id AND c.user_id = auth.uid())
  );
CREATE POLICY item_collections_delete_own ON public.item_collections
  FOR DELETE TO authenticated
  USING (EXISTS (SELECT 1 FROM public.items i WHERE i.id = item_id AND i.user_id = auth.uid()));

DROP POLICY IF EXISTS sync_events_own ON public.sync_events;
DROP POLICY IF EXISTS sync_events_select_own ON public.sync_events;
DROP POLICY IF EXISTS sync_events_insert_own ON public.sync_events;
CREATE POLICY sync_events_select_own ON public.sync_events
  FOR SELECT TO authenticated USING (auth.uid() = user_id);
CREATE POLICY sync_events_insert_own ON public.sync_events
  FOR INSERT TO authenticated WITH CHECK (auth.uid() = user_id);

-- ── Device registration RPCs (bypass RLS via SECURITY DEFINER) ──────────

-- Self-healing: merges stale duplicate rows for the same machine into the
-- registering device (items reattributed first). Rows active in the last
-- 10 minutes are never pruned, so two live machines sharing a hostname
-- cannot delete each other.
CREATE OR REPLACE FUNCTION public.register_device(
  device_id uuid,
  device_name text,
  device_platform text
)
RETURNS void
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = public
SET row_security = off
AS $$
DECLARE
  uid uuid := public.current_user_id();
BEGIN
  IF uid IS NULL THEN
    RAISE EXCEPTION 'Not authenticated — sign in again'
      USING ERRCODE = '42501';
  END IF;

  IF device_platform NOT IN ('macos', 'windows') THEN
    RAISE EXCEPTION 'Invalid platform: %', device_platform
      USING ERRCODE = '22023';
  END IF;

  IF EXISTS (
    SELECT 1 FROM public.devices d
    WHERE d.id = device_id AND d.user_id IS DISTINCT FROM uid
  ) THEN
    RAISE EXCEPTION
      'Device id already linked to another account. Reset app data and sign in again.'
      USING ERRCODE = '23505';
  END IF;

  INSERT INTO public.devices (id, user_id, name, platform, last_seen_at)
  VALUES (device_id, uid, device_name, device_platform, now())
  ON CONFLICT (id) DO UPDATE SET
    name = EXCLUDED.name,
    platform = EXCLUDED.platform,
    last_seen_at = now()
  WHERE public.devices.user_id = uid;

  UPDATE public.items i
  SET source_device_id = register_device.device_id
  WHERE i.user_id = uid
    AND i.source_device_id IN (
      SELECT d.id FROM public.devices d
      WHERE d.user_id = uid
        AND d.name = device_name
        AND d.platform = device_platform
        AND d.id <> register_device.device_id
        AND (d.last_seen_at IS NULL OR d.last_seen_at < now() - interval '10 minutes')
    );

  DELETE FROM public.devices d
  WHERE d.user_id = uid
    AND d.name = device_name
    AND d.platform = device_platform
    AND d.id <> register_device.device_id
    AND (d.last_seen_at IS NULL OR d.last_seen_at < now() - interval '10 minutes');
END;
$$;

CREATE OR REPLACE FUNCTION public.touch_device(device_id uuid)
RETURNS void
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = public
SET row_security = off
AS $$
DECLARE
  uid uuid := public.current_user_id();
BEGIN
  IF uid IS NULL THEN
    RETURN;
  END IF;

  UPDATE public.devices
  SET last_seen_at = now()
  WHERE id = device_id AND user_id = uid;
END;
$$;

REVOKE ALL ON FUNCTION public.register_device(uuid, text, text) FROM PUBLIC;
REVOKE ALL ON FUNCTION public.touch_device(uuid) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION public.register_device(uuid, text, text) TO anon, authenticated;
GRANT EXECUTE ON FUNCTION public.touch_device(uuid) TO anon, authenticated;

-- ── Realtime (guarded: ALTER PUBLICATION errors on duplicates) ──────────

DO $$
BEGIN
  IF NOT EXISTS (
    SELECT 1 FROM pg_publication_tables
    WHERE pubname = 'supabase_realtime' AND schemaname = 'public' AND tablename = 'items'
  ) THEN
    ALTER PUBLICATION supabase_realtime ADD TABLE public.items;
  END IF;
  IF NOT EXISTS (
    SELECT 1 FROM pg_publication_tables
    WHERE pubname = 'supabase_realtime' AND schemaname = 'public' AND tablename = 'collections'
  ) THEN
    ALTER PUBLICATION supabase_realtime ADD TABLE public.collections;
  END IF;
  IF NOT EXISTS (
    SELECT 1 FROM pg_publication_tables
    WHERE pubname = 'supabase_realtime' AND schemaname = 'public' AND tablename = 'item_collections'
  ) THEN
    ALTER PUBLICATION supabase_realtime ADD TABLE public.item_collections;
  END IF;
END $$;
