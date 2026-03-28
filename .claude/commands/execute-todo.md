# Execute TODO

Work through every actionable item in `docs/TODO.md` sequentially without stopping for input.

## Process (repeat until the TODO list is empty or only deferred items remain)

1. **Read** `docs/TODO.md` and pick the next concrete, implementable item. Skip items that are explicitly complex/deferred (e.g. "ParasiteSubtype", "Door interaction", "Liberator lerp") unless they are the only items left.

2. **Investigate** — read all source files relevant to the item before writing any code. Understand the current state; never modify code you haven't read.

3. **Implement** the item:
   - Keep changes focused: one concern per system, no speculative abstractions.
   - Add `#[cfg(test)]` unit tests for any pure logic introduced.
   - Follow all rules in `CLAUDE.md` (no `cd`, no `rand::thread_rng()`, gate new systems with `run_if(in_state(GameState::Playing))` where appropriate, `LevelEntity` on level-scoped spawns, etc.).

4. **Build & test**:
   ```bash
   cargo check --manifest-path C:/workspace/necrophage/Cargo.toml
   cargo check --manifest-path C:/workspace/necrophage/Cargo.toml --features debug
   cargo test --manifest-path C:/workspace/necrophage/Cargo.toml -p necrophage-core
   ```
   Fix all errors and warnings before proceeding. Zero errors, zero warnings is the bar.

5. **Remove the item** from `docs/TODO.md` (delete the checkbox line). If a sub-item was only partially addressed, update the wording to reflect what remains.

6. **Commit and push**:
   ```bash
   git -C C:/workspace/necrophage add -A
   git -C C:/workspace/necrophage commit -m "<concise summary>\n\nCo-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>"
   git -C C:/workspace/necrophage push origin master
   ```

7. **Loop** — go back to step 1 with the updated TODO.

## Notes

- Tackle simpler, higher-value items before complex ones.
- Items in the **MVP Vertical Slice** checklist require a running game and cannot be automated — skip them.
- Items tagged as "complex" or "out of scope" in the TODO comments may be deferred; note them and move on.
- After finishing all feasible items, update `docs/PRODUCT_PLAN.md` and `docs/IMPLEMENTATION_PLAN.md` to reflect the new state, then do a final commit.
