# Copyright (C) 2026 yrmuq
# SPDX-License-Identifier: GPL-3.0-or-later

import unittest
from pathlib import Path

from yoin.config import apply_autostart, autostart_text, format_exec, load_config, save_config
from yoin.ha import (
    APP_ID,
    action_for,
    button_label,
    button_on,
    confirm_id,
    entities_from_states,
    notification_text,
    registration_body,
    state_event,
    ws_url,
)


class HaTests(unittest.TestCase):
    def test_ws_url(self):
        self.assertEqual(ws_url("http://ha.local:8123"), "ws://ha.local:8123/api/websocket")
        self.assertEqual(
            ws_url("https://ha.local/home/"), "wss://ha.local/home/api/websocket"
        )
        with self.assertRaises(ValueError):
            ws_url("ha.local")

    def test_registration_is_not_the_official_app(self):
        body = registration_body("abc", "Fedora", "0.1.0", "6.8.0")
        self.assertEqual(body["app_id"], APP_ID)
        self.assertNotIn("robbie", body["app_id"])
        self.assertTrue(body["app_data"]["push_websocket_channel"])
        self.assertFalse(body["supports_encryption"])

    def test_actions(self):
        self.assertEqual(action_for("switch.porch"), ("switch", "toggle"))
        self.assertEqual(action_for("light.kitchen"), ("light", "toggle"))
        self.assertEqual(action_for("script.night"), ("script", "turn_on"))
        self.assertEqual(action_for("lock.door", "locked"), ("lock", "unlock"))
        self.assertEqual(action_for("lock.door", "unlocked"), ("lock", "lock"))
        self.assertEqual(
            action_for("alarm_control_panel.home", "armed_away"),
            ("alarm_control_panel", "alarm_disarm"),
        )
        self.assertEqual(
            action_for("alarm_control_panel.home", "disarmed"),
            ("alarm_control_panel", "alarm_arm_away"),
        )
        self.assertEqual(action_for("climate.hall", "heat"), ("climate", "turn_off"))
        self.assertEqual(action_for("water_heater.bath", "off"), ("water_heater", "turn_on"))
        self.assertIsNone(action_for("sensor.temp"))
        self.assertIsNone(action_for(""))

    def test_notification(self):
        self.assertEqual(notification_text({"message": "hi"}), ("yoin", "hi"))
        self.assertEqual(
            notification_text({"title": "Door", "message": "open"}), ("Door", "open")
        )
        self.assertIsNone(notification_text({}))
        self.assertEqual(confirm_id({"hass_confirm_id": "abc"}), "abc")
        self.assertIsNone(confirm_id({"hass_confirm_id": ""}))

    def test_entities_keep_toggleable(self):
        states = [
            {"entity_id": "light.a", "state": "on", "attributes": {"friendly_name": "A"}},
            {"entity_id": "switch.b", "state": "off", "attributes": {"friendly_name": "Porch"}},
            {"entity_id": "script.a", "state": "off", "attributes": {}},
            {"entity_id": "sensor.temp", "state": "20", "attributes": {"friendly_name": "Temp"}},
            {
                "entity_id": "alarm_control_panel.home",
                "state": "disarmed",
                "attributes": {"friendly_name": "Home"},
            },
        ]
        found = entities_from_states(states)
        self.assertEqual(
            [item["entity_id"] for item in found],
            ["light.a", "alarm_control_panel.home", "switch.b", "script.a"],
        )
        porch = next(item for item in found if item["entity_id"] == "switch.b")
        script = next(item for item in found if item["entity_id"] == "script.a")
        self.assertEqual(button_label("switch.b", porch), "Porch: off")
        self.assertEqual(button_label("script.a", script), "script.a: off")
        self.assertFalse(button_on(porch))
        self.assertTrue(button_on({"state": "on"}))
        self.assertTrue(button_on({"state": "armed_away"}))
        self.assertFalse(button_on({"state": "disarmed"}))
        self.assertEqual(button_label("", None), "Not set")

    def test_state_event(self):
        event = {
            "event_type": "state_changed",
            "data": {
                "entity_id": "switch.b",
                "new_state": {
                    "entity_id": "switch.b",
                    "state": "on",
                    "attributes": {"friendly_name": "Porch"},
                },
            },
        }
        found = state_event(event)
        self.assertEqual(found["state"], "on")
        self.assertIsNone(state_event({"event_type": "state_changed", "data": {}}))


class ConfigTests(unittest.TestCase):
    def test_roundtrip_permissions(self):
        root = Path("/tmp/yoin-cfg-test")
        path = root / "config.json"
        if root.exists():
            for child in root.iterdir():
                child.unlink()
        save_config(
            {
                "url": "http://ha.local:8123",
                "token": "secret",
                "device_name": "Fedora",
                "device_id": "abc",
                "webhook_id": "hook",
                "buttons": ["switch.b", 3],
                "autostart": True,
            },
            path,
        )
        self.assertEqual(path.stat().st_mode & 0o777, 0o600)
        loaded = load_config(path)
        self.assertEqual(loaded["token"], "secret")
        self.assertEqual(loaded["buttons"], ["switch.b", ""])
        self.assertTrue(loaded["autostart"])
        save_config({"buttons": [f"switch.{i}" for i in range(12)]}, path)
        self.assertEqual(len(load_config(path)["buttons"]), 10)
        save_config({"buttons": []}, path)
        self.assertEqual(load_config(path)["buttons"], [""])
        path.write_text("{", encoding="utf-8")
        self.assertEqual(load_config(path)["token"], "")
        self.assertTrue((root / "config.json.bak").is_file())
        (root / "config.json.bak").unlink()

    def test_exec_and_autostart(self):
        self.assertEqual(format_exec(["/usr/bin/python3", "-m", "yoin"]), "/usr/bin/python3 -m yoin")
        self.assertEqual(format_exec(["/home/a b/yoin.AppImage"]), '"/home/a b/yoin.AppImage"')
        text = autostart_text("/usr/bin/yoin")
        self.assertIn("X-KDE-autostart-after=panel", text)
        path = Path("/tmp/yoin-autostart.desktop")
        if path.exists():
            path.unlink()
        # apply_autostart writes the real home path. This only checks removal
        # of a file we pass in.
        path.write_text("x", encoding="utf-8")
        apply_autostart(False, path)
        self.assertFalse(path.exists())


if __name__ == "__main__":
    unittest.main()
