import { Store } from '@tauri-apps/plugin-store';
import { UnlistenFn } from '@tauri-apps/api/event';

import { ToDelete } from '../types';

import { EndpointInfo } from '@martichou/core_lib/bindings/EndpointInfo';
import { Visibility } from '@martichou/core_lib/bindings/Visibility';
import { OutboundPayload } from '@martichou/core_lib/bindings/OutboundPayload';
import { ChannelMessage } from '@martichou/core_lib/bindings/ChannelMessage';

export interface TauriVM {
	store: Store;
    isAppInForeground: boolean;
    discoveryRunning: boolean;
    isDragHovering: boolean;
    requests: ChannelMessage[];
    endpointsInfo: EndpointInfo[];
    toDelete: ToDelete[];
    outboundPayload: OutboundPayload | undefined;
    unlisten: Array<UnlistenFn>;
    version: string | null;
    autostart: boolean;
    realclose: boolean;
    startminimized: boolean;
    darkmode: boolean;
    themeMediaQuery: MediaQueryList | null;
    themeMediaQueryHandler: ((event: MediaQueryListEvent) => void) | undefined;
    visibility: Visibility;
    downloadPath: string | undefined;
    updateCheckerEnabled: boolean;
    hostname: string | undefined;
    settingsOpen: boolean;
    new_version: string | null;
    enable: () => Promise<void>;
    disable: () => Promise<void>;
    invoke: (cmd: string, args?: InvokeArgs) => Promise<unknown>
    setVisibility: (vm: TauriVM, visibility: Visibility) => Promise<void>;
    setUpdateChecker: (vm: TauriVM, enabled: boolean) => Promise<void>;
    getUpdateChecker: (vm: TauriVM) => Promise<void>;
    setDarkMode: (vm: TauriVM, darkmode: boolean) => Promise<void>;
    getDarkMode: (vm: TauriVM) => Promise<void>;
    initSystemTheme: (vm: TauriVM) => void;
    cleanupSystemTheme: (vm: TauriVM) => void;
    applyTheme: (darkmode: boolean) => void;

    displayedIsEmpty: boolean;
    displayedItems: DisplayedItem[];

    // Remapped function for compatibility with Tauri v1 and v2
    dialogOpen: (options?: {
        title: string,
        directory: boolean,
        multiple: boolean,
    }) => Promise<unknown>;
}