# Profile — Automated Performance Analysis

Runs the headless binary, captures frame-timing JSON, diagnoses slowness, and applies fixes.

---

## Instructions

You are a performance engineer. Execute every step below in order.

### Step 1 — Build check

```bash
cargo check --bin headless --features headless 2>&1
```

If there are compiler errors, fix them before continuing.

### Step 2 — Run headless profiler

Run 600 frames (10 seconds at 60 fps) and capture the JSON report:

```bash
cargo run --bin headless --features headless --release -- 600 2>/dev/null
```

Parse the `timing` block from the JSON output:
- `fps_avg` — average FPS
- `frame_ms_avg` — average frame time (ms)
- `frame_ms_p95` — 95th-percentile frame time (ms)
- `frame_ms_max` — worst single frame (ms)

Print a summary table:

```
Metric          | Value
----------------|-------
FPS (avg)       | X
Frame ms (avg)  | X
Frame ms (p95)  | X
Frame ms (max)  | X
Sample count    | X
```

### Step 3 — Verdict

Classify performance:
- `frame_ms_avg` ≤ 16.6 ms → **PASS** (60+ fps target met). Print pass and stop.
- `frame_ms_avg` 16.6–33.3 ms → **WARN** (30–60 fps). Investigate.
- `frame_ms_avg` > 33.3 ms → **FAIL** (< 30 fps). Investigate.

If PASS, stop here.

### Step 4 — Hot path analysis

Read the following source files that contain the game's most complex per-frame logic:

- `src/combat.rs`
- `src/movement.rs`
- `src/swarm.rs`
- `src/population.rs`
- `src/levels/generator.rs`

For each file, look for these Bevy performance anti-patterns:

1. **O(n²) entity iteration** — nested `for` loops over two queries
2. **Per-frame heap allocation** — `Vec::new()`, `HashMap::new()`, `.collect()` inside `Update` systems without caching
3. **Redundant query lookups** — calling `.get(entity)` inside a loop over another query
4. **Missing `Changed<>` / `Added<>` filters** — running expensive logic every frame when only needed on change
5. **Broad system ordering** — systems that block each other unnecessarily (check `.chain()` or `.before()`/`.after()` usage)
6. **Large components copied by value** — big structs passed as components instead of behind indirection

### Step 5 — Report findings

For each issue found:
- File and line number
- Anti-pattern type
- Specific code excerpt
- Suggested fix

### Step 6 — Apply fixes

For each issue:
1. Read the full function context.
2. Apply the minimal correct fix (don't refactor beyond what's needed).
3. After all edits, run:
   ```bash
   cargo check --bin headless --features headless 2>&1
   ```
   Fix any compiler errors introduced.

### Step 7 — Re-profile

Run Step 2 again with the same parameters. Compare before/after timing tables. Report the improvement (or lack thereof).

---

## Thresholds reference

| Target    | Frame budget |
|-----------|-------------|
| 60 fps    | 16.6 ms     |
| 30 fps    | 33.3 ms     |
| 20 fps    | 50.0 ms     |

p95 > 2× avg indicates frame spikes (hitching), even if avg looks fine.
