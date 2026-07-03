-- Data repair: clear local filesystem paths that clients before v0.1.9
-- mistakenly pushed to the cloud (leaked local usernames/directory layout
-- and were never valid on other devices). Idempotent.

UPDATE public.items SET blob_path = NULL WHERE blob_path IS NOT NULL;
