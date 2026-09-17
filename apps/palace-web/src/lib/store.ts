import { create } from "zustand";
// Server records live in React Query; only transient library controls live here.
export const useLibrary = create<{
  search: string;
  setSearch: (search: string) => void;
}>((set) => ({ search: "", setSearch: (search) => set({ search }) }));
