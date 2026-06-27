import { create } from "zustand";
import type { AppTab, PreviewCard, TimelineSection, Collection } from "@memora/shared-types";
import * as api from "@/lib/api";

function tabSearchFilters(
  tab: AppTab,
  collectionId: string | null,
): Partial<import("@memora/shared-types").SearchFilters> {
  switch (tab) {
    case "pinned":
      return { isPinned: true };
    case "favorites":
      return { isFavorite: true };
    case "collections":
      return collectionId
        ? { collection: collectionId }
        : { inCollection: true };
    case "snippets":
      return { isSnippet: true };
    default:
      return {};
  }
}

interface AppState {
  quickPasteOpen: boolean;
  trayOpen: boolean;
  query: string;
  activeTab: AppTab;
  selectedCollectionId: string | null;
  clipboardPaused: boolean;
  results: PreviewCard[];
  timeline: TimelineSection[];
  collections: Collection[];
  selectedIndex: number;
  loading: boolean;
  setQuickPasteOpen: (open: boolean) => void;
  setTrayOpen: (open: boolean) => void;
  setQuery: (query: string) => void;
  setActiveTab: (tab: AppTab) => void;
  setSelectedCollectionId: (id: string | null) => void;
  setSelectedIndex: (index: number) => void;
  toggleClipboardPause: () => Promise<void>;
  refresh: () => Promise<void>;
  search: (query: string) => Promise<void>;
}

export const useAppStore = create<AppState>((set, get) => ({
  quickPasteOpen: false,
  trayOpen: false,
  query: "",
  activeTab: "history",
  selectedCollectionId: null,
  clipboardPaused: false,
  results: [],
  timeline: [],
  collections: [],
  selectedIndex: 0,
  loading: false,

  setQuickPasteOpen: (open) => set({ quickPasteOpen: open }),
  setTrayOpen: (open) => set({ trayOpen: open }),
  setQuery: (query) => set({ query }),
  setActiveTab: (tab) => {
    set({ activeTab: tab, selectedIndex: 0 });
    if (tab !== "collections") {
      set({ selectedCollectionId: null });
    }
    const { query } = get();
    if (query.trim()) {
      void get().search(query);
    } else {
      void get().refresh();
    }
  },
  setSelectedCollectionId: (id) => {
    set({ selectedCollectionId: id, activeTab: "collections", selectedIndex: 0 });
    const { query } = get();
    if (query.trim()) {
      void get().search(query);
    } else {
      void get().refresh();
    }
  },
  setSelectedIndex: (index) => set({ selectedIndex: index }),

  toggleClipboardPause: async () => {
    const paused = await api.toggleClipboardPause();
    set({ clipboardPaused: paused });
  },

  refresh: async () => {
    const { activeTab, selectedCollectionId } = get();
    set({ loading: true });
    try {
      const [timeline, collections, clipboardPaused] = await Promise.all([
        api.getTabTimeline(activeTab, selectedCollectionId ?? undefined),
        api.getCollections(),
        api.getClipboardPaused(),
      ]);
      set({ timeline, collections, clipboardPaused, loading: false });
    } catch {
      set({ loading: false });
    }
  },

  search: async (query: string) => {
    const { activeTab, selectedCollectionId } = get();
    if (!query.trim()) {
      await get().refresh();
      set({ results: [], selectedIndex: 0, query });
      return;
    }

    set({ loading: true, query });
    try {
      const results = await api.searchItems({
        query,
        ...tabSearchFilters(activeTab, selectedCollectionId),
      });
      set({ results, selectedIndex: 0, loading: false });
    } catch {
      set({ loading: false });
    }
  },
}));
