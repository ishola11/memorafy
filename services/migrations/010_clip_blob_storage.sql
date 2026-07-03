-- Supabase Storage bucket for synced clipboard image blobs (E2E-encrypted bytes).
-- blob_path on items stores the object key: {user_id}/{item_id}.png

INSERT INTO storage.buckets (id, name, public, file_size_limit, allowed_mime_types)
VALUES (
  'clip-blobs',
  'clip-blobs',
  false,
  10485760,
  ARRAY['image/png', 'application/octet-stream', 'text/plain']
)
ON CONFLICT (id) DO NOTHING;

DO $$
BEGIN
  IF NOT EXISTS (
    SELECT 1 FROM pg_policies
    WHERE schemaname = 'storage' AND tablename = 'objects' AND policyname = 'clip_blobs_select_own'
  ) THEN
    CREATE POLICY clip_blobs_select_own ON storage.objects
      FOR SELECT TO authenticated
      USING (bucket_id = 'clip-blobs' AND (storage.foldername(name))[1] = auth.uid()::text);
  END IF;

  IF NOT EXISTS (
    SELECT 1 FROM pg_policies
    WHERE schemaname = 'storage' AND tablename = 'objects' AND policyname = 'clip_blobs_insert_own'
  ) THEN
    CREATE POLICY clip_blobs_insert_own ON storage.objects
      FOR INSERT TO authenticated
      WITH CHECK (bucket_id = 'clip-blobs' AND (storage.foldername(name))[1] = auth.uid()::text);
  END IF;

  IF NOT EXISTS (
    SELECT 1 FROM pg_policies
    WHERE schemaname = 'storage' AND tablename = 'objects' AND policyname = 'clip_blobs_update_own'
  ) THEN
    CREATE POLICY clip_blobs_update_own ON storage.objects
      FOR UPDATE TO authenticated
      USING (bucket_id = 'clip-blobs' AND (storage.foldername(name))[1] = auth.uid()::text)
      WITH CHECK (bucket_id = 'clip-blobs' AND (storage.foldername(name))[1] = auth.uid()::text);
  END IF;

  IF NOT EXISTS (
    SELECT 1 FROM pg_policies
    WHERE schemaname = 'storage' AND tablename = 'objects' AND policyname = 'clip_blobs_delete_own'
  ) THEN
    CREATE POLICY clip_blobs_delete_own ON storage.objects
      FOR DELETE TO authenticated
      USING (bucket_id = 'clip-blobs' AND (storage.foldername(name))[1] = auth.uid()::text);
  END IF;
END $$;

-- Preserve blob_path (storage object key) on item upsert.
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
  blob_key text := NULLIF(p->>'blob_path', '');
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
    blob_key,
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
    blob_path = COALESCE(EXCLUDED.blob_path, public.items.blob_path),
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
