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

export interface SyncState {
  configured: boolean;
  loggedIn: boolean;
  userEmail: string | null;
  pendingCount: number;
  lastSyncAt: string | null;
  cloudDeviceCount: number;
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
}

export type HistoryRetentionOption = 0 | 30 | 60 | 90;

export interface CreateCollectionInput {
  name: string;
  color: string;
}

export interface UpdateCollectionInput {
  id: string;
  name?: string;
  color?: string;
}
