---
title: Nightly tests failed
---

Tests failed on {{ date | date('YYYY-MM-DD') }}.

(a future version of this could give a link to the failing tests...)

[Here's the job that
failed]({{ github.server_url }}/${{ github.repository }}/actions/runs/${{ github.run_id }})
