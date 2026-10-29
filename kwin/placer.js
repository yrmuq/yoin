const GAP = 8;
const windows = new Map();
let lastAnchor = null;

function textOf(value) {
    return value == null ? "" : String(value);
}

function isPopup(window) {
    return textOf(window.caption) === "yoin-buttons" && !window.dialog;
}

function isNotice(window) {
    return textOf(window.caption) === "yoin-notices" && !window.dialog;
}

function findPopup() {
    const list = workspace.windowList();
    for (let i = 0; i < list.length; i++) {
        const window = list[i];
        if (isPopup(window) && !window.deleted && !window.hidden) {
            return window;
        }
    }
    return null;
}

function stickNotice(notice, popup) {
    if (!notice || !popup || notice.deleted || notice.hidden || popup.deleted || popup.hidden) {
        return;
    }
    const pf = popup.frameGeometry;
    const nf = notice.frameGeometry;
    if (nf.width <= 0 || nf.height <= 0 || pf.width <= 0) {
        return;
    }
    const ratio = ratioOf(notice);
    const x = snap(pf.x, ratio);
    const y = snap(pf.y - nf.height - GAP, ratio);
    if (Math.abs(nf.x - x) < 0.5 && Math.abs(nf.y - y) < 0.5) {
        return;
    }
    notice.frameGeometry = { x: x, y: y, width: nf.width, height: nf.height };
}

function repositionNotices() {
    const popup = findPopup();
    if (!popup) {
        return;
    }
    const list = workspace.windowList();
    for (let i = 0; i < list.length; i++) {
        if (isNotice(list[i])) {
            stickNotice(list[i], popup);
        }
    }
}

function hideFromTaskbar(window) {
    window.skipTaskbar = true;
    window.skipSwitcher = true;
    window.skipPager = true;
}

function contains(rect, point) {
    return (
        point.x >= rect.x && point.x < rect.x + rect.width &&
        point.y >= rect.y && point.y < rect.y + rect.height
    );
}

function edgeOf(panel, screen) {
    if (panel.width >= panel.height) {
        return Math.abs(panel.y - screen.y) < Math.abs(screen.y + screen.height - panel.y - panel.height)
            ? "top" : "bottom";
    }
    return Math.abs(panel.x - screen.x) < Math.abs(screen.x + screen.width - panel.x - panel.width)
        ? "left" : "right";
}

function clamp(value, start, end) {
    return Math.max(start, Math.min(value, Math.max(start, end)));
}

function ratioOf(window) {
    const output = window && window.output;
    const ratio = output && output.devicePixelRatio;
    return ratio > 0 ? ratio : 1;
}

function snap(value, ratio) {
    return Math.round(value * ratio) / ratio;
}

function placement(size, area, anchor) {
    let x = area.x + area.width - size.width - GAP;
    let y = area.y + GAP;
    if (anchor) {
        const panel = anchor.panel.frameGeometry;
        switch (anchor.edge) {
        case "top":
            x = anchor.point.x - size.width / 2;
            y = panel.y + panel.height + GAP;
            break;
        case "bottom":
            x = anchor.point.x - size.width / 2;
            y = panel.y - size.height - GAP;
            break;
        case "left":
            x = panel.x + panel.width + GAP;
            y = anchor.point.y - size.height / 2;
            break;
        case "right":
            x = panel.x - size.width - GAP;
            y = anchor.point.y - size.height / 2;
            break;
        }
    }
    return {
        x: Math.round(clamp(x, area.x + GAP, area.x + area.width - size.width - GAP)),
        y: Math.round(clamp(y, area.y + GAP, area.y + area.height - size.height - GAP)),
        width: size.width,
        height: size.height,
    };
}

function panelWindows() {
    return workspace.windowList().filter((window) =>
        window.dock && String(window.resourceClass) === "plasmashell" &&
        window.frameGeometry.width > 0 && window.frameGeometry.height > 0
    );
}

function selectAnchor(window) {
    const panels = panelWindows();
    const cursor = { x: workspace.cursorPos.x, y: workspace.cursorPos.y };
    const clicked = panels.find((panel) => contains(panel.frameGeometry, cursor));
    if (clicked) {
        lastAnchor = {
            panel: clicked,
            edge: edgeOf(clicked.frameGeometry, clicked.output.geometry),
            point: cursor,
        };
        return lastAnchor;
    }
    if (lastAnchor && panels.includes(lastAnchor.panel)) {
        const panel = lastAnchor.panel;
        const edge = edgeOf(panel.frameGeometry, panel.output.geometry);
        if (edge === lastAnchor.edge && contains(panel.frameGeometry, lastAnchor.point)) {
            return lastAnchor;
        }
    }
    lastAnchor = null;
    const output = workspace.screenAt(cursor) || window.output;
    const panel = panels.find((candidate) => candidate.output === output);
    if (!panel) {
        return null;
    }
    const rect = panel.frameGeometry;
    return {
        panel: panel,
        edge: edgeOf(rect, panel.output.geometry),
        point: { x: rect.x + rect.width - GAP, y: rect.y + rect.height - GAP },
    };
}

