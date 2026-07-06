export type ContentType = 'text' | 'url' | 'code' | 'image' | 'richtext' | 'snippet';
export type ItemKind = 'history' | 'snippet';
export type Platform = 'macos' | 'windows';
export type SyncStatus = 'pending' | 'synced' | 'failed';
export type AppTab = 'history' | 'pinned' | 'favorites' | 'collections' | 'snippets';

export interface ClipItem {
  id: string;
  kind: ItemKind;
  contentType: ContentType;
  displayTitle: string | null;
  previewText: string | null;
  charCount: number | null;
  url: string | null;
  urlTitle: string | null;
  urlDomain: string | null;
  codeLanguage: string | null;
  lineCount: number | null;
  blobPath: string | null;
  blobSize: number | null;
  thumbnailPath: string | null;
  plainText: string | null;
  trigger: string | null;
  sourceDeviceId: string | null;
  sourceDeviceName: string | null;
  isPinned: boolean;
  isFavorited: boolean;
  syncStatus: SyncStatus;
  createdAt: string;
  updatedAt: string;
}

export interface PreviewCard {
  id: string;
  kind: ContentType | 'snippet';
  title: string;
  subtitle?: string;
  meta: string;
  thumbnail?: string;
  badges: Array<'pinned' | 'favorite' | 'snippet'>;
  timelineBucket?: TimelineBucket;
  isPinned: boolean;
  isFavorited: boolean;
}

export type TimelineBucket =
  | 'now'
  | 'today'
  | 'yesterday'
  | 'last_7_days'
  | 'earlier'
  | 'pinned'
  | 'snippets';

export interface TimelineSection {
  bucket: TimelineBucket;
  label: string;
  items: PreviewCard[];
}

export interface SearchFilters {
  query: string;
  device?: Platform;
  contentType?: ContentType;
  tag?: string;
  collection?: string;
  isPinned?: boolean;
  isFavorite?: boolean;
  isSnippet?: boolean;
  dateToday?: boolean;
  inCollection?: boolean;
}

export interface Collection {
  id: string;
  name: string;
  color: string;
  icon: string | null;
  itemCount: number;
}

export interface DeviceInfo {
  id: string;
  name: string;
  platform: Platform;
  lastSeenAt: string | null;
  isCurrent: boolean;
  isOnline: boolean;
}

export interface SyncQueueItem {
  id: string;
  op: 'create' | 'update' | 'delete';
  entityType: 'item' | 'collection' | 'tag';
  entityId: string;
  status: SyncStatus;
  retryCount: number;
}

/**
 * End-to-end encryption key state: 'off' while signed out or unconfigured,
 * 'ready' when clips sync encrypted, 'locked' when the key is unavailable
 * (e.g. after a password reset) and sync decryption is paused.
 */
export type E2eStatus = 'off' | 'ready' | 'locked';

export interface SyncState {
  configured: boolean;
  loggedIn: boolean;
  userEmail: string | null;
  pendingCount: number;
  lastSyncAt: string | null;
  cloudDeviceCount: number;
  e2eStatus: E2eStatus;
}

export interface SignUpResult extends SyncState {
  /** Account created but the emailed confirmation link must be clicked first. */
  needsEmailConfirmation: boolean;
}

/** Result of opening a Supabase email link in the Memorafy desktop app. */
export interface AuthCallbackResult extends SyncState {
  callbackType: string;
  needsNewPassword: boolean;
}

export interface SyncActionResult extends SyncState {
  message: string;
  pendingBefore: number;
  pendingAfter: number;
}

export interface SyncRepairResult extends SyncActionResult {
  queueCleared: number;
  deviceRotated: boolean;
}

export interface SyncTransfer {
  itemId: string;
  title: string;
  sourceDevice: string;
  onlineDevices: string[];
}

export type ThemePreference = 'system' | 'light' | 'dark';

export interface AppSettings {
  historyRetentionDays: number;
  clipboardPaused: boolean;
  themePreference: ThemePreference;
  launchAtLogin: boolean;
}

export type HistoryRetentionOption = 0 | 30 | 60 | 90;

export type ClearHistoryScope = 'local' | 'everywhere';
export type ClearHistoryMode = 'expired' | 'all';

export interface ClearHistoryPreview {
  expiredCount: number;
  allCount: number;
  retentionDays: number;
}

export interface ClearHistoryResult {
  cleared: number;
  scope: ClearHistoryScope;
  mode: ClearHistoryMode;
}

export interface CreateCollectionInput {
  name: string;
  color: string;
}

export interface UpdateCollectionInput {
  id: string;
  name?: string;
  color?: string;
}

export interface CreateSnippetInput {
  title: string;
  text: string;
  trigger?: string | null;
}

export interface UpdateSnippetInput {
  id: string;
  title: string;
  text: string;
  trigger?: string | null;
}

export interface Diagnostics {
  appVersion: string;
  os: string;
  arch: string;
  syncConfigured: boolean;
  loggedIn: boolean;
  pendingCount: number;
  deviceId: string | null;
  accountId: string | null;
  recentLogs: string | null;
}

export type FeedbackKind = 'bug' | 'feature';

export interface FeedbackSection {
  label: string;
  value: string;
}

export interface FeedbackReport {
  kind: FeedbackKind;
  title: string;
  sections: FeedbackSection[];
  contactEmail?: string | null;
  /** Only set when the user explicitly consented in the UI. */
  diagnostics?: Diagnostics | null;
}

export type FeedbackOutcome = { type: 'openUrl'; url: string };
