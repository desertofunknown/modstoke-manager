<script lang="ts">
    import { onMount, createEventDispatcher } from "svelte";

    export let threshold = 0;
    export let horizontal = false;
    export let hasMore = true;
    export let loading = false;
    export let contentKey = "";

    const dispatch = createEventDispatcher<{ loadMore: void }>();
    let component: HTMLDivElement;
    let scrollElement: HTMLElement | null = null;
    let frame: number | null = null;
    let requested = false;

    $: if (!loading) {
        requested = false;
    }
    $: {
        contentKey;
        if (scrollElement && hasMore && !loading) scheduleCheck();
    }

    function scheduleCheck() {
        if (frame === null && scrollElement) {
            frame = requestAnimationFrame(checkPosition);
        }
    }

    function checkPosition() {
        frame = null;
        if (!scrollElement || !hasMore || loading || requested || scrollElement.clientHeight === 0) {
            return;
        }
        const offset = horizontal
            ? scrollElement.scrollWidth - scrollElement.clientWidth - scrollElement.scrollLeft
            : scrollElement.scrollHeight - scrollElement.clientHeight - scrollElement.scrollTop;
        if (offset <= threshold) {
            requested = true;
            dispatch("loadMore");
        }
    }

    onMount(() => {
        scrollElement = component.parentElement;
        scrollElement?.addEventListener("scroll", scheduleCheck, { passive: true });
        const observer = new ResizeObserver(scheduleCheck);
        if (scrollElement) observer.observe(scrollElement);
        scheduleCheck();
        return () => {
            scrollElement?.removeEventListener("scroll", scheduleCheck);
            observer.disconnect();
            if (frame !== null) cancelAnimationFrame(frame);
            scrollElement = null;
        };
    });
</script>

<div bind:this={component} aria-hidden="true" style="width: 0; height: 0;"></div>
