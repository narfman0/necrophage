# QA Game Smoke-Test

Drives the running Necrophage game via `bevy_remote` (BRP) to smoke-test all major systems.

**Prerequisite:** The game must already be running with debug features:
```
cargo run -p necrophage
```

The BRP server listens on `http://localhost:15702`.

---

## Instructions

You are a QA agent. Run each step below in order using `curl` to call the BRP JSON-RPC API. After each call, assert the expected result and print `PASS` or `FAIL` with a brief reason. At the end, print a summary table.

Use this helper for all calls (replace METHOD and PARAMS):
```bash
curl -s -X POST http://localhost:15702 \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"METHOD","params":PARAMS,"id":1}'
```

---

## Test Sequence

### Step 0 — Verify game is running
Call `necrophage/state` with `params: null`.
- If you get "connection refused": print instructions and stop:
  ```
  Game not running. Start it with: cargo run -p necrophage
  ```
- Otherwise proceed.

### Step 1 — State baseline
Call `necrophage/state` with `params: null`.
Assert: `biomass == 0`, `tier == "Tiny"`, `quest_step == 0`.

### Step 2 — Biomass gain (stays Tiny)
Call `necrophage/command` with `params: {"command": "give biomass 10"}`.
Wait 200ms (one frame). Call `necrophage/state`.
Assert: `biomass == 10`, `tier == "Tiny"`.

### Step 3 — Tier flip to Small
Call `necrophage/command` with `params: {"command": "give biomass 1"}`.
Wait 200ms. Call `necrophage/state`.
Assert: `tier == "Small"`.

### Step 4 — Tier progression: Medium (biomass → 31+)
Call `necrophage/command` with `params: {"command": "give biomass 20"}`.
Wait 200ms. Call `necrophage/state`.
Assert: `tier == "Medium"`.

### Step 5 — Tier progression: Large (biomass → 76+)
Call `necrophage/command` with `params: {"command": "give biomass 45"}`.
Wait 200ms. Call `necrophage/state`.
Assert: `tier == "Large"`.

### Step 6 — HP mutation
Call `necrophage/command` with `params: {"command": "set_hp 1"}`.
Wait 200ms. Call `necrophage/state`.
Assert: `hp == 1.0`.

### Step 7 — Teleport
Call `necrophage/command` with `params: {"command": "teleport 5 5"}`.
Wait 200ms. Call `necrophage/state`.
Assert: `position.x == 5` AND `position.y == 5`.

### Step 8 — Kill all enemies
Call `necrophage/command` with `params: {"command": "kill_all enemies"}`.
Wait 200ms. Call `necrophage/entities` with `params: null`.
Assert: no entry in `entities` array has `"type": "Enemy"`.

### Step 9 — Quest advance × 3
Call `necrophage/command` with `params: {"command": "quest advance"}` three times (200ms between each).
Call `necrophage/state`.
Assert: `quest_step == 3`.

### Step 10 — Apex threshold
Call `necrophage/command` with `params: {"command": "give biomass 200"}`.
Wait 200ms. Call `necrophage/state`.
Assert: `tier == "Apex"`.

---

## Summary

After all steps, print a table:

```
Step | Description              | Result
-----|--------------------------|-------
0    | Game running             | PASS/FAIL
1    | State baseline           | PASS/FAIL
2    | Biomass gain (Tiny)      | PASS/FAIL
3    | Tier flip Small          | PASS/FAIL
4    | Tier Medium              | PASS/FAIL
5    | Tier Large               | PASS/FAIL
6    | HP mutation              | PASS/FAIL
7    | Teleport                 | PASS/FAIL
8    | Kill enemies             | PASS/FAIL
9    | Quest advance ×3         | PASS/FAIL
10   | Apex threshold           | PASS/FAIL
```

Final line: `X/11 tests passed`.
