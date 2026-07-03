-- End-to-end encryption key storage.
--
-- Holds each user's data key (DEK) *wrapped* by a key derived client-side
-- from their password (Argon2id). The server only ever stores this opaque
-- wrapped blob — it cannot decrypt clipboard content. See the desktop
-- app's crypto.rs for the scheme.

CREATE TABLE IF NOT EXISTS public.user_encryption_keys (
  user_id UUID PRIMARY KEY REFERENCES auth.users(id) ON DELETE CASCADE DEFAULT auth.uid(),
  wrapped_dek TEXT NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

ALTER TABLE public.user_encryption_keys ENABLE ROW LEVEL SECURITY;

GRANT SELECT, INSERT, UPDATE ON public.user_encryption_keys TO authenticated;

DROP POLICY IF EXISTS user_keys_select_own ON public.user_encryption_keys;
DROP POLICY IF EXISTS user_keys_insert_own ON public.user_encryption_keys;
DROP POLICY IF EXISTS user_keys_update_own ON public.user_encryption_keys;

CREATE POLICY user_keys_select_own ON public.user_encryption_keys
  FOR SELECT TO authenticated USING (auth.uid() = user_id);
CREATE POLICY user_keys_insert_own ON public.user_encryption_keys
  FOR INSERT TO authenticated WITH CHECK (auth.uid() = user_id);
CREATE POLICY user_keys_update_own ON public.user_encryption_keys
  FOR UPDATE TO authenticated USING (auth.uid() = user_id) WITH CHECK (auth.uid() = user_id);
