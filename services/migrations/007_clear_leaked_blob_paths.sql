-- Clear any local filesystem paths pushed to the cloud before Memorafy
-- stopped syncing blob_path (leaked local usernames/directory layout and
-- was never valid on other devices anyway). Safe to re-run.
-- Run once in Supabase → SQL Editor.

UPDATE public.items SET blob_path = NULL WHERE blob_path IS NOT NULL;
