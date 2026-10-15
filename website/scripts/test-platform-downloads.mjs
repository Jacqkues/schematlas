import assert from 'node:assert/strict';
import { chromium } from 'playwright-core';
const base = process.env.DEMO_TEST_URL ?? 'http://localhost:5173';
const root = 'https://github.com/Jacqkues/schematlas';
const kinds = ['macos-arm64.dmg', 'macos-x64.dmg', 'windows-x64.exe', 'windows-x64.msi', 'linux-x64.AppImage', 'linux-x64.deb'];
const assets = Object.fromEntries(kinds.map(kind => [kind, `${root}/releases/download/v9.8.7/Schematlas-${kind}`]));
const browser = await chromium.launch({channel:'chrome', headless:true});
try {
  for (const [agent, platform, touch, expected, label, kind] of [
    ['Mozilla/5.0 (Windows NT 10.0; Win64; x64)', 'Win32', 0, 'windows', 'Windows', 'windows-x64.exe'],
    ['Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7)', 'MacIntel', 0, 'macos', 'macOS', null],
    ['Mozilla/5.0 (X11; Linux x86_64)', 'Linux x86_64', 0, 'linux', 'Linux', 'linux-x64.AppImage'],
    ['Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7)', 'MacIntel', 5, 'all', null, null],
    ['Mozilla/5.0 (Linux; Android 15)', 'Linux armv8l', 5, 'all', null, null],
    ['Unknown browser', 'Unknown', 0, 'all', null, null],
  ]) {
    const page = await browser.newPage({userAgent:agent, reducedMotion:'reduce', viewport:{width:390,height:844}});
    const errors=[];page.on('pageerror',e=>errors.push(e.message));
    await page.addInitScript(({platform,touch})=>{
      Object.defineProperty(navigator,'platform',{get:()=>platform});
      Object.defineProperty(navigator,'maxTouchPoints',{get:()=>touch});
    },{platform,touch});
    await page.route('**/api/releases/latest',route=>route.fulfill({json:{tag:'v9.8.7',url:`${root}/releases/tag/v9.8.7`,assets}}));
    await page.goto(base);
    await page.getByRole('link',{name:'Release notes · v9.8.7'}).waitFor();
    await page.locator(`.m-platforms[data-platform="${expected}"]`).waitFor();
    const hero=page.locator('.m-hero .m-button');
    assert.equal(await hero.textContent(),label ? `Download for ${label}` : 'Download Schematlas');
    assert.equal(await hero.getAttribute('href'),kind ? assets[kind] : '#download');
    assert.equal(await page.locator('.m-platforms article').count(),expected==='all'?3:1);
    if(expected!=='all'){
      await page.getByRole('button',{name:'Other platforms',exact:true}).click();
      assert.equal(await page.locator('.m-platforms article').count(),3);
      assert.equal(await page.locator('.m-platforms .download-link').count(),6);
      await page.getByRole('button',{name:`Show ${label} only`,exact:true}).click();
      assert.equal(await page.locator('.m-platforms article').count(),1);
    }
    assert.ok(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth));
    assert.deepEqual(errors,[]);
    await page.close();console.log(`PASS ${expected}: ${agent}`);
  }
} finally {await browser.close();}
