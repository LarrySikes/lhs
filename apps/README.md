# Day planner (LHS CLI)

Port of the IWBasic **Perpetual Day Planner** feature set (no GUI).

| Area | Commands |
|------|----------|
| Views | `day` `week` `month` `prev` `next` `today` `goto` `sched` |
| Appointments | `aadd` `aupd` `adel` |
| Undated tasks | `tasks` `tadd` `tdone` `tdel` |
| Address book | `contacts` `cadd` `cdel` |
| Files | `save` `load` `print` `help` `quit` |

```bash
cargo run -p npc --bin lhsc -- run apps/day_planner.lhs
```

Data files: `/tmp/lhs_planner.dat`, `/tmp/lhs_contacts.dat`  
Non-interactive smoke: `examples/14_day_planner.lhs` (todos only).