function place(entry) {
    const window = entry.window;
    if (entry.moving || window.deleted || window.hidden || window.fullScreen || window.move || window.resize) {
        return;
    }
    const frame = window.frameGeometry;
    if (frame.width <= 0 || frame.height <= 0) {
        return;
    }
    const output = entry.anchor ? entry.anchor.panel.output : window.output;
    const area = workspace.clientArea(KWin.MaximizeArea, output, workspace.currentDesktop);
    const ratio = ratioOf(window);
    const target = placement(frame, area, entry.anchor);
    target.x = snap(target.x, ratio);
    target.y = snap(target.y, ratio);
    if (frame.x === target.x && frame.y === target.y) {
        return;
    }
    entry.moving = true;
    try {
        window.frameGeometry = target;
    } finally {
        entry.moving = false;
    }
    repositionNotices();
}

function track(window) {
    if (!isPopup(window) || windows.has(window)) {
        return;
    }
    hideFromTaskbar(window);
    window.keepAbove = true;
    const entry = { window: window, anchor: null, moving: false };
    windows.set(window, entry);
    const onShow = () => {
        entry.anchor = selectAnchor(window);
        place(entry);
    };
    const visibilityChanged = window.windowShown || window.hiddenChanged;
    if (visibilityChanged) {
        visibilityChanged.connect(() => {
            if (!window.hidden) {
                onShow();
            }
        });
    }
    window.frameGeometryChanged.connect((previous) => {
        if (entry.moving) {
            return;
        }
        const current = window.frameGeometry;
        if (previous && previous.height !== current.height && Math.abs(previous.width - current.width) < 1) {
            const ratio = ratioOf(window);
            const bottom = previous.y + previous.height;
            const next = {
                x: snap(previous.x, ratio),
                y: snap(bottom - current.height, ratio),
                width: current.width,
                height: current.height,
            };
            if (next.x !== current.x || next.y !== current.y) {
                entry.moving = true;
                try {
                    window.frameGeometry = next;
                } finally {
                    entry.moving = false;
                }
            }
            repositionNotices();
            return;
        }
        place(entry);
    });
    window.closed.connect(() => windows.delete(window));
    onShow();
}

function consider(window) {
    if (isPopup(window)) {
        hideFromTaskbar(window);
        track(window);
        return;
    }
    if (isNotice(window)) {
        hideFromTaskbar(window);
        trackNotice(window);
        return;
    }
    if (isSettings(window)) {
        showOnTaskbar(window);
    }
}

function showOnTaskbar(window) {
    window.skipTaskbar = false;
    window.skipSwitcher = false;
    window.skipPager = false;
}

function isSettings(window) {
    return textOf(window.caption) === "yoin";
}

function raiseSettings() {
    const list = workspace.windowList();
    for (let i = 0; i < list.length; i++) {
        const window = list[i];
        if (!isSettings(window)) {
            continue;
        }
        showOnTaskbar(window);
        window.minimized = false;
        workspace.activeWindow = window;
    }
}

registerShortcut("Yoin Raise Settings", "Raise yoin settings", "", raiseSettings);

const trackedNotices = new Map();

function trackNotice(window) {
    if (!isNotice(window) || trackedNotices.has(window)) {
        return;
    }
    hideFromTaskbar(window);
    window.keepAbove = true;
    const entry = { moving: false };
    trackedNotices.set(window, entry);
    const visibilityChanged = window.windowShown || window.hiddenChanged;
    if (visibilityChanged) {
        visibilityChanged.connect(() => {
            if (!window.hidden) {
                repositionNotices();
            }
        });
    }
    window.frameGeometryChanged.connect(() => {
        if (entry.moving) {
            return;
        }
        const popup = findPopup();
        entry.moving = true;
        try {
            stickNotice(window, popup);
        } finally {
            entry.moving = false;
        }
    });
    window.closed.connect(() => trackedNotices.delete(window));
    repositionNotices();
}

function arm(window) {
    consider(window);
    if (window.captionChanged) {
        window.captionChanged.connect(() => consider(window));
    }
}

workspace.windowAdded.connect(arm);
workspace.windowList().forEach(arm);
