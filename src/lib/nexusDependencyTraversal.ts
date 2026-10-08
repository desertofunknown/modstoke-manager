import { fetchNexusFileDependencies, type NexusModDependency, type NexusRateLimit } from "./nexus";

export const NESTED_DEPENDENCY_BATCH_SIZE = 8;
export const NESTED_DEPENDENCY_DEPTH_STEP = 2;

export type NestedDependencySource = {
    key: string;
    parentFileId: number;
    parentNexusFileId?: string;
    parentName: string;
    depth: number;
    dependency: NexusModDependency;
};

export type NestedDependencyTraversal = {
    pending: {
        fileId: number;
        nexusFileId?: string;
        parentName: string;
        depth: number;
    }[];
    visitedFileKeys: Set<string>;
    sources: NestedDependencySource[];
    checkedFiles: number;
    depthLimit: number;
    status: "idle" | "partial" | "complete" | "failed";
    error: string | null;
    rateLimit?: NexusRateLimit;
};

function fileKey(fileId: number, nexusFileId?: string): string {
    return `${fileId}:${nexusFileId?.trim() ?? ""}`;
}

export function createNestedDependencyTraversal(dependencies: NexusModDependency[]): NestedDependencyTraversal {
    const pending: NestedDependencyTraversal["pending"] = [];
    const queuedFileKeys = new Set<string>();

    for (const dependency of dependencies) {
        if (typeof dependency.file_id !== "number") {
            continue;
        }
        const key = fileKey(dependency.file_id, dependency.nexus_file_id);
        if (queuedFileKeys.has(key)) {
            continue;
        }
        queuedFileKeys.add(key);
        pending.push({
            fileId: dependency.file_id,
            nexusFileId: dependency.nexus_file_id,
            parentName: dependency.mod_name,
            depth: 1
        });
    }

    return {
        pending,
        visitedFileKeys: new Set<string>(),
        sources: [],
        checkedFiles: 0,
        depthLimit: NESTED_DEPENDENCY_DEPTH_STEP,
        status: "idle",
        error: null
    };
}

export async function advanceNestedDependencyTraversal(
    previous: NestedDependencyTraversal,
    isCurrent: () => boolean
): Promise<NestedDependencyTraversal | null> {
    if (!isCurrent()) {
        return null;
    }

    const traversal: NestedDependencyTraversal = {
        ...previous,
        pending: [...previous.pending],
        visitedFileKeys: new Set(previous.visitedFileKeys),
        sources: [...previous.sources],
        error: null
    };
    const queuedFileKeys = new Set(traversal.pending.map(file => fileKey(file.fileId, file.nexusFileId)));
    const rowKeys = new Set(traversal.sources.map(source => source.key));
    const first = traversal.pending[0];
    if (first && first.depth > traversal.depthLimit) {
        traversal.depthLimit = first.depth + NESTED_DEPENDENCY_DEPTH_STEP - 1;
    }

    let checkedInBatch = 0;
    try {
        while (traversal.pending.length > 0 && checkedInBatch < NESTED_DEPENDENCY_BATCH_SIZE) {
            const current = traversal.pending[0];
            if (current.depth > traversal.depthLimit) {
                break;
            }
            if (!isCurrent()) {
                return null;
            }

            const response = await fetchNexusFileDependencies(current.fileId, {
                nexusFileId: current.nexusFileId
            });
            if (!isCurrent()) {
                return null;
            }

            const currentKey = fileKey(current.fileId, current.nexusFileId);
            traversal.pending.shift();
            queuedFileKeys.delete(currentKey);
            traversal.visitedFileKeys.add(currentKey);
            traversal.checkedFiles += 1;
            checkedInBatch += 1;
            traversal.rateLimit = { ...response.rate_limit };

            for (const dependency of response.dependencies) {
                const rowKey = JSON.stringify([
                    current.fileId,
                    current.nexusFileId?.trim() ?? "",
                    dependency.id,
                    dependency.mod_id ?? null,
                    dependency.file_id ?? null,
                    dependency.nexus_file_id?.trim() ?? ""
                ]);
                if (!rowKeys.has(rowKey)) {
                    rowKeys.add(rowKey);
                    traversal.sources.push({
                        key: rowKey,
                        parentFileId: current.fileId,
                        parentNexusFileId: current.nexusFileId,
                        parentName: current.parentName,
                        depth: current.depth,
                        dependency: { ...dependency }
                    });
                }

                if (typeof dependency.file_id !== "number") {
                    continue;
                }
                const childKey = fileKey(dependency.file_id, dependency.nexus_file_id);
                if (traversal.visitedFileKeys.has(childKey) || queuedFileKeys.has(childKey)) {
                    continue;
                }
                queuedFileKeys.add(childKey);
                traversal.pending.push({
                    fileId: dependency.file_id,
                    nexusFileId: dependency.nexus_file_id,
                    parentName: dependency.mod_name,
                    depth: current.depth + 1
                });
            }
        }
    } catch (error) {
        if (!isCurrent()) {
            return null;
        }
        traversal.status = "failed";
        traversal.error = error instanceof Error ? error.message : String(error);
        return traversal;
    }

    traversal.status = traversal.pending.length === 0 ? "complete" : "partial";
    return traversal;
}
