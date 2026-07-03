# Legacy manual migrations

**⚠️ These scripts are superseded.** The canonical, CLI-managed schema now
lives in [`services/supabase/migrations/`](../supabase/migrations/) and is
deployed automatically by the Supabase GitHub integration (branching) —
schema changes ship as new timestamped files in that directory, applied to
preview branches on PRs and to production on merge.

The files here are kept only for:

- **Existing self-hosted projects** that were set up by pasting these into
  the SQL editor and haven't adopted the CLI flow.
- **History/reference** — the baseline migration was consolidated from
  001–008.

Do not add new files here.
