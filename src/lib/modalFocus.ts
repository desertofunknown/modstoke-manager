type ModalFocusOptions = {
    generation: number;
    close: () => void;
};

function isVisible(element: HTMLElement): boolean {
    return element.isConnected
        && !element.closest('[inert], [aria-hidden="true"]')
        && element.getClientRects().length > 0
        && getComputedStyle(element).visibility === "visible";
}

export function modalFocus(node: HTMLElement, options: ModalFocusOptions) {
    const opener = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    node.focus({ preventScroll: true });

    function handleKeydown(event: KeyboardEvent) {
        if (event.defaultPrevented || event.isComposing) {
            return;
        }
        if (event.key === "Escape") {
            event.preventDefault();
            event.stopPropagation();
            options.close();
            return;
        }
        if (event.key !== "Tab") {
            return;
        }

        const controls = Array.from(node.querySelectorAll<HTMLElement>(
            'a[href], button, input, select, textarea, [tabindex], [contenteditable="true"], summary'
        )).filter(element => element.tabIndex >= 0 && !element.matches(":disabled") && isVisible(element));
        const active = document.activeElement;
        const index = controls.findIndex(element => element === active);
        let target: HTMLElement | undefined;
        if (index >= 0) {
            target = controls[(index + (event.shiftKey ? -1 : 1) + controls.length) % controls.length];
        } else if (active && active !== node && node.contains(active)) {
            // Section navigation focuses a container outside the normal tab order.
            const ordered = event.shiftKey ? [...controls].reverse() : controls;
            const direction = event.shiftKey ? Node.DOCUMENT_POSITION_PRECEDING : Node.DOCUMENT_POSITION_FOLLOWING;
            target = ordered.find(element => active.compareDocumentPosition(element) & direction);
        }
        event.preventDefault();
        (target ?? (event.shiftKey ? controls[controls.length - 1] : controls[0]) ?? node).focus();
    }

    node.addEventListener("keydown", handleKeydown);
    return {
        update(next: ModalFocusOptions) {
            const changed = next.generation !== options.generation;
            options = next;
            if (changed) {
                node.focus({ preventScroll: true });
            }
        },
        destroy() {
            node.removeEventListener("keydown", handleKeydown);
            if (opener && isVisible(opener) && !opener.matches(":disabled")) {
                opener.focus({ preventScroll: true });
            }
        }
    };
}
