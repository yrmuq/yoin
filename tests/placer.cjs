// Copyright (C) 2026 yrmuq
// SPDX-License-Identifier: GPL-3.0-or-later
const assert = require('node:assert/strict');
const { readFileSync } = require('node:fs');
const { runInNewContext } = require('node:vm');
const test = require('node:test');

class Signal {
    handlers = new Set();
    connect = (handler) => this.handlers.add(handler);
    disconnect = (handler) => this.handlers.delete(handler);
    emit(...args) { for (const handler of [...this.handlers]) handler(...args); }
}

function setup() {
    const windows = [];
    const calls = [];
    const shortcuts = new Map();
    const added = new Signal();
    const changed = new Signal();
    const context = {
        yoinReadyBus: ':1.42',
        options: { configChanged: changed },
        KWin: { MaximizeArea: 0 },
        workspace: {
            currentDesktop: 1,
            clientArea: (_kind, output) => output.area,
            windowList: () => windows,
            windowAdded: added,
            cursorPos: { x: 0, y: 0 },
            screenAt: () => null,
        },
        registerShortcut: (name, _label, _key, action) => shortcuts.set(name, action),
        callDBus: (...args) => calls.push(args),
    };
    runInNewContext(readFileSync('kwin/placer.js', 'utf8'), context);
    return { windows, calls, shortcuts, added, changed, workspace: context.workspace };
}

test('popup class is hidden before its caption arrives, settings stay on the taskbar', () => {
    const state = setup();
    const popup = {
        resourceClass: 'yoin-popup', resourceName: '', caption: '',
        captionChanged: new Signal(),
    };
    state.added.emit(popup);
    assert.equal(popup.skipTaskbar, true);
    assert.equal(popup.skipSwitcher, true);
    const settings = {
        resourceClass: 'yoin', resourceName: '', caption: 'yoin',
        captionChanged: new Signal(), minimized: true,
    };
    state.windows.push(settings);
    state.added.emit(settings);
    state.shortcuts.get('Yoin Raise Settings')();
    assert.equal(settings.skipTaskbar, false);
    assert.equal(settings.minimized, false);
    assert.equal(popup.skipTaskbar, true);
});

test('rule reload acknowledgement follows configChanged and is sent once', () => {
    const state = setup();
    assert.equal(state.calls.length, 0);
    state.changed.emit();
    assert.deepEqual(state.calls, [[ ':1.42', '/yoin/Placer', 'io.github.yrmuq.yoin.Placer', 'Ready' ]]);
    state.changed.emit();
    assert.equal(state.calls.length, 1);
});

function panelState(edge) {
    const state = setup();
    const output = { devicePixelRatio: 1.25, geometry: { x: 0, y: 0, width: 1920, height: 1080 } };
    const frames = {
        top: { x: 0, y: 0, width: 1920, height: 40 },
        bottom: { x: 0, y: 1040, width: 1920, height: 40 },
        left: { x: 0, y: 0, width: 40, height: 1080 },
        right: { x: 1880, y: 0, width: 40, height: 1080 },
    };
    output.area = {
        x: edge === 'left' ? 40 : 0, y: edge === 'top' ? 40 : 0,
        width: edge === 'left' || edge === 'right' ? 1880 : 1920,
        height: edge === 'top' || edge === 'bottom' ? 1040 : 1080,
    };
    state.windows.push({ dock: true, resourceClass: 'plasmashell', frameGeometry: frames[edge], output });
    state.workspace.cursorPos = {
        x: frames[edge].x + frames[edge].width / 2,
        y: frames[edge].y + frames[edge].height / 2,
    };
    state.workspace.screenAt = () => output;
    return { ...state, output };
}

function surface(state, kind, width, height) {
    const window = {
        resourceClass: 'yoin-popup-' + kind, resourceName: '', caption: '',
        output: state.output, hidden: false, dialog: false,
        captionChanged: new Signal(), frameGeometryChanged: new Signal(),
        windowShown: new Signal(), closed: new Signal(),
    };
    let frame = { x: 800, y: 400, width, height };
    Object.defineProperty(window, 'frameGeometry', {
        get: () => frame,
        set: (next) => {
            const previous = frame;
            frame = { ...next };
            window.frameGeometryChanged.emit(previous);
        },
    });
    state.windows.push(window);
    state.added.emit(window);
    return window;
}

function near(actual, expected) {
    assert.ok(Math.abs(actual - expected) < 0.41, `${actual} is not pixel-aligned near ${expected}`);
}

test('panels are placed by class before the first caption and notices can arrive first', () => {
    const state = panelState('bottom');
    const notices = surface(state, 'notices', 300, 87);
    const popup = surface(state, 'buttons', 300, 96);
    assert.equal(popup.caption, '');
    assert.equal(popup.frameGeometry.y, 936);
    near(notices.frameGeometry.y, popup.frameGeometry.y - 87);
    assert.equal(notices.frameGeometry.x, popup.frameGeometry.x);
    for (const window of [popup, notices]) {
        assert.equal(window.skipTaskbar, true);
        assert.equal(window.noBorder, true);
        assert.equal(window.skipsCloseAnimation, true);
    }
    // A delayed notice map still gets placed when the popup did not move.
    notices.hidden = true;
    notices.frameGeometry = { x: 800, y: 400, width: 300, height: 231 };
    notices.hidden = false;
    popup.windowShown.emit();
    near(notices.frameGeometry.y, popup.frameGeometry.y - 231);
    assert.equal(notices.frameGeometry.x, popup.frameGeometry.x);
});

test('content resizing follows the selected panel edge and preserves the requested size', () => {
    for (const edge of ['top', 'bottom', 'left', 'right']) {
        const state = panelState(edge);
        const popup = surface(state, 'buttons', 300, 96);
        const notices = surface(state, 'notices', 300, 87);
        for (const [width, height] of [[300, 510], [300, 96], [526, 388], [300, 96]]) {
            popup.frameGeometry = { ...popup.frameGeometry, width, height };
            const frame = popup.frameGeometry;
            assert.equal(frame.width, width);
            assert.equal(frame.height, height);
            if (edge === 'top') near(frame.y, 48);
            if (edge === 'bottom') near(frame.y + height, 1032);
            if (edge === 'left') near(frame.x, 48);
            if (edge === 'right') near(frame.x + width, 1872);
            near(notices.frameGeometry.y + notices.frameGeometry.height, frame.y);
            assert.equal(notices.frameGeometry.x, frame.x);
        }
    }
});
