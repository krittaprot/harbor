(async () => {
  const { invoke, transformCallback } = window.__TAURI_INTERNALS__;
  const delay = ms => new Promise(resolve => setTimeout(resolve, ms));
  const assert = (value, message) => { if (!value) throw new Error(message); };
  const events = [];
  const violations = [];
  document.addEventListener("securitypolicyviolation", e => violations.push({ directive: e.violatedDirective, uri: e.blockedURI }));
  try {
    // Exercise the same explicit action as the menu-bar Open Harbor button.
    await invoke("show_main");
    assert(await invoke("plugin:window|is_visible", { label: "main" }), "menu bar opens the dashboard");
    await invoke("plugin:event|listen", {
      event: "harbor://snapshot", target: { kind: "Any" },
      handler: transformCallback(e => events.push(e.payload)),
    });
    let first;
    for (let n = 0; n < 300 && !first; n++) {
      try { first = await invoke("get_snapshot"); } catch { await delay(100); }
    }
    assert(first, "initial snapshot did not arrive");
    assert(document.title === "Harbor", "HTML title");
    const nav = text => [...document.querySelectorAll('.h-nav')].find(b => b.textContent.includes(text));
    // A saved restart report can select Restart Readiness on startup.
    for (let n = 0; n < 100 && document.querySelector("h1")?.textContent !== "Services"; n++) {
      nav('Services')?.click();
      await delay(100);
    }
    assert(document.querySelector("h1")?.textContent === "Services", "rendered Services heading");
    const visual = async theme => {
      await delay(300);
      const css = Array.from(document.styleSheets).map(sheet => Array.from(sheet.cssRules).map(rule => rule.cssText).join('\n')).join('\n');
      const html = `<!doctype html><html class="${document.documentElement.className}"><head><meta charset="utf-8"><style>${css}</style></head><body>${document.body.innerHTML}</body></html>`;
      await invoke("plugin:event|emit", {event:"harbor://smoke-dom", payload:{stage:theme,html}});
      await invoke("plugin:event|emit", {event:"harbor://smoke-stage", payload:theme});
      await delay(1800);
    };
    document.documentElement.classList.remove('dark');
    await visual('light');
    document.documentElement.classList.add('dark');
    await visual('dark');
    document.documentElement.classList.remove('dark');
    const jobs = await invoke("list_jobs");
    assert(jobs.length > 0, "upstream job listing");
    const row = document.querySelector('.h-job:not([disabled])');
    assert(row, "rendered grouped job button");
    const label = row.title.split(" · ")[0];
    row.click(); // Only opens the existing read-only detail panel.
    for (let n = 0; n < 100 && !document.querySelector('[role="dialog"]')?.textContent.includes(label); n++) await delay(100);
    assert(document.querySelector('[role="dialog"]')?.textContent.includes(label), "rendered job details");
    const job = jobs.find(job => job.label === label);
    const detail = await invoke("get_job_detail", { plistPath: job.plist_path });
    assert(detail.label === label, "upstream details IPC");
    [...document.querySelectorAll('[role="dialog"] button')].find(b => b.textContent.trim() === 'Close')?.click();
    await delay(600);
    assert(!document.querySelector('[role="dialog"][data-state="open"]'), 'job detail closes: ' + [...document.querySelectorAll('[role="dialog"]')].map(d => d.getAttribute('data-state') + ':' + [...d.querySelectorAll('button')].map(b=>b.textContent.trim()).join('|')).join(';'));
    const search = document.querySelector('input[aria-label="Search services"]');
    const setter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value').set;
    setter.call(search, '5432'); search.dispatchEvent(new Event('input', { bubbles: true }));
    await delay(250);
    assert(document.querySelector('[data-service="crystal-blood"]'), "search by port");
    setter.call(search, ''); search.dispatchEvent(new Event('input', { bubbles: true }));
    await delay(250);
    nav('Restart Readiness').click();
    await delay(350);
    assert(document.querySelector('h1')?.textContent === 'Restart Readiness', 'restart navigation');
    await visual('restart');
    nav('Services').click();
    await delay(350);
    const adopt = [...document.querySelectorAll('.h-unmanaged-row button')].find(b => b.textContent === 'Adopt…');
    if (adopt) {
      adopt.click(); await delay(1500);
      assert(document.querySelector('.h-adopt-sheet'), 'adopt sheet opens');
      await visual('adopt');
      [...document.querySelectorAll('.h-adopt-sheet button')].find(b => b.textContent === 'Cancel')?.click();
    }
    nav('Open at Login').click();
    await delay(400);
    for (let n = 0; n < 320 && document.querySelector('.h-loading')?.textContent.includes('Reading login'); n++) await delay(100);
    assert(document.querySelector('h1')?.textContent === 'Open at Login', 'login navigation');
    await visual('login');
    const loginResult = document.querySelector('.h-page-content [role="alert"]')?.textContent ?? 'Login list rendered';
    nav('Services').click();
    await delay(300);
    nav('Settings').click();
    await delay(300);
    assert(document.querySelector('h1')?.textContent === 'Settings', 'settings navigation');
    assert(document.querySelector('#harbor-updates-title')?.textContent === 'Software updates', 'native updates card');
    const updaterStatus = await invoke('update_status');
    assert(updaterStatus.current_version && !updaterStatus.can_install, 'smoke builds can check but cannot install');
    assert([...document.querySelectorAll('button')].some(b => b.textContent === 'Check for Updates'), 'native update check button');
    await visual('settings');
    nav('Services').click();
    await delay(300);
    await invoke('plugin:event|emit', {event:'harbor://smoke-popover',payload:null});
    await delay(700);
    assert(document.documentElement.scrollWidth <= innerWidth, "no horizontal page overflow");
    const crystal = first.services.find(s => s.id === 'crystal-blood');
    assert(crystal?.processes.some(p => p.name.includes('postgres')), "crystal-blood descendant attribution");
    assert(first.services.find(s => s.id === 'fasting')?.jobs.length >= 4, "fasting job grouping");
    for (let n = 0; n < 200 && !events.some(s => s.taken_at > first.taken_at); n++) await delay(100);
    let second = await invoke("get_snapshot");
    assert(second.taken_at > first.taken_at, "periodic scan/cache refresh");
    assert(events.some(s => s.taken_at === second.taken_at), "snapshot event/cache agree");
    const refreshed = await invoke("rescan_now");
    assert(refreshed.taken_at > second.taken_at, "explicit Rescan returns a fresh snapshot");
    second = refreshed;
    assert(second.processes.some(p => p.cpu !== null), "CPU warmed across scans");
    assert(new Set(second.ports.map(p => p.port)).size === second.listening_port_count, "port count");
    assert(violations.length === 0, "CSP violation");
    let adoption = null;
    if (window.__HARBOR_FIXTURE_PID__) {
      const target = second.unmanaged.find(g => g.copies.some(p => p.pid === window.__HARBOR_FIXTURE_PID__));
      assert(target && target.copies.length === 2, `owned fixture group has two copies (found ${target?.copies.length ?? 0})`);
      const options = {run_at_load:true,keep_alive:false,logs:true,stop_copies:true};
      const label = `com.harbor.smoke.${window.__HARBOR_FIXTURE_PID__}`;
      const expectedXml = await invoke('adopt_preview', {key:target.key,label,options});
      // Leave an identical, unloaded plist behind, like a retained failed attempt.
      // Only this uniquely named, test-owned agent is loaded/stopped here.
      const initialChannel = transformCallback(() => {});
      await invoke('adopt_apply', {key:target.key,label,options:{...options,stop_copies:false},expectedXml,onProgress:`__CHANNEL__:${initialChannel}`});
      window.__TAURI_INTERNALS__.unregisterCallback(initialChannel);
      const fixtureJob = (await invoke('list_jobs')).find(j => j.label === label);
      assert(fixtureJob, 'fixture plist exists after first adoption');
      await invoke('stop_job', {plistPath:fixtureJob.plist_path});
      assert(await invoke('adopt_preview', {key:target.key,label,options}) === expectedXml, 'matching retained plist is previewable');
      const progress = [];
      const channel = transformCallback(event => { if (event.message) progress.push(event.message); });
      adoption = await invoke('adopt_apply', {key:target.key,label,options,expectedXml,onProgress:`__CHANNEL__:${channel}`});
      for (let n = 0; n < 20 && !progress.some(p => p.stage === 'complete'); n++) await delay(50);
      window.__TAURI_INTERNALS__.unregisterCallback(channel);
      for (const stage of ['verify','write','load','ready','stop','complete']) assert(progress.some(p => p.stage === stage), `adoption progress: ${stage}`);
      assert(progress.findIndex(p => p.stage === 'ready') < progress.findIndex(p => p.stage === 'stop'), 'readiness before stopping originals');
      assert(progress.some(p => p.message.includes('Reusing the identical retained plist')), 'retry reused the matching plist');
      const diagnosis = await invoke('diagnose_target', {target:label});
      assert(diagnosis.jobs.some(j => j.label === label) && diagnosis.last_adoption.results.length > 0, 'native diagnostic report includes saved adoption');
      assert(adoption.filter(line => line.includes('exited')).length === 2, 'both originals stopped after readiness');
      second = await invoke('rescan_now');
      assert(second.services.some(s => s.jobs.some(j => j.label === label) && s.ports.length > 0 && !s.apple), 'new agent visible as a managed service');
    }
    await invoke("plugin:event|emit", { event: "harbor://smoke-result", payload: {
      ok: true, job_count: jobs.length, detail_label: label, events: events.length,
      first_taken_at: first.taken_at, second_taken_at: second.taken_at,
      first_cpu_samples: first.processes.filter(p => p.cpu !== null).length,
      second_cpu_samples: second.processes.filter(p => p.cpu !== null).length,
      process_count: second.processes.length, ports: second.ports,
      listening_port_count: second.listening_port_count, diagnostics: second.diagnostics,
      csp_violations: violations,
      services: second.services.filter(s => !s.apple).map(s => ({id:s.id, kind:s.kind, jobs:s.jobs.map(j=>j.label), processes:s.processes.length, ports:s.ports.map(p=>p.port), health:s.health, state:s.state})),
      unmanaged: second.unmanaged.map(g => ({name:g.name, copies:g.copies.length, verdict:g.verdict})),
      system: second.system,
      updater: updaterStatus,
      adoption,
      login: loginResult,
    }});
  } catch (error) {
    await invoke("plugin:event|emit", { event: "harbor://smoke-result", payload: { ok: false, error: String(error), violations } });
  }
})();
