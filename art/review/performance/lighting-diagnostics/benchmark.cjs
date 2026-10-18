const { chromium } = require('playwright');
const { mkdirSync, writeFileSync } = require('node:fs');
const assert = require('node:assert/strict');
const output = process.argv[2] || '/tmp/casino-lighting-results';
const modes = ['baseline', 'no-fog', 'no-spot-shadows', 'no-sun-shadows', 'no-shadows', 'no-spotlights', 'no-sunlight', 'no-direct-lights', 'hardware-shadow-filter', '1024-shadow-maps', 'no-shadow-receiving', 'no-shadow-casters'];
function summarize(sample) {
  const {times,start,end}=sample;
  assert(times.length > 2 && end > start && times[0] >= start && times.at(-1) <= end);
  const intervals=times.slice(1).map((t,i)=>t-times[i]);
  assert(intervals.every(t=>t>0));intervals.sort((a,b)=>a-b);
  const seconds=(end-start)/1000;
  return {frames:times.length,seconds,fps:times.length/seconds,p95_ms:intervals[Math.floor(intervals.length*.95)]};
}
assert.equal(summarize({times:[10,20,30],start:0,end:100}).fps,30);
(async()=>{
  mkdirSync(output,{recursive:true});
  const browser = await chromium.launch({executablePath:'/usr/bin/chromium',headless:false,args:['--no-sandbox']});
  try {
    const session=await browser.newBrowserCDPSession();
    const {gpu}=await session.send('SystemInfo.getInfo');
    const page=await browser.newPage({viewport:{width:2458,height:1536},deviceScaleFactor:1});
    const errors=[],log=[],results=[];
    page.on('pageerror',e=>errors.push(String(e)));
    page.on('response',r=>{if(r.status()>=400)errors.push(`${r.status()} ${r.url()}`)});
    page.on('console',m=>{log.push(m.text());if(m.type()==='error')errors.push(m.text());if(/ready|LIGHT_ABLATION/.test(m.text()))console.log(m.text())});
    await page.addInitScript(()=>{
      const requestAdapter=GPU.prototype.requestAdapter;
      GPU.prototype.requestAdapter=async function(...args){
        const adapter=await requestAdapter.apply(this,args);
        if(adapter)window.gameAdapter={options:args,info:{vendor:adapter.info.vendor,architecture:adapter.info.architecture,description:adapter.info.description,device:adapter.info.device,isFallbackAdapter:adapter.info.isFallbackAdapter},features:[...adapter.features]};
        return adapter;
      };
      window.addEventListener('keydown',e=>{if(/^F\d+$/.test(e.key))e.preventDefault()},true);
      window.gameFrames=null;
      const acquire=GPUCanvasContext.prototype.getCurrentTexture;
      GPUCanvasContext.prototype.getCurrentTexture=function(...args){const t=acquire.apply(this,args);if(window.gameFrames)window.gameFrames.push(performance.now());return t};
    });
    const ready=page.waitForEvent('console',{predicate:m=>m.text().includes('First-person player ready'),timeout:180000});
    await page.goto('http://127.0.0.1:8080/light-ablation/');await ready;await page.bringToFront();await page.waitForTimeout(10000);
    const canvas=await page.locator('canvas').evaluate(c=>({width:c.width,height:c.height}));
    const adapter=await page.evaluate(()=>window.gameAdapter);console.log("ACTUAL_WEBGPU_ADAPTER",JSON.stringify(adapter));
    const save=()=>writeFileSync(`${output}/results.json`,JSON.stringify({browser:browser.version(),gpu,adapter,canvas,sampleMs:15000,modes,results,errors,log},null,2)+'\n');
    for(const [key,view] of [['F9','apartment'],['F7','court-from-window']]) {
      const changed=page.waitForEvent('console',{predicate:m=>m.text().includes('City review camera'),timeout:10000});await page.keyboard.press(key);await changed;
      for(const [round,order] of [[1,[0,1,2,10,11,5,0]]]) {
        for(const mode of order) {
          const changed=page.waitForEvent('console',{predicate:m=>m.text().includes(`LIGHT_ABLATION mode=${mode}`),timeout:10000});
          await page.keyboard.press(mode===10?'-':mode===11?'=':String(mode));await changed;await page.waitForTimeout(7000);
          await page.evaluate(()=>{window.gameFrames=[];window.sampleStart=performance.now()});await page.waitForTimeout(15000);
          const times=await page.evaluate(()=>{const t={times:window.gameFrames,start:window.sampleStart,end:performance.now()};window.gameFrames=null;return t});
          const result={view,round,mode:modes[mode],...summarize(times)};results.push(result);console.log(JSON.stringify(result));save();
          assert.equal(errors.length,0,errors.join('\n'));
        }
      }
    }
    assert.deepEqual(await page.locator('canvas').evaluate(c=>({width:c.width,height:c.height})),canvas);
    save();console.log('ABLATION_OK');
  }finally{await browser.close()}
})().catch(e=>{console.error(e);process.exitCode=1});
