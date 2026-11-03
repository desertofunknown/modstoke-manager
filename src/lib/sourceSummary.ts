// What the SOTF Mods and Nexus panels report to the Mod Hub header through summaryChange.
export type SourceSummary = {
    visible: number;
    total: number;
    mode: string;
    note: string;
    activeFilters: boolean;
    attention: number;
};
