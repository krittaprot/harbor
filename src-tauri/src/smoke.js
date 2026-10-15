(async () => {
  const { invoke, transformCallback } = window.__TAURI_INTERNALS__;
  const delay = ms => new Promise(resolve => setTimeout(resolve, ms));
  const assert = (value, message) => { if (!value) throw new Error(message); };
  const events = [];
  const violations = [];
  document.addEventListener("securitypolicyviolation", e => violations.push({ directive: e.violatedDirective, uri: e.blockedURI }));
  try {
    await invoke("plugin:event|listen", {
      event: "harbor://snapshot", target: { kind: "Any" },
      handler: transformCallback(e => events.push(e.payload)),
    });
    let first;
    for (let n = 0; n < 100 && !first; n++) {
      try { first = await invoke("get_snapshot"); } catch { await delay(100); }
    }
    assert(first, "initial snapshot did not arrive");
    assert(document.title === "Harbor", "HTML title");
    assert(document.querySelector("h1")?.textContent === "Harbor", "rendered heading");
    const jobs = await invoke("list_jobs");
    assert(jobs.length > 0, "upstream job listing");
    for (let n = 0; n < 100 && !document.querySelector("tbody tr.cursor-pointer"); n++) await delay(100);
    const row = document.querySelector("tbody tr.cursor-pointer");
    assert(row, "rendered job row");
    const label = row.querySelector("td").textContent;
    row.click(); // Only opens the existing read-only detail panel.
    for (let n = 0; n < 100 && !document.querySelector('[role="dialog"]')?.textContent.includes(label); n++) await delay(100);
    assert(document.querySelector('[role="dialog"]')?.textContent.includes(label), "rendered job details");
    const job = jobs.find(job => job.label === label);
    const detail = await invoke("get_job_detail", { plistPath: job.plist_path });
    assert(detail.label === label, "upstream details IPC");
    for (let n = 0; n < 200 && !events.some(s => s.taken_at > first.taken_at); n++) await delay(100);
    const second = await invoke("get_snapshot");
    assert(second.taken_at > first.taken_at, "periodic scan/cache refresh");
    assert(events.some(s => s.taken_at === second.taken_at), "snapshot event/cache agree");
    assert(second.processes.some(p => p.cpu !== null), "CPU warmed across scans");
    assert(new Set(second.ports.map(p => p.port)).size === second.listening_port_count, "port count");
    assert(violations.length === 0, "CSP violation");
    await invoke("plugin:event|emit", { event: "harbor://smoke-result", payload: {
      ok: true, job_count: jobs.length, detail_label: label, events: events.length,
      first_taken_at: first.taken_at, second_taken_at: second.taken_at,
      first_cpu_samples: first.processes.filter(p => p.cpu !== null).length,
      second_cpu_samples: second.processes.filter(p => p.cpu !== null).length,
      process_count: second.processes.length, ports: second.ports,
      listening_port_count: second.listening_port_count, diagnostics: second.diagnostics,
      csp_violations: violations,
    }});
  } catch (error) {
    await invoke("plugin:event|emit", { event: "harbor://smoke-result", payload: { ok: false, error: String(error), violations } });
  }
})();
