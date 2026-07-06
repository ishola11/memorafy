-- Definitive device registration fix for Memorafy.
-- Run this ENTIRE file once in Supabase → SQL Editor.
-- Requires Memorafy v0.1.6+ (app calls register_device RPC, not direct table insert).

-- ── Resolve user id from JWT (auth.uid() is sometimes NULL on PostgREST upserts) ──
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

-- ── Drop every policy on devices (removes stale "devices_own" etc.) ──
DO $$
DECLARE
  pol record;
BEGIN
  FOR pol IN
    SELECT policyname
    FROM pg_policies
    WHERE schemaname = 'public' AND tablename = 'devices'
  LOOP
    EXECUTE format('DROP POLICY IF EXISTS %I ON public.devices', pol.policyname);
  END LOOP;
END $$;

-- ── RPC: register / refresh this device (bypasses RLS via SECURITY DEFINER) ──
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

-- Direct table writes blocked — app must use RPC (avoids all RLS upsert bugs).
REVOKE INSERT, UPDATE, DELETE ON public.devices FROM anon, authenticated;

-- Reads still use RLS.
CREATE POLICY devices_select_own ON public.devices
  FOR SELECT
  USING (public.current_user_id() = user_id);

GRANT SELECT ON public.devices TO anon, authenticated;
