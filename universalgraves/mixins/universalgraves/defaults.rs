// The config's defaults, as the original writes them (`config/universal-graves/config.json`).
// Ported from Universal Graves by Patbox (LGPL-3.0).
pub(super) const DEFAULT_CONFIG: &str = r##"{
  "_comment": "Before changing anything, see https://github.com/Patbox/UniversalGraves#configuration",
  "config_version": 4,
  "protection": {
    "non_owner_protection_time": 900,
    "self_destruction_time": -1,
    "drop_items_on_expiration": true,
    "attackers_bypass_protection": false,
    "use_real_time": false
  },
  "interactions": {
    "unlocking_cost": {
      "type": "free",
      "count": 0
    },
    "give_death_compass": true,
    "enable_use_death_compass_to_open_gui": true,
    "enable_click_to_open_gui": true,
    "shift_and_use_quick_pickup": true,
    "break_quick_pickup": true,
    "allow_remote_protection_removal": true,
    "allow_remote_breaking": true,
    "allow_remote_unlocking": false
  },
  "storage": {
    "experience_type": "percent_points",
    "experience_percent:setting_value": 100,
    "can_store_only_xp": false,
    "alternative_experience_entity": false,
    "blocked_enchantments": []
  },
  "placement": {
    "player_grave_limit": -1,
    "replace_any_block": false,
    "max_distance_from_source_location": 8,
    "shift_location_on_failure": true,
    "max_shift_tries": 5,
    "max_shift_distance": 40,
    "generate_on_top_of_fluids": false,
    "generate_on_ground": false,
    "create_gravestone_after_emptying": false,
    "restore_replaced_block_after_player_breaking": true,
    "move_inside_world_border": true,
    "actively_move_inside_world_border": false,
    "cancel_creation_for_damage_types": {},
    "cancel_creation_for_ignored_attacker_types": {},
    "blocking_predicates": [],
    "block_in_protected_area": {},
    "blacklisted_worlds": [],
    "blacklisted_areas": {},
    "creation_default_failure_text": "<red><lang:'text.graves.creation_failed':'<gold>${position}':'<yellow>${world}'>",
    "creation_claim_failure_text": "<red><lang:'text.graves.creation_failed_claim':'<gold>${position}':'<yellow>${world}'>"
  },
  "teleportation": {
    "cost": {
      "type": "creative",
      "count": 1
    },
    "required_time": 5,
    "y_offset": 1,
    "invincibility_time": 2,
    "allow_movement_while_waiting": false,
    "text": {
      "timer": "<lang:'text.graves.teleport.teleport_timer':'${time}'>",
      "timer_allow_moving": "<lang:'text.graves.teleport.teleport_timer_moving':'${time}'>",
      "location": "<lang:'text.graves.teleport.teleport_location':'${position}'>",
      "canceled": "<red><lang:'text.graves.teleport.teleport_cancelled'>"
    }
  },
  "model": {
    "default": "default",
    "alternative": [],
    "hide_f3_debug_lines": true,
    "gravestone_item_base": "minecraft:skeleton_skull",
    "gravestone_item_nbt": {}
  },
  "ui": {
    "title": "<lang:'text.graves.players_grave':'${player}'>",
    "admin_title": "<lang:'text.graves.admin_graves'>",
    "list_grave_icon": {
      "base": {
        "icon": "minecraft:chest",
        "text": [
          "${position} <gray>(${world})",
          "<yellow>${death_cause}",
          "<gray><lang:'text.graves.items_xp':'<white>${item_count}':'<white>${xp}'>",
          "<blue><lang:'text.graves.protected_time':'<white>${protection_time}'>",
          "<red><lang:'text.graves.break_time':'<white>${break_time}'>"
        ]
      },
      "alt": {
        "icon": "minecraft:trapped_chest",
        "text": [
          "${position} <gray>(${world})",
          "<yellow>${death_cause}",
          "<gray><lang:'text.graves.items_xp':'<white>${item_count}':'<white>${xp}'>",
          "<blue><lang:'text.graves.not_protected'>",
          "<red><lang:'text.graves.break_time':'<white>${break_time}'>"
        ]
      }
    },
    "admin_list_grave_icon": {
      "base": {
        "icon": "minecraft:chest",
        "text": [
          "<dark_gray>[<white>${player}</>]</> ${position} <gray>(${world})",
          "<yellow>${death_cause}",
          "<gray><lang:'text.graves.items_xp':'<white>${item_count}':'<white>${xp}'>",
          "<blue><lang:'text.graves.protected_time':'<white>${protection_time}'>",
          "<red><lang:'text.graves.break_time':'<white>${break_time}'>"
        ]
      },
      "alt": {
        "icon": "minecraft:trapped_chest",
        "text": [
          "<dark_gray>[<white>${player}</>]</> ${position} <gray>(${world})",
          "<yellow>${death_cause}",
          "<gray><lang:'text.graves.items_xp':'<white>${item_count}':'<white>${xp}'>",
          "<blue><lang:'text.graves.not_protected'>",
          "<red><lang:'text.graves.break_time':'<white>${break_time}'>"
        ]
      }
    },
    "grave_info": {
      "base": {
        "icon": "minecraft:oak_sign",
        "text": [
          "${position} <gray>(${world})",
          "<yellow>${death_cause}",
          "<gray><lang:'text.graves.items_xp':'<white>${item_count}':'<white>${xp}'>",
          "<blue><lang:'text.graves.protected_time':'<white>${protection_time}'>",
          "<red><lang:'text.graves.break_time':'<white>${break_time}'>"
        ]
      },
      "alt": {
        "icon": "minecraft:oak_sign",
        "text": [
          "${position} <gray>(${world})",
          "<yellow>${death_cause}",
          "<gray><lang:'text.graves.items_xp':'<white>${item_count}':'<white>${xp}'>",
          "<blue><lang:'text.graves.not_protected'>",
          "<red><lang:'text.graves.break_time':'<white>${break_time}'>"
        ]
      }
    },
    "unlock_grave": {
      "base": {
        "icon": "minecraft:gold_ingot",
        "text": [
          "<#ffd257><lang:'text.graves.gui.unlock_grave'>",
          "<white><lang:'text.graves.gui.cost'> <#cfcfcf>${cost}"
        ]
      },
      "alt": {
        "icon": "minecraft:gold_ingot",
        "text": [
          "<dark_gray><lang:'text.graves.gui.unlock_grave'>",
          "<white><lang:'text.graves.gui.cost'> <#cfcfcf>${cost} <gray>(<red><lang:'text.graves.gui.cost.not_enough'></red>)"
        ]
      }
    },
    "previous_button": {
      "base": {
        "icon": {
          "id": "universal_graves:icon",
          "count": 1,
          "components": {
            "universal_graves:texture": "previous_page"
          }
        },
        "text": "<lang:'text.graves.gui.previous_page'>"
      },
      "alt": {
        "icon": {
          "id": "universal_graves:icon",
          "count": 1,
          "components": {
            "universal_graves:texture": "previous_page_blocked"
          }
        },
        "text": "<dark_gray><lang:'text.graves.gui.previous_page'>"
      }
    },
    "next_button": {
      "base": {
        "icon": {
          "id": "universal_graves:icon",
          "count": 1,
          "components": {
            "universal_graves:texture": "next_page"
          }
        },
        "text": "<lang:'text.graves.gui.next_page'>"
      },
      "alt": {
        "icon": {
          "id": "universal_graves:icon",
          "count": 1,
          "components": {
            "universal_graves:texture": "next_page_blocked"
          }
        },
        "text": "<dark_gray><lang:'text.graves.gui.next_page'>"
      }
    },
    "remove_protection_button": {
      "base": {
        "icon": {
          "id": "universal_graves:icon",
          "count": 1,
          "components": {
            "universal_graves:texture": "remove_protection"
          }
        },
        "text": "<red><lang:'text.graves.gui.remove_protection'>"
      },
      "alt": {
        "icon": {
          "id": "universal_graves:icon",
          "count": 1,
          "components": {
            "universal_graves:texture": "remove_protection"
          }
        },
        "text": [
          "<red><lang:'text.graves.gui.remove_protection'>",
          "<dark_red><bold><lang:'text.graves.gui.cant_reverse'>",
          "",
          "<white><lang:'text.graves.gui.click_to_confirm'>"
        ]
      }
    },
    "break_grave_button": {
      "base": {
        "icon": {
          "id": "universal_graves:icon",
          "count": 1,
          "components": {
            "universal_graves:texture": "break_grave"
          }
        },
        "text": "<red><lang:'text.graves.gui.break_grave'>"
      },
      "alt": {
        "icon": {
          "id": "universal_graves:icon",
          "count": 1,
          "components": {
            "universal_graves:texture": "break_grave"
          }
        },
        "text": [
          "<red><lang:'text.graves.gui.break_grave'>",
          "<dark_red><bold><lang:'text.graves.gui.cant_reverse'>",
          "",
          "<white><lang:'text.graves.gui.click_to_confirm'>"
        ]
      }
    },
    "quick_pickup_button": {
      "icon": {
        "id": "universal_graves:icon",
        "count": 1,
        "components": {
          "universal_graves:texture": "quick_pickup"
        }
      },
      "text": "<red><lang:'text.graves.gui.quick_pickup'>"
    },
    "fetch_button": {
      "base": {
        "icon": "minecraft:lead",
        "text": "<yellow><lang:'text.graves.gui.fetch'>"
      },
      "alt": {
        "icon": "minecraft:lead",
        "text": "<yellow><lang:'text.graves.gui.fetch'>"
      }
    },
    "teleport_button": {
      "base": {
        "icon": "minecraft:ender_pearl",
        "text": [
          "<#a52dfa><lang:'text.graves.gui.teleport'>",
          "<white><lang:'text.graves.gui.cost'> <#cfcfcf>${cost}"
        ]
      },
      "alt": {
        "icon": "minecraft:ender_pearl",
        "text": [
          "<dark_gray><lang:'text.graves.gui.teleport'>",
          "<white><lang:'text.graves.gui.cost'> <#cfcfcf>${cost} <gray>(<red><lang:'text.graves.gui.cost.not_enough'></red>)"
        ]
      }
    },
    "back_button": {
      "icon": "minecraft:structure_void",
      "text": "<red><lang:'text.graves.gui.quick_pickup'>"
    },
    "bar": {
      "icon": "minecraft:white_stained_glass_pane",
      "text": ""
    }
  },
  "text": {
    "grave_created": "<white><lang:'text.graves.created_at':'<yellow>${position}':'<gray>${world}':'<red>${break_time}'>",
    "protection_ended": "<red><lang:'text.graves.no_longer_protected':'<gold>${position}':'<white>${world}':'<yellow>${item_count}'>",
    "grave_expired": "<red><lang:'text.graves.expired':'<gold>${position}':'<white>${world}':'<yellow>${item_count}'>",
    "grave_broken": "<gray><lang:'text.graves.somebody_broke':'<white>${position}':'<white>${world}':'<white>${item_count}'>",
    "grave_access_payment_no_access": "<red><lang:'text.graves.grave_unlock_payment.no_access'>",
    "grave_payment_accepted": "<white><lang:'text.graves.grave_unlock_payment.accepted'>",
    "grave_payment_failed": "<red><lang:'text.graves.grave_unlock_payment.failed':'<yellow>${cost}'>",
    "years_suffix": "y",
    "days_suffix": "d",
    "hours_suffix": "h",
    "minutes_suffix": "m",
    "seconds_suffix": "s",
    "infinity": "∞",
    "date_format": "dd.MM.yyyy, HH:mm",
    "world_names": {}
  }
}
"##;

