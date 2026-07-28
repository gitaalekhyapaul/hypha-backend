# SP1 Plonk Benchmark Records

This directory stores public, aggregate results from successful ADR-0012 proof runs.

`scripts/cluster/render-sp1-plonk-result` creates each record from an ignored local
log. It copies a fixed field allowlist. It rejects runs with swap use, OOM events,
source changes, missing proof gates, or missing mutation checks.

Do not add raw proof files, public-journal files, witness data, setup files, or raw
scheduler logs.
