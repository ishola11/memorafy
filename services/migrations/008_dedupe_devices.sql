-- Fix duplicate device rows.
--
-- Every "Repair sync" in clients before v0.1.9 rotated the local device id,
-- registering a brand-new device row each time and orphaning the old one —
-- users ended up with the same machine listed five times. This migration
-- makes register_device self-healing: when a device registers, stale rows
-- for the same user + name + platform are merged into it (their items are
-- reattributed first, so no history is lost).
--
-- Safety guard: rows seen in the last 10 minutes are never pruned, so two
-- genuinely active machines that happen to share a hostname cannot delete
-- each other (both ping presence every 30 seconds).
--
-- Run once in Supabase → SQL Editor. Safe to re-run.

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

  -- Merge stale duplicates of this machine into the row just registered:
  -- reattribute their items (FK: items.source_device_id), then remove them.
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

REVOKE ALL ON FUNCTION public.register_device(uuid, text, text) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION public.register_device(uuid, text, text) TO anon, authenticated;
