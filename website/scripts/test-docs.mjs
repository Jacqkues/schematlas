import assert from 'node:assert/strict';
import { chromium } from 'playwright-core';
const base = process.env.DEMO_TEST_URL ?? 'http://localhost:5177';
const browser = await chromium.launch({channel:'chrome',headless:true});
try {
  for(const width of [1440,390]){
    const page=await browser.newPage({viewport:{width,height:900},reducedMotion:'reduce'});
    const errors=[];page.on('pageerror',e=>errors.push(e.message));
    await page.addInitScript(()=>localStorage.setItem('schematlas-docs-test','1'));
    await page.goto(new URL('/docs',base).href);
    const reject=page.getByRole('button',{name:'Reject',exact:true});if(await reject.count())await reject.click();
    const nav=page.getByRole('navigation',{name:'Documentation',exact:true});
    const pages=await nav.locator('a').evaluateAll(links=>links.map(a=>({href:a.getAttribute('href'),title:a.textContent})));
    for(const item of pages){
      const response=await page.request.get(new URL(item.href,base).href);
      assert.equal(response.status(),200,item.href);
      assert.ok((await response.text()).includes('id="doc"'),`${item.href} has article content`);
    }
    // Wait for hydration: the production Link crash happened only after its click handler attached.
    await page.waitForTimeout(1200);
    await Promise.all([page.waitForURL('**/docs/agents'),nav.getByRole('link',{name:'Agents',exact:true}).click()]);
    await page.locator('#doc h1').filter({hasText:'Agents'}).waitFor();
    assert.equal(await page.locator('.d-nav a[aria-current="page"]').getAttribute('href'),'/docs/agents');
    await page.waitForTimeout(500);
    const next=page.locator('.d-pager .d-next');
    const nextHref=await next.getAttribute('href');
    await Promise.all([page.waitForURL(new URL(nextHref,base).href),next.click()]);
    await page.goBack();await page.locator('#doc h1').filter({hasText:'Agents'}).waitFor();
    assert.ok(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth+1),'No horizontal page overflow');
    await page.screenshot({path:`/private/tmp/schematlas-docs-fixed-${width}.png`});
    await Promise.all([page.waitForURL(new URL('/',base).href),page.getByRole('link',{name:'Schematlas home',exact:true}).click()]);
    await Promise.all([page.waitForURL(new URL('/docs',base).href),page.getByRole('link',{name:'Docs',exact:true}).last().click()]);
    await page.locator('#doc h1').waitFor();
    assert.deepEqual(errors,[]);
    console.log(`PASS ${width}px: all docs routes, hydrated navigation, next page, back, home, and landing-to-docs link`);
    await page.close();
  }
  const noJS=await browser.newPage({javaScriptEnabled:false});
  await noJS.goto(new URL('/docs',base).href);
  await Promise.all([noJS.waitForURL('**/docs/agents'),noJS.locator('.d-nav').getByRole('link',{name:'Agents',exact:true}).click()]);
  assert.match(await noJS.locator('#doc h1').textContent(),/Agents/);
  console.log('PASS: docs navigation without JavaScript');
} finally {await browser.close();}