/// The heads the menus show (`IconItem.Texture`): id and skin texture value.
pub(super) const TEXTURES: &[(&str, &str)] = &[
    ("tater", "ewogICJ0aW1lc3RhbXAiIDogMTYwNjIyODAxMzY0NCwKICAicHJvZmlsZUlkIiA6ICJiMGQ0YjI4YmMxZDc0ODg5YWYwZTg2NjFjZWU5NmFhYiIsCiAgInByb2ZpbGVOYW1lIiA6ICJNaW5lU2tpbl9vcmciLAogICJzaWduYXR1cmVSZXF1aXJlZCIgOiB0cnVlLAogICJ0ZXh0dXJlcyIgOiB7CiAgICAiU0tJTiIgOiB7CiAgICAgICJ1cmwiIDogImh0dHA6Ly90ZXh0dXJlcy5taW5lY3JhZnQubmV0L3RleHR1cmUvNTczNTE0YTIzMjQ1ZjE1ZGJhZDVmYjRlNjIyMTYzMDIwODY0Y2NlNGMxNWQ1NmRlM2FkYjkwZmE1YTcxMzdmZCIKICAgIH0KICB9Cn0="),
    ("next_page", "eyJ0ZXh0dXJlcyI6eyJTS0lOIjp7InVybCI6Imh0dHA6Ly90ZXh0dXJlcy5taW5lY3JhZnQubmV0L3RleHR1cmUvYzg2MTg1YjFkNTE5YWRlNTg1ZjE4NGMzNGYzZjNlMjBiYjY0MWRlYjg3OWU4MTM3OGU0ZWFmMjA5Mjg3In19fQ"),
    ("next_page_blocked", "ewogICJ0aW1lc3RhbXAiIDogMTY0MDYxNjExMDQ4OCwKICAicHJvZmlsZUlkIiA6ICIxZjEyNTNhYTVkYTQ0ZjU5YWU1YWI1NmFhZjRlNTYxNyIsCiAgInByb2ZpbGVOYW1lIiA6ICJOb3RNaUt5IiwKICAic2lnbmF0dXJlUmVxdWlyZWQiIDogdHJ1ZSwKICAidGV4dHVyZXMiIDogewogICAgIlNLSU4iIDogewogICAgICAidXJsIiA6ICJodHRwOi8vdGV4dHVyZXMubWluZWNyYWZ0Lm5ldC90ZXh0dXJlLzdlNTc3MjBhNDg3OGM4YmNhYjBlOWM5YzQ3ZDllNTUxMjhjY2Q3N2JhMzQ0NWE1NGE5MWUzZTFlMWEyNzM1NmUiLAogICAgICAibWV0YWRhdGEiIDogewogICAgICAgICJtb2RlbCIgOiAic2xpbSIKICAgICAgfQogICAgfQogIH0KfQ=="),
    ("previous_page", "eyJ0ZXh0dXJlcyI6eyJTS0lOIjp7InVybCI6Imh0dHA6Ly90ZXh0dXJlcy5taW5lY3JhZnQubmV0L3RleHR1cmUvMzEwODI5OGZmMmIyNjk1MWQ2ODNlNWFkZTQ2YTQyZTkwYzJmN2M3ZGQ0MWJhYTkwOGJjNTg1MmY4YzMyZTU4MyJ9fX0"),
    ("previous_page_blocked", "ewogICJ0aW1lc3RhbXAiIDogMTY0MDYxNjE5MjE0MiwKICAicHJvZmlsZUlkIiA6ICJmMjc0YzRkNjI1MDQ0ZTQxOGVmYmYwNmM3NWIyMDIxMyIsCiAgInByb2ZpbGVOYW1lIiA6ICJIeXBpZ3NlbCIsCiAgInNpZ25hdHVyZVJlcXVpcmVkIiA6IHRydWUsCiAgInRleHR1cmVzIiA6IHsKICAgICJTS0lOIiA6IHsKICAgICAgInVybCIgOiAiaHR0cDovL3RleHR1cmVzLm1pbmVjcmFmdC5uZXQvdGV4dHVyZS81MDgyMGY3NmUzZTA0MWM3NWY3NmQwZjMwMTIzMmJkZjQ4MzIxYjUzNGZlNmE4NTljY2I4NzNkMjk4MWE5NjIzIiwKICAgICAgIm1ldGFkYXRhIiA6IHsKICAgICAgICAibW9kZWwiIDogInNsaW0iCiAgICAgIH0KICAgIH0KICB9Cn0="),
    ("quick_pickup", "eyJ0ZXh0dXJlcyI6eyJTS0lOIjp7InVybCI6Imh0dHA6Ly90ZXh0dXJlcy5taW5lY3JhZnQubmV0L3RleHR1cmUvZDVjNmRjMmJiZjUxYzM2Y2ZjNzcxNDU4NWE2YTU2ODNlZjJiMTRkNDdkOGZmNzE0NjU0YTg5M2Y1ZGE2MjIifX19"),
    ("break_grave", "eyJ0ZXh0dXJlcyI6eyJTS0lOIjp7InVybCI6Imh0dHA6Ly90ZXh0dXJlcy5taW5lY3JhZnQubmV0L3RleHR1cmUvZGQ3NGQyOGQwOTdlNTNkYjExMTJlMzkwYTdkNGZmODJkZjcxODg2NmFlYThmMTY1MGQ5NDY2NTdhOTM2OTY5OSJ9fX0"),
    ("remove_protection", "eyJ0ZXh0dXJlcyI6eyJTS0lOIjp7InVybCI6Imh0dHA6Ly90ZXh0dXJlcy5taW5lY3JhZnQubmV0L3RleHR1cmUvODE5OWI1ZWUzMjBlNzk5N2Q5MWJiNWY4NjY1ZjNkMzJhZTQ5MjBlMDNjNmIzZDliN2VlY2E2OTcxMTk5OTcifX19"),
];
