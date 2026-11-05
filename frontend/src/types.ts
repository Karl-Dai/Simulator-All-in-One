import type { Language, Theme } from "../../ui/contracts/appearance";
export type { Language } from "../../ui/contracts/appearance";
export interface Preferences {
  schemaVersion: number;
  theme: Theme;
  language: Language;
  favorites: string[];
  recent: Record<string, string>;
}
export interface RoleState {
  id: string;
  protocol: string;
  name: string;
  product: string;
  version: string;
  ready: boolean;
  error: string;
}
export interface Snapshot {
  version: string;
  platform: string;
  architecture: string;
  roles: RoleState[];
  preferences: Preferences;
  warning: string;
}
export interface LaunchResult {
  id: string;
  pid: number;
  warning: string;
}

export interface SuiteAPI {
  Snapshot(): Promise<Snapshot>;
  Launch(id: string): Promise<LaunchResult>;
  SetFavorite(id: string, favorite: boolean): Promise<Preferences>;
  SetTheme(theme: Preferences["theme"]): Promise<Preferences>;
  SetLanguage(language: Language): Promise<Preferences>;
}
