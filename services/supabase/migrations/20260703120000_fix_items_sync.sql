-- Fix item/collection sync (RLS upsert failures + FK-safe upsert RPC).
-- See services/migrations/009_fix_items_sync.sql (kept in sync manually).

DO $$
DECLARE pol record;
BEGIN
  FOR pol IN SELECT policyname FROM pg_policies WHERE schemaname = 'public' AND tablename = 'items'
  LOOP EXECUTE format('DROP POLICY IF EXISTS %I ON public.items', pol.policyname); END LOOP;
  FOR pol IN SELECT policyname FROM pg_policies WHERE schemaname = 'public' AND tablename = 'collections'
  LOOP EXECUTE format('DROP POLICY IF EXISTS %I ON public.collections', pol.policyname); END LOOP;
END $$;

ALTER TABLE public.items ALTER COLUMN user_id SET DEFAULT auth.uid();
ALTER TABLE public.collections ALTER COLUMN user_id SET DEFAULT auth.uid();

CREATE POLICY items_select_own ON public.items
  FOR SELECT USING (public.current_user_id() = user_id);
CREATE POLICY items_insert_own ON public.items
  FOR INSERT WITH CHECK (public.current_user_id() = user_id);
CREATE POLICY items_update_own ON public.items
  FOR UPDATE
  USING (public.current_user_id() = user_id)
  WITH CHECK (public.current_user_id() = user_id);
CREATE POLICY items_delete_own ON public.items
  FOR DELETE USING (public.current_user_id() = user_id);

CREATE POLICY collections_select_own ON public.collections
  FOR SELECT USING (public.current_user_id() = user_id);
CREATE POLICY collections_insert_own ON public.collections
  FOR INSERT WITH CHECK (public.current_user_id() = user_id);
CREATE POLICY collections_update_own ON public.collections
  FOR UPDATE
  USING (public.current_user_id() = user_id)
  WITH CHECK (public.current_user_id() = user_id);
CREATE POLICY collections_delete_own ON public.collections
  FOR DELETE USING (public.current_user_id() = user_id);

CREATE OR REPLACE FUNCTION public.upsert_item(p jsonb)
RETURNS void
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = public
SET row_security = off
AS $$
DECLARE
  uid uuid := public.current_user_id();
  dev_id uuid;
BEGIN
  IF uid IS NULL THEN
    RAISE EXCEPTION 'Not authenticated. Sign in again.' USING ERRCODE = '42501';
  END IF;

  dev_id := NULLIF(p->>'source_device_id', '')::uuid;
  IF dev_id IS NOT NULL AND NOT EXISTS (
    SELECT 1 FROM public.devices d WHERE d.id = dev_id AND d.user_id = uid
  ) THEN
    dev_id := NULL;
  END IF;

  INSERT INTO public.items (
    id, user_id, kind, content_type, display_title, preview_text, char_count,
    url, url_title, url_domain, code_language, line_count,
    blob_path, blob_size, content_hash, plain_text, trigger, source_device_id,
    is_pinned, is_favorited, encrypted, created_at, updated_at, deleted_at
  ) VALUES (
    (p->>'id')::uuid,
    uid,
    COALESCE(p->>'kind', 'history'),
    p->>'content_type',
    p->>'display_title',
    p->>'preview_text',
    NULLIF(p->>'char_count', '')::int,
    p->>'url',
    p->>'url_title',
    p->>'url_domain',
    p->>'code_language',
    NULLIF(p->>'line_count', '')::int,
    NULL,
    NULLIF(p->>'blob_size', '')::bigint,
    p->>'content_hash',
    p->>'plain_text',
    p->>'trigger',
    dev_id,
    COALESCE((p->>'is_pinned')::boolean, false),
    COALESCE((p->>'is_favorited')::boolean, false),
    COALESCE((p->>'encrypted')::boolean, false),
    (p->>'created_at')::timestamptz,
    (p->>'updated_at')::timestamptz,
    NULLIF(p->>'deleted_at', '')::timestamptz
  )
  ON CONFLICT (id) DO UPDATE SET
    kind = EXCLUDED.kind,
    content_type = EXCLUDED.content_type,
    display_title = EXCLUDED.display_title,
    preview_text = EXCLUDED.preview_text,
    char_count = EXCLUDED.char_count,
    url = EXCLUDED.url,
    url_title = EXCLUDED.url_title,
    url_domain = EXCLUDED.url_domain,
    code_language = EXCLUDED.code_language,
    line_count = EXCLUDED.line_count,
    blob_path = NULL,
    blob_size = EXCLUDED.blob_size,
    content_hash = EXCLUDED.content_hash,
    plain_text = EXCLUDED.plain_text,
    trigger = EXCLUDED.trigger,
    source_device_id = EXCLUDED.source_device_id,
    is_pinned = EXCLUDED.is_pinned,
    is_favorited = EXCLUDED.is_favorited,
    encrypted = EXCLUDED.encrypted,
    updated_at = EXCLUDED.updated_at,
    deleted_at = EXCLUDED.deleted_at
  WHERE public.items.user_id = uid;
END;
$$;

REVOKE ALL ON FUNCTION public.upsert_item(jsonb) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION public.upsert_item(jsonb) TO anon, authenticated;
