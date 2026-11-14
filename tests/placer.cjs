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
        workspace: {
            windowList: () => windows,
            windowAdded: added,
            cursorPos: { x: 0, y: 0 },
            screenAt: () => null,
        },
        registerShortcut: (name, _label, _key, action) => shortcuts.set(name, action),
        callDBus: (...args) => calls.push(args),
    };
    runInNewContext(readFileSync('kwin/placer.js', 'utf8'), context);
    return { windows, calls, shortcuts, added, changed };
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
