<script lang="ts">
  import { onMount } from "svelte";
  import { t, setLang, type Lang } from "./lib/i18n";
  import {
    call, inTauri, onProgress, openUrl, reveal, copyPng,
    type Found, type Connected, type UpdateInfo, type PlanView, type ApplyView, type DriftView, type RescueView, type ProblemView, type Progress,
  } from "./lib/api";

  type View = "connect" | "clean" | "check" | "finish" | "rescue";
  const steps: { id: View; title: string; sub: string }[] = [
    { id: "connect", title: "Connect", sub: "Find your TV" },
    { id: "clean", title: "Clean up", sub: "Switch off the bloat" },
    { id: "check", title: "Check", sub: "Fix it after updates" },
    { id: "finish", title: "Finish", sub: "Close debugging" },
  ];

  const shot = typeof location !== "undefined" && location.search.includes("shot");
  let lang: Lang = $state("en");
  let view: View = $state("connect");
  let done = $state(new Set<View>());
  let busy = $state("");
  let error = $state("");
  let log = $state<string[]>([]);
  let showLog = $state(false);

  // つなぐ
  let found = $state<Found[]>([]);
  let scanning = $state(false);
  let manual = $state("");
  let tv = $state<Connected | null>(null);
  // ペアリング（6 桁コード）
  let pairTv = $state<Found | null>(null);
  let pairIp = $state("");
  let pairPort = $state("");
  let digits = $state(["", "", "", "", "", ""]);
  let boxes: HTMLInputElement[] = $state([]);
  const code = $derived(digits.join(""));

  // 片付ける
  let plan = $state<PlanView | null>(null);
  let preset = $state<"light" | "full">("light");
  let picked = $state(new Set<string>());
  let tab = $state<"all" | "off" | "protected" | "already" | "missing">("all");
  let filter = $state("");
  let safe = $state(true);
  const batch = 4;
  let result = $state<ApplyView | null>(null);

  // 点検・直す・終わる
  let drift = $state<DriftView | null>(null);
  let extra = $state(new Set<string>());
  let rescue = $state<RescueView | null>(null);
  let how = $state<"quick" | "record" | "all">("quick");
  let after = $state<ProblemView[] | null>(null);
  let closed = $state<boolean | null>(null);
  let notice = $state("");

  // 自動アップデート
  let update = $state<UpdateInfo | null>(null);
  let showUpdate = $state(false);
  const whatsNew = $derived(
    ((update?.notes ?? "").split("---ja---")[String(lang) === "ja" && (update?.notes ?? "").includes("---ja---") ? 1 : 0] ?? "").split("\n").filter((l) => l.startsWith("- ")).map((l) => l.slice(2).replace(/\*\*|`/g, "").replace(/\s*https?:\/\/\S+/g, "")).slice(0, 5),
  );

  async function checkUpdate() {
    try {
      update = await call<UpdateInfo | null>("update_check");
    } catch (e) {
      addLog(`update: ${e}`);
    }
  }

  async function installUpdate() {
    showUpdate = false;
    await run(t("Updating… hushtv restarts when it's done."), () => call<void>("update_install"));
  }

  function addLog(s: string) {
    log = [...log.slice(-200), `${new Date().toLocaleTimeString()}  ${s}`];
  }

  async function run<T>(label: string, f: () => Promise<T>): Promise<T | undefined> {
    busy = label;
    error = "";
    addLog(label);
    try {
      return await f();
    } catch (e) {
      error = String(e);
      addLog(`! ${error}`);
    } finally {
      busy = "";
    }
  }

  onMount(() => {
    let un: (() => void) | undefined;
    (async () => {
    lang = (await call<string>("ui_lang")) as Lang;
    setLang(lang);
    un = await onProgress((p: Progress) => {
      const m: Record<string, string> = {
        checking: t("Checking the safety rules…"), saving: t("Saving the restore point…"),
        shot_before: t("Taking a home screen picture…"), shot_after: t("Taking a home screen picture…"),
        read_back: t("Reading the TV back…"),
      };
      busy = p.kind === "batch" ? t("Safe mode: batch {d} of {t}", { d: p.done ?? 0, t: p.total ?? 0 })
        : p.kind === "package" ? `${p.done}/${p.total} ${p.name}` : (m[p.kind] ?? busy);
      addLog(busy);
    });
    scan();
    checkUpdate();
    })();
    return () => un?.();
  });

  function switchLang(l: Lang) {
    lang = l;
    setLang(l);
    if (plan) loadPlan(plan.list_id);
    if (rescue) diagnose();
  }

  async function scan() {
    scanning = true;
    found = (await run(t("Searching…"), () => call<Found[]>("scan", { seconds: 4 }))) ?? found;
    scanning = false;
  }

  async function connect(host: string) {
    const c = await run(t("Connecting… Check the TV for \"Allow USB debugging?\""), () => call<Connected>("connect", { host }));
    if (!c) return;
    tv = c;
    done = new Set([...done, "connect"]);
    if (c.lists[0]) await loadPlan(c.lists[0].id);
    view = "clean";
  }

  function pickTv(f: Found) {
    if (f.legacy_open) return connect(`${f.address}:5555`);
    pairTv = f;
    pairIp = f.address;
    pairPort = f.pair_port ? String(f.pair_port) : "";
    digits = ["", "", "", "", "", ""];
    boxes[0]?.focus();
  }

  function typeDigit(i: number, e: Event) {
    const el = e.target as HTMLInputElement;
    const v = el.value.replace(/\D/g, "");
    if (v.length > 1) {
      // 6 桁をまとめて貼り付けたとき
      const all = v.slice(0, 6).split("");
      digits = [0, 1, 2, 3, 4, 5].map((k) => all[k] ?? "");
      boxes[Math.min(all.length, 5)]?.focus();
      return;
    }
    digits[i] = v;
    el.value = v;
    if (v && i < 5) boxes[i + 1]?.focus();
  }

  function keyDigit(i: number, e: KeyboardEvent) {
    if (e.key === "Backspace" && !digits[i] && i > 0) boxes[i - 1]?.focus();
    if (e.key === "Enter" && code.length === 6) pairConnect();
  }

  async function pairConnect() {
    const port = Number(pairPort);
    if (!pairIp || !port || code.length !== 6) return;
    const c = await run(t("Pairing…"), () =>
      call<Connected>("pair_connect", { address: pairIp.trim(), pairPort: port, code, tlsPort: pairTv?.address === pairIp.trim() ? pairTv?.tls_port ?? null : null, lang }));
    if (!c) return;
    tv = c;
    done = new Set([...done, "connect"]);
    if (c.lists[0]) await loadPlan(c.lists[0].id);
    view = "clean";
  }

  async function loadPlan(listId: string) {
    const p = await run(t("Working…"), () => call<PlanView>("make_plan", { listId, lang }));
    if (!p) return;
    plan = p;
    choosePreset(preset);
  }

  function choosePreset(p: "light" | "full") {
    preset = p;
    if (!plan) return;
    picked = new Set(plan.rows.filter((r) => r.state === "on" && !r.protected && !r.keep && (p === "full" || r.light)).map((r) => r.package));
  }

  function toggle(pkg: string) {
    const s = new Set(picked);
    s.has(pkg) ? s.delete(pkg) : s.add(pkg);
    picked = s;
  }

  const counts = $derived.by(() => {
    const rows = plan?.rows ?? [];
    return {
      off: picked.size,
      already: rows.filter((r) => r.state === "off").length,
      protected: rows.filter((r) => r.protected).length,
      missing: rows.filter((r) => r.state === "absent").length,
      all: rows.length,
    };
  });

  const visible = $derived.by(() => {
    const rows = plan?.rows ?? [];
    const f = filter.trim().toLowerCase();
    return rows.filter((r) => {
      if (f && !r.package.toLowerCase().includes(f)) return false;
      if (tab === "off") return picked.has(r.package);
      if (tab === "protected") return !!r.protected;
      if (tab === "already") return r.state === "off";
      if (tab === "missing") return r.state === "absent";
      return true;
    });
  });

  async function apply() {
    if (!plan) return;
    const r = await run(t("Working…"), () => call<ApplyView>("apply", { listId: plan!.list_id, selected: [...picked], safe, batch, lang }));
    if (!r) return;
    result = r;
    done = new Set([...done, "clean"]);
    addLog(`${t("Switched off")}: ${r.disabled}`);
    view = "check";
    loadDrift();
  }

  async function loadDrift() {
    drift = (await run(t("Working…"), () => call<DriftView>("check"))) ?? null;
    extra = new Set();
  }

  async function fixDrift() {
    await run(t("Working…"), () => call<number>("fix_drift", { extra: [...extra] }));
    await loadDrift();
  }

  async function diagnose() {
    rescue = (await run(t("Working…"), () => call<RescueView>("rescue_diagnose", { lang }))) ?? null;
    after = null;
  }

  async function repair() {
    after = (await run(t("Working…"), () => call<ProblemView[]>("rescue_run", { how, lang }))) ?? null;
    if (after) await diagnose();
  }

  async function undoAll() {
    const n = await run(t("Working…"), () => call<number>("restore"));
    if (n !== undefined) notice = t("Undone. Reboot the TV to finish.");
  }

  async function closeDebug() {
    closed = (await run(t("Working…"), () => call<boolean>("close_debugging"))) ?? null;
    if (closed) tv = null;
  }

  async function saveCard() {
    if (!result?.card_path) return;
    const p = await run(t("Working…"), () => call<string>("save_card", { path: result!.card_path }));
    if (p) {
      notice = t("Saved to {p}", { p });
      reveal(p);
    }
  }

  async function copyCard() {
    if (result?.card_png) {
      await copyPng(result.card_png);
      notice = t("Copied");
    }
  }

  function postX() {
    const n = result?.disabled ?? 0;
    const text = `I switched off ${n} ads and trackers on my Android TV with hushtv. Nothing uninstalled, every change can be undone.`;
    openUrl(`https://x.com/intent/post?text=${encodeURIComponent(text)}&url=${encodeURIComponent("https://github.com/kajisho5/hushtv")}`);
  }

  function go(v: View) {
    if (v !== "connect" && v !== "rescue" && !tv && inTauri) {
      error = t("Connect to a TV first.");
      return;
    }
    error = "";
    notice = "";
    view = v;
    if (v === "check") loadDrift();
    if (v === "rescue" && tv) diagnose();
  }

  const stateLabel = (s: string) => (s === "on" ? t("On") : s === "off" ? t("Off") : t("Absent"));
</script>

{#key lang}
<div class="shell">
  <aside>
    <div class="brand">
      <div class="logo"><svg viewBox="0 0 256 256" width="30" height="30" aria-hidden="true"><rect x="40" y="62" width="176" height="118" rx="24" fill="none" stroke="var(--mint)" stroke-width="18"/><path d="M98 208h60" stroke="var(--mint)" stroke-width="18" stroke-linecap="round"/><path d="M74 121c12-26 24-26 36 0s22 18 32 0s18-8 26 0s12 2 16 0" stroke="var(--mint)" stroke-width="16" fill="none" stroke-linecap="round" stroke-linejoin="round"/></svg></div>
      <div><div class="name">hushtv</div><div class="tag">{t("Quiet your Android TV")}</div></div>
    </div>
    <div class="tvpill">
      <span class="dot" class:on={!!tv}></span>
      <div>
        <div class="b">{tv ? (tv.model ?? tv.host) : t("Not connected")}</div>
        <div class="small {tv ? 'mono' : ''}">{tv ? tv.host : t("Choose a TV")}</div>
      </div>
    </div>
    <nav>
      {#each steps as s, i}
        <button class="step" class:active={view === s.id} onclick={() => go(s.id)}>
          <span class="num" class:done={done.has(s.id)}>{done.has(s.id) ? "✓" : i + 1}</span>
          <span><span class="st">{t(s.title)}</span><span class="ss">{t(s.sub)}</span></span>
        </button>
      {/each}
    </nav>
    <div class="spacer"></div>
    {#if update}
      <button class="updatecard" onclick={() => (showUpdate = true)} disabled={!!busy}>
        <span class="uicon">↓</span>
        <span><b>{t("New version {v}", { v: update.version })}</b><span class="mint small b">{t("Update")} →</span></span>
      </button>
    {/if}
    <button class="repair" class:active={view === "rescue"} onclick={() => go("rescue")}>⚠ {t("Repair a TV")}</button>
    <div class="langs">
      <button class:on={lang === "en"} onclick={() => switchLang("en")}>English</button>
      <button class:on={lang === "ja"} onclick={() => switchLang("ja")}>日本語</button>
    </div>
    <div class="foot">{t("Nothing is uninstalled.")}<br />{t("Every change can be undone.")}<div class="mono dim ver">hushtv v0.2.0-beta.6 · MIT</div></div>
  </aside>

  {#if showUpdate && update}
    <div class="overlay" role="presentation" onclick={(e) => { if (e.target === e.currentTarget) showUpdate = false; }}>
      <div class="dialog" role="dialog" aria-modal="true" aria-labelledby="uptitle">
        <div class="dhead">
          <span class="uicon big">↓</span>
          <div><h2 id="uptitle">{t("Update to hushtv {v}", { v: update.version })}</h2><div class="sub small">{t("You have {v}", { v: update.current })}</div></div>
        </div>
        {#if whatsNew.length}
          <div class="sub small b">{t("What's new")}</div>
          <ul class="news">{#each whatsNew as n}<li>{n}</li>{/each}</ul>
        {/if}
        <div class="infobox sub small">{t("Your TV settings and restore points stay as they are.")}<br />{t("The update is checked against the developer's signature first.")}</div>
        <div class="dbtns">
          <button onclick={() => (showUpdate = false)}>{t("Later")}</button>
          <button class="primary" onclick={installUpdate} disabled={!!busy}>{t("Update and restart")}</button>
        </div>
      </div>
    </div>
  {/if}

  <main>
    {#if !inTauri && !shot}<div class="demo">{t("Demo mode: no TV is connected, the data is made up.")}</div>{/if}
    {#if error}<div class="err" role="alert">{error}</div>{/if}
    {#if notice}<div class="okmsg" role="status">{notice}</div>{/if}

    {#if view === "connect"}
      <h1>{t("Connect to your TV")}</h1>
      <p class="sub">{t("Looking for TVs on your Wi-Fi. If yours isn't listed, type its IP address.")}</p>
      <div class="grid2">
        <section class="panel">
          <div class="row"><h2>{t("TVs found")} <span class="sub">{found.length}</span></h2><button onclick={scan} disabled={scanning}>{scanning ? t("Searching…") : t("Rescan")}</button></div>
          {#if found.length === 0}<p class="dim">{t("Nothing found yet.")}</p>{/if}
          {#each found as f}
            <button class="tvrow" class:sel={pairTv?.address === f.address} disabled={!!busy || (!f.legacy_open && !f.tls_port && !f.pair_port)} onclick={() => pickTv(f)}>
              <span class="tvname">{f.name ?? f.address}</span>
              <span class="mono dim">{f.address}</span>
              <span class="chip">{f.legacy_open ? t("Port 5555") : f.tls_port ? t("Wireless debugging") : t("Debugging off")}</span>
              <span class={f.legacy_open ? "okc" : "warnc"}>{f.legacy_open ? t("Ready") : f.tls_port ? t("Needs pairing") : t("Needs a TV setting")}</span>
            </button>
          {/each}
          <div class="manual">
            <span class="sub">{t("Not listed?")}</span>
            <input type="text" class="mono" placeholder="192.168.10.80:5555" bind:value={manual} />
            <button onclick={() => connect(manual)} disabled={!manual || !!busy}>{t("Connect")}</button>
          </div>
        </section>
        <section class="panel pair">
          <h2>{t("Pair")}</h2>
          {#if pairTv}
            <div class="mint b">{pairTv.name ?? pairTv.address}</div>
          {:else}
            <input type="text" class="mono" placeholder="192.168.10.23" aria-label={t("TV IP address")} bind:value={pairIp} />
          {/if}
          <p class="sub">{t("On the TV: Wireless debugging → Pair device with pairing code. Type the six digits it shows.")}</p>
          <div class="digits">
            {#each digits as d, i}
              <input bind:this={boxes[i]} value={d} inputmode="numeric" autocomplete="one-time-code" maxlength={i === 0 ? 6 : 1}
                aria-label={t("Digit {n}", { n: i + 1 })} oninput={(e) => typeDigit(i, e)} onkeydown={(e) => keyDigit(i, e)} />
            {/each}
          </div>
          <label class="sub small" for="pairport">{t("Pairing port")}</label>
          <div class="portrow">
            <input id="pairport" type="text" class="mono" inputmode="numeric" placeholder="37145" bind:value={pairPort} />
            {#if pairTv?.pair_port && String(pairTv.pair_port) === pairPort}<span class="dim small">{t("Filled in automatically")}</span>{/if}
          </div>
          <button class="primary wide" onclick={pairConnect} disabled={!!busy || code.length !== 6 || !pairIp || !Number(pairPort)}>{t("Pair and connect")}</button>
          <p class="sub small">{t("Google's adb is not used.")}<br />{t("The key is kept on this computer only.")}</p>
          {#if pairTv?.tls_port}
            <button class="link" onclick={() => connect(`${pairTv!.address}:${pairTv!.tls_port}`)} disabled={!!busy}>{t("Already paired? Connect")}</button>
          {/if}
        </section>
      </div>
      <section class="panel">
        <h2>{t("On the TV (first time only)")}</h2>
        <div class="three">
          <div><b>1 · {t("Unlock Developer options")}</b><p class="sub">{t("Settings → System → About → press Build 7 times")}</p></div>
          <div><b>2 · {t("Turn on debugging")}</b><p class="sub">{t("Developer options → USB debugging and Network debugging")}</p></div>
          <div><b>3 · {t("Allow this computer")}</b><p class="sub">{t("When the TV asks, choose Allow")}</p></div>
        </div>
        <div class="warnbar">{t("While debugging is on, devices on your Wi-Fi can control the TV. The last step turns it off again.")}</div>
      </section>
    {/if}

    {#if view === "clean"}
      <div class="row top">
        <div><h1>{t("Switch off the bloat ")}</h1><p class="sub">{t("Using the closest tested list")}: <b>{plan?.list_title ?? "-"}</b><br />{t("Apps are switched off, never uninstalled.")}</p></div>
        <div class="right">
          <div class="seg"><button class:on={preset === "light"} onclick={() => choosePreset("light")}>{t("Light: ads & tracking")}</button><button class:on={preset === "full"} onclick={() => choosePreset("full")}>{t("Full list")}</button></div>
          {#if tv && tv.lists.length > 1}
            <select aria-label={t("Change list")} onchange={(e) => loadPlan((e.target as HTMLSelectElement).value)}>
              {#each tv.lists as l}<option value={l.id} selected={l.id === plan?.list_id}>{l.title} · {Math.round(l.score * 100)}%</option>{/each}
            </select>
          {/if}
        </div>
      </div>
      <div class="stats">
        <div class="stat"><b class="mint">{counts.off}</b><span>{t("Switch off")}</span></div>
        <div class="stat"><b>{counts.already}</b><span>{t("Already off")}</span></div>
        <div class="stat"><b class="warn">{counts.protected}</b><span>{t("Protected")}</span></div>
        <div class="stat"><b class="dim">{counts.missing}</b><span>{t("Not on this TV")}</span></div>
      </div>
      <div class="infobar">🛡 {t("No restore point yet. Applying saves your TV's current state first, so you can always undo.")}</div>
      {#each plan?.notes ?? [] as n}<div class="warnbar">{n}</div>{/each}
      <div class="row tabs">
        <div>
          {#each [["all", `${t("All")} ${counts.all}`], ["off", `${t("Switch off")} ${counts.off}`], ["protected", `${t("Protected")} ${counts.protected}`], ["already", `${t("Already off")} ${counts.already}`], ["missing", `${t("Missing")} ${counts.missing}`]] as [k, label]}
            <button class="tab" class:on={tab === k} onclick={() => (tab = k as typeof tab)}>{label}</button>
          {/each}
        </div>
        <input type="search" placeholder={t("Filter apps")} bind:value={filter} />
      </div>
      <div class="table">
        <div class="thead"><span></span><span>{t("App")}</span><span>{t("Type")}</span><span>{t("On TV")}</span><span>{t("Why")}</span></div>
        {#each visible as r (r.package)}
          <label class="tr" class:prot={!!r.protected}>
            <input type="checkbox" checked={picked.has(r.package)} disabled={!!r.protected || r.state !== "on"} onchange={() => toggle(r.package)} />
            <span class="mono pkg">{r.package}</span>
            <span><span class="chip {r.protected ? 'protected' : r.category}">{r.protected ? t("Protected") : r.category_label}</span></span>
            <span class="sub">● {stateLabel(r.state)}</span>
            <span>
              {#if r.protected}<span class="warn">{r.protected}</span><br /><span class="dim small">{t("Safety check")}</span>
              {:else}{r.note ?? ""}<br /><span class="dim small">{r.keep ? t("The list's author keeps it on") : !r.light && preset === "light" && r.state === "on" ? t("Full list only · tick to add") : t("Off in {n} lists", { n: r.lists })}</span>{/if}
            </span>
          </label>
        {/each}
      </div>
    {/if}

    {#if view === "check"}
      <h1>{t("Check after an update")}</h1>
      <p class="sub">{t("Compares your TV with your last cleanup.")}</p>
      {#if drift && !drift.has_applied}<section class="panel"><p class="sub">{t("No cleanup recorded for this TV yet.")}</p></section>{/if}
      {#if drift?.has_applied}
        {@const n = drift.returned.length + drift.new_apps.length + (drift.home_changed ? 1 : 0)}
        {#if n === 0}<div class="okmsg">{t("No changes since your last cleanup.")}</div>
        {:else}
          <div class="warnbar big"><b>{t("{n} changes since your last cleanup", { n })}</b><button class="primary" onclick={fixDrift}>{t("Fix all")}</button></div>
          <div class="three">
            <section class="panel"><h2>{t("Came back")} <span class="danger">{drift.returned.length}</span></h2>{#each drift.returned as p}<div class="item mono">{p}<div class="sub small">{t("Turned on by the update")}</div></div>{/each}</section>
            <section class="panel"><h2>{t("New apps")} <span class="warn">{drift.new_apps.length}</span></h2>{#each drift.new_apps as p}<label class="item"><span class="mono">{p}</span><br /><input type="checkbox" checked={extra.has(p)} onchange={() => { const s = new Set(extra); s.has(p) ? s.delete(p) : s.add(p); extra = s; }} /> {t("Switch it off too")}</label>{/each}</section>
            <section class="panel"><h2>{t("Home screen")} <span class="warn">{drift.home_changed ? 1 : 0}</span></h2>{#if drift.home_changed}<div class="item mono">{drift.home_now}<div class="sub small">{t("Home went back to stock")}</div></div>{/if}</section>
          </div>
        {/if}
      {/if}
    {/if}

    {#if view === "finish"}
      <h1>{t("All done")}</h1>
      <div class="grid2">
        <section>
          {#if result?.card_png}<img class="card" alt="" src={`data:image/png;base64,${result.card_png}`} />{:else}<div class="card ph">hushtv · {result?.disabled ?? 0} apps switched off</div>{/if}
          <p class="dim small center">{t("Share image (1600×900)")}</p>
          <div class="btns"><button class="primary" onclick={saveCard} disabled={!result?.card_path}>{t("Save image")}</button><button onclick={copyCard} disabled={!result?.card_png}>{t("Copy image")}</button><button onclick={postX}>{t("Post to X")}</button></div>
          <p class="sub small">{t("Post to X opens your browser with the text ready. You decide whether to post.")}</p>
          <div class="stats three-s"><div class="stat"><b class="mint">{result?.disabled ?? 0}</b><span>{t("Switched off")}</span></div><div class="stat"><b class="warn">{result?.protected ?? 0}</b><span>{t("Protected")}</span></div><div class="stat"><b class="okc">{result?.problems ?? 0}</b><span>{t("Problems")}</span></div></div>
        </section>
        <section class="panel">
          <h2>{t("Before you go")}</h2>
          <p>✓ {t("Restore point saved")}<br /><span class="dim small mono">{result?.record_path ?? ""}</span></p>
          <p>{closed ? "✓" : "○"} {t("Turn off network debugging")}<br /><span class="dim small">{t("hushtv confirms the port is closed")}</span></p>
          <button class="primary wide" onclick={closeDebug} disabled={!tv}>{t("Close debugging")}</button>
          {#if closed === true}<p class="okc">{t("Debugging is off. The port is closed.")}</p>{/if}
          {#if closed === false}<p class="warn">{t("The port still answers. Turn debugging off in the TV's Developer options.")}</p>{/if}
          <hr />
          <p class="dim">{t("Changed your mind?")}</p>
          <button class="danger wide" onclick={undoAll} disabled={!tv}>{t("Undo everything")}</button>
        </section>
      </div>
    {/if}

    {#if view === "rescue"}
      <h1>{t("Repair a TV")}</h1>
      <p class="sub">{t("Diagnose a TV that misbehaves after a debloat with cleanyourtv, CleanMyTV or adb.")}<br />{t("Repairs only turn things back on. Nothing is switched off or removed.")}</p>
      {#if !tv && inTauri}<div class="warnbar">{t("Connect to a TV first.")} <button class="link" onclick={() => go("connect")}>{t("Connect")} →</button></div>{/if}
      <div class="grid2">
        <section class="panel">
          <h2>{t("Problems found")}</h2>
          {#if rescue && rescue.problems.length === 0}<p class="okc">{t("No problems found.")}</p>{/if}
          {#each rescue?.problems ?? [] as p}
            <div class="prob"><span class="badge s{p.severity >= 4 ? 'hi' : p.severity >= 2 ? 'mid' : 'lo'}">{p.severity >= 4 ? "!!" : p.severity >= 2 ? "!" : "–"}</span>{p.title}</div>
          {/each}
        </section>
        <section class="panel">
          <h2>{t("Choose a repair")}</h2>
          <label class="opt" class:on={how === "quick"}><input type="radio" bind:group={how} value="quick" /> <b>{t("Quick fix (recommended)")}</b><span class="sub">{t("Turns back on only what boot, Home, Settings, inputs and Wi-Fi need")}</span></label>
          <label class="opt" class:on={how === "record"}><input type="radio" bind:group={how} value="record" disabled={!rescue?.records.length} /> <b>{t("Replay a restore record")}</b><span class="sub">{rescue?.records.length ? (rescue.records.length === 1 ? t("1 record found") : t("{n} records found", { n: rescue.records.length })) : t("No record found")}</span></label>
          <label class="opt" class:on={how === "all"}><input type="radio" bind:group={how} value="all" /> <b>{t("Everything")}</b><span class="sub">{t("Turns every app the debloat lists switch off back on")}</span></label>
          <button class="primary wide" onclick={repair} disabled={(!tv && inTauri) || !!busy}>{t("Repair (turn back on)")}</button>
        </section>
      </div>
      <section class="panel">
        <h2>{t("If the TV can't be reached")}</h2>
        <div class="three">
          <div><b class="warn">{t("Wi-Fi is gone")}</b><p class="sub">{t("Plug in Ethernet, or re-join Wi-Fi in Settings")}</p></div>
          <div><b class="warn">{t("Black screen")}</b><p class="sub">{t("Open Settings with the remote, turn debugging on, try again")}</p></div>
          <div><b class="warn">{t("Stuck on the logo")}</b><p class="sub">{t("No tool can reach it. Factory reset from recovery (see the manual)")}</p></div>
        </div>
      </section>
    {/if}

    <footer>
      <button class="link sub" onclick={() => (showLog = !showLog)}>{showLog ? "▼" : "▲"} {t("Log")}</button>
      {#if view === "clean"}
        <button class="primary" onclick={apply} disabled={!picked.size || !!busy}>{t("Save restore point and apply ({n})", { n: picked.size })}</button>
        <label class="toggle"><input type="checkbox" bind:checked={safe} /> <span><b>{t("Safe mode")}</b><br /><span class="sub small">{t("{n} at a time, checked", { n: batch })}</span></span></label>
        <button class="link" onclick={() => go("check")}>{t("Next: {s}", { s: t("Check") })} →</button>
      {:else if view === "check"}
        <button onclick={loadDrift} disabled={!!busy}>{t("Check again")}</button>
        <button class="link" onclick={() => { done = new Set([...done, "check"]); go("finish"); }}>{t("Next: {s}", { s: t("Finish") })} →</button>
      {:else if view === "rescue"}
        <button onclick={diagnose} disabled={(!tv && inTauri) || !!busy}>{t("Diagnose")}</button>
        <span class="dim small">{t("Reboot the TV afterwards and check it")}</span>
      {:else if view === "finish"}
        <button class="link" onclick={() => { tv = null; plan = null; result = null; view = "connect"; scan(); }}>{t("Clean another TV")} →</button>
      {/if}
      {#if busy}<span class="busy" role="status">⏳ {busy}</span>{/if}
    </footer>
    {#if showLog}<pre class="log">{log.join("\n")}</pre>{/if}
  </main>
</div>
{/key}

<style>
  .shell { display: grid; grid-template-columns: 260px 1fr; height: 100vh; }
  aside { background: var(--side); border-right: 1px solid var(--border); padding: 24px 16px; display: flex; flex-direction: column; gap: 14px; }
  .brand { display: flex; gap: 12px; align-items: center; padding: 0 8px; }
  .logo { width: 40px; height: 40px; border-radius: 10px; background: var(--mintbg); display: grid; place-items: center; }
  .name { font-size: 20px; font-weight: 700; }
  .tag, .small { font-size: 11px; color: var(--sub); }
  .tvpill { display: flex; gap: 10px; align-items: center; background: var(--panel); border: 1px solid var(--border); border-radius: 12px; padding: 10px 14px; }
  .tvpill .b { font-weight: 600; }
  .dot { width: 10px; height: 10px; border-radius: 50%; background: var(--dim); flex: none; }
  .dot.on { background: var(--ok); }
  nav { display: flex; flex-direction: column; gap: 4px; }
  .step { display: flex; gap: 14px; align-items: center; height: 60px; border: none; text-align: left; padding: 0 12px; color: var(--sub); }
  .step.active { background: var(--panel2); color: var(--text); box-shadow: inset 4px 0 0 var(--mint); }
  .num { width: 26px; height: 26px; border-radius: 50%; border: 1.6px solid currentColor; display: grid; place-items: center; font-size: 12px; flex: none; }
  .num.done { background: var(--mint); border-color: var(--mint); color: var(--ink); }
  .st { display: block; font-size: 15px; font-weight: 600; }
  .ss { display: block; font-size: 11px; color: var(--dim); font-weight: 400; }
  .spacer { flex: 1; }
  .updatecard { display: flex; gap: 10px; align-items: center; text-align: left; background: var(--mintbg); border: 1px solid var(--mint); border-radius: 10px; padding: 10px 12px; height: auto; color: var(--text); }
  .updatecard b { display: block; font-size: 13px; }
  .uicon { width: 20px; height: 20px; border-radius: 50%; background: var(--mint); color: var(--ink); display: grid; place-items: center; font-weight: 800; font-size: 13px; flex: none; }
  .uicon.big { width: 36px; height: 36px; font-size: 18px; background: var(--mintbg); color: var(--mint); }
  .overlay { position: fixed; inset: 0; background: rgba(0, 0, 0, 0.55); display: grid; place-items: center; z-index: 50; }
  .dialog { width: 480px; background: var(--panel); border: 1px solid var(--border); border-radius: 16px; padding: 24px; display: flex; flex-direction: column; gap: 12px; }
  .dhead { display: flex; gap: 14px; align-items: center; }
  .dhead h2 { margin: 0; font-size: 20px; }
  .news { margin: 0; padding-left: 18px; line-height: 1.8; }
  .news li::marker { color: var(--mint); }
  .infobox { background: var(--panel2); border-radius: 10px; padding: 10px 14px; line-height: 1.7; }
  .dbtns { display: grid; grid-template-columns: 1fr 1.1fr; gap: 16px; margin-top: 8px; }
  .dbtns button { height: 44px; }
  .repair { border-color: var(--border); text-align: left; height: 44px; color: var(--sub); }
  .repair.active { border-color: var(--warn); color: var(--text); background: var(--panel2); }
  .langs { display: flex; border: 1px solid var(--border); border-radius: 8px; background: var(--panel); padding: 3px; }
  .langs button { flex: 1; height: 26px; border: none; font-size: 12px; color: var(--dim); font-weight: 400; }
  .langs button.on { background: var(--panel2); color: var(--text); font-weight: 700; }
  .foot { font-size: 12px; color: var(--sub); border-top: 1px solid var(--border); padding-top: 14px; line-height: 1.7; }
  .ver { margin-top: 8px; font-size: 11px; }
  main { overflow: auto; padding: 36px 40px 0; display: flex; flex-direction: column; gap: 14px; }
  .demo { background: var(--panel2); color: var(--sub); border-radius: 8px; padding: 6px 12px; font-size: 12px; }
  .err { background: var(--dangerbg); color: var(--danger); border-radius: 10px; padding: 10px 14px; }
  .okmsg { background: var(--mintbg); color: var(--mint); border-radius: 10px; padding: 10px 14px; }
  .row { display: flex; justify-content: space-between; align-items: center; gap: 12px; }
  .row.top { align-items: flex-start; }
  .right { display: flex; flex-direction: column; align-items: flex-end; gap: 8px; }
  select { background: var(--panel); color: var(--text); border: 1px solid var(--border); border-radius: 8px; height: 32px; max-width: 340px; font: inherit; }
  .grid2 { display: grid; grid-template-columns: 1.6fr 1fr; gap: 20px; }
  .three { display: grid; grid-template-columns: repeat(3, 1fr); gap: 16px; }
  .tvrow { display: grid; grid-template-columns: 1fr auto auto auto; gap: 12px; align-items: center; width: 100%; height: 56px; margin-bottom: 8px; border-color: var(--border); text-align: left; }
  .tvrow:hover { border-color: var(--mint); }
  .tvrow.sel { border-color: var(--mint); background: var(--panel2); }
  .pair { display: flex; flex-direction: column; gap: 10px; }
  .pair .b { font-weight: 600; font-size: 12px; }
  .digits { display: flex; gap: 8px; }
  .digits input { width: 44px; height: 56px; background: var(--bg); color: var(--text); border: 1px solid var(--border); border-radius: 10px; text-align: center; font-family: var(--mono); font-size: 24px; font-weight: 700; padding: 0; }
  .digits input:focus { border-color: var(--mint); outline: 1px solid var(--mint); }
  .portrow { display: flex; gap: 10px; align-items: center; }
  .portrow input { flex: 1; }
  .link { border: none; background: none; color: var(--mint); font-weight: 600; text-align: left; padding: 0; height: auto; }
  .tvname { font-weight: 700; font-size: 15px; }
  .manual { display: flex; gap: 10px; align-items: center; border-top: 1px solid var(--border); padding-top: 14px; margin-top: 8px; }
  .manual input { flex: 1; }
  .okc { color: var(--ok); font-weight: 600; }
  .warnc, .warn { color: var(--warn); }
  .danger { color: var(--danger); }
  .mint { color: var(--mint); }
  .warnbar { background: var(--warnbg); color: var(--warn); border-radius: 8px; padding: 10px 14px; }
  .warnbar.big { display: flex; justify-content: space-between; align-items: center; font-size: 15px; }
  .infobar { background: var(--mintbg); border-radius: 10px; padding: 12px 16px; }
  .seg { display: flex; border: 1px solid var(--border); border-radius: 10px; padding: 4px; background: var(--panel); }
  .seg button { border: none; height: 32px; color: var(--sub); }
  .seg button.on { background: var(--mintbg); color: var(--mint); outline: 1px solid var(--mint); }
  .stats { display: grid; grid-template-columns: repeat(4, 1fr); gap: 12px; }
  .stats.three-s { grid-template-columns: repeat(3, 1fr); margin-top: 12px; }
  .stat { background: var(--panel); border: 1px solid var(--border); border-radius: 12px; padding: 14px 20px; display: flex; flex-direction: column; }
  .stat b { font-family: var(--mono); font-size: 28px; }
  .stat span { color: var(--sub); font-size: 12px; }
  .tabs .tab { height: 34px; border-color: var(--border); margin-right: 8px; color: var(--sub); }
  .tabs .tab.on { background: var(--text); color: var(--bg); border-color: var(--text); }
  .table { border: 1px solid var(--border); border-radius: 12px; background: var(--panel); }
  .thead, .tr { display: grid; grid-template-columns: 36px 1.4fr 130px 110px 1.4fr; gap: 10px; align-items: center; padding: 10px 16px; }
  .thead { color: var(--dim); font-size: 11px; font-weight: 600; border-bottom: 1px solid var(--border); }
  .tr { border-bottom: 1px solid color-mix(in srgb, var(--border) 60%, transparent); cursor: pointer; }
  .tr.prot { background: color-mix(in srgb, var(--warnbg) 35%, transparent); cursor: default; }
  .pkg { overflow-wrap: anywhere; }
  .tr input { width: 18px; height: 18px; accent-color: var(--mint); }
  .item { background: var(--panel2); border-radius: 10px; padding: 10px 12px; margin-bottom: 8px; display: block; }
  .card { width: 100%; border-radius: 12px; border: 1px solid var(--border); display: block; }
  .card.ph { aspect-ratio: 16/9; display: grid; place-items: center; color: var(--mint); font-size: 22px; font-weight: 700; background: var(--panel); }
  .center { text-align: center; }
  .btns { display: grid; grid-template-columns: repeat(3, 1fr); gap: 12px; }
  .wide { width: 100%; margin: 8px 0; }
  hr { border: none; border-top: 1px solid var(--border); margin: 16px 0; }
  .prob { display: flex; gap: 12px; align-items: center; background: var(--panel2); border-radius: 10px; padding: 12px; margin-bottom: 8px; font-weight: 600; }
  .badge { width: 30px; height: 30px; border-radius: 50%; display: grid; place-items: center; flex: none; font-weight: 700; }
  .badge.shi { background: var(--dangerbg); color: var(--danger); }
  .badge.smid { background: var(--warnbg); color: var(--warn); }
  .badge.slo { background: var(--panel); color: var(--sub); }
  .opt { display: grid; grid-template-columns: auto 1fr; gap: 4px 10px; border: 1px solid var(--border); border-radius: 10px; padding: 12px; margin-bottom: 10px; cursor: pointer; }
  .opt.on { border-color: var(--mint); background: var(--panel2); }
  .opt .sub { grid-column: 2; font-size: 12px; }
  footer { position: sticky; bottom: 0; margin-top: auto; background: var(--bg); border-top: 1px solid var(--border); display: flex; gap: 18px; align-items: center; padding: 14px 0; }
  footer > .link:last-of-type { margin-left: auto; }
  .toggle { display: flex; gap: 8px; align-items: center; }
  .toggle input { width: 18px; height: 18px; accent-color: var(--mint); }
  .busy { color: var(--mint); margin-left: auto; }
  .log { background: var(--panel); border: 1px solid var(--border); border-radius: 10px; padding: 12px; max-height: 200px; overflow: auto; font-family: var(--mono); font-size: 11px; color: var(--sub); }
</style>
