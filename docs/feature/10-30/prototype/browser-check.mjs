import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import {spawn} from 'node:child_process';
import {fileURLToPath,pathToFileURL} from 'node:url';
const root=path.dirname(fileURLToPath(import.meta.url));
const profile=fs.mkdtempSync(path.join(os.tmpdir(),'keepline-review-check-'));
const chrome=spawn('/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',['--headless=new','--no-first-run','--no-default-browser-check','--remote-debugging-pipe','--user-data-dir='+profile,'about:blank'],{stdio:['ignore','ignore','pipe','pipe','pipe']});
let pending=new Map(),next=0,buffer=Buffer.alloc(0),errors=[];
chrome.stderr.on('data',()=>{});
chrome.stdio[4].on('data',chunk=>{buffer=Buffer.concat([buffer,chunk]);let i;while((i=buffer.indexOf(0))>=0){const raw=buffer.subarray(0,i);buffer=buffer.subarray(i+1);if(!raw.length)continue;const message=JSON.parse(raw.toString());if(message.id&&pending.has(message.id)){const p=pending.get(message.id);pending.delete(message.id);clearTimeout(p.timer);message.error?p.reject(new Error(JSON.stringify(message.error))):p.resolve(message.result)}if(message.method==='Runtime.exceptionThrown')errors.push(message.params.exceptionDetails.text)}});
function cdp(method,params={},sessionId){return new Promise((resolve,reject)=>{const id=++next;const timer=setTimeout(()=>{pending.delete(id);reject(new Error('CDP timeout '+method))},15000);pending.set(id,{resolve,reject,timer});chrome.stdio[3].write(JSON.stringify({id,method,params,...(sessionId?{sessionId}:{})})+'\0')})}
let session;
const results={date:new Date().toISOString(),kind:'隔离 Chrome 对交互原型的代理走查，非原生 App 验收',checks:[],errors};
function check(name,value){results.checks.push({name,observed:value});if(!value)throw new Error(name+' failed')}
async function evaluate(expression){const r=await cdp('Runtime.evaluate',{expression,returnByValue:true,awaitPromise:true},session);if(r.exceptionDetails)throw new Error(JSON.stringify(r.exceptionDetails));return r.result.value}
async function ready(){return evaluate(`new Promise(resolve=>{if(document.readyState==='complete')resolve(document.querySelector('#probe-summary')?.textContent||'');else addEventListener('load',()=>resolve(document.querySelector('#probe-summary')?.textContent||''),{once:true})})`)}
async function screenshot(name){const m=await cdp('Page.getLayoutMetrics',{},session);const size=m.cssContentSize;const shot=await cdp('Page.captureScreenshot',{format:'png',captureBeyondViewport:true,clip:{x:0,y:0,width:size.width,height:Math.min(size.height,5000),scale:1}},session);fs.writeFileSync(path.join(root,name+'.png'),Buffer.from(shot.data,'base64'))}
try{
 const {targetId}=await cdp('Target.createTarget',{url:'about:blank'});
 ({sessionId:session}=await cdp('Target.attachToTarget',{targetId,flatten:true}));
 await cdp('Page.enable',{},session);await cdp('Runtime.enable',{},session);
 await cdp('Emulation.setDeviceMetricsOverride',{width:1440,height:1080,deviceScaleFactor:1,mobile:false},session);
 await cdp('Page.navigate',{url:pathToFileURL(path.join(root,'index.html')).href},session);
 check('页面显示真实 probe 输出',(await ready()).includes('256 → 65,536'));
 check('未将函数通过写成整个任务验收',await evaluate(`document.body.textContent.includes('有局部证据，仍待运行验证')&&document.body.textContent.includes('尚未验证')`));
 await evaluate(`document.querySelector('[data-source=probe]').click()`);
 check('证据对话框能看见原始输出',await evaluate(`document.querySelector('dialog').open&&document.querySelector('#dialog-text').textContent.includes('second_call_soft=65536')`));
 await evaluate(`document.querySelector('#close-dialog').click();document.querySelector('#tab-source').click()`);
 check('来源页明确未关联 Agent 会话',await evaluate(`!document.querySelector('#pane-source').hidden&&document.querySelector('#pane-diff').hidden&&document.querySelector('#pane-source').textContent.includes('没有关联原 Agent 会话')`));
 await evaluate(`document.querySelector('#ask').click()`);
 check('缺口反馈生成且未发送',await evaluate(`document.querySelector('#feedback').value.includes('超过 50 个终端')&&document.body.textContent.includes('此页面不会发送指令')`));
 const draft='请补验 Finder 启动后的实际上限，保留原始输出。';
 await evaluate(`document.querySelector('#feedback').value=${JSON.stringify(draft)};document.querySelector('#feedback').dispatchEvent(new Event('input',{bubbles:true}));document.querySelector('#readmark').click();document.querySelector('#tab-diff').click()`);
 await cdp('Page.reload',{},session);await ready();
 const copyResult=await evaluate(`new Promise(resolve=>{const s=document.querySelector('#draft-status');const o=new MutationObserver(()=>{if(s.textContent.includes('已复制')||s.textContent.includes('请按')){o.disconnect();resolve(s.textContent)}});o.observe(s,{childList:true});document.querySelector('#copy').click()})`);
 check('复制反馈给出可用的复制结果或选中文字提示',copyResult.includes('已复制')||copyResult.includes('请按'));
 results.clipboard_observed=copyResult;
 check('编辑过的草稿重新打开仍完整',await evaluate(`document.querySelector('#feedback').value===${JSON.stringify(draft)}`));
 check('阅读勾选没有变成任务已验收',await evaluate(`document.querySelector('#readmark').checked&&document.querySelector('#review-status').textContent.includes('尚未验收')`));
 check('桌面无横向溢出',await evaluate(`document.documentElement.scrollWidth<=innerWidth`));
 await screenshot('desktop');
 await evaluate(`document.querySelector('#theme').click()`);check('深色模式正文和背景使用深色配色',await evaluate(`getComputedStyle(document.querySelector('#main')).backgroundColor==='rgb(18, 32, 25)'&&getComputedStyle(document.querySelector('h1')).color==='rgb(225, 235, 227)'`));await screenshot('desktop-dark');
 await evaluate(`document.querySelector('#theme').click()`);
 await cdp('Emulation.setDeviceMetricsOverride',{width:390,height:844,deviceScaleFactor:1,mobile:true},session);
 check('窄屏无横向溢出',await evaluate(`document.documentElement.scrollWidth<=innerWidth`));await screenshot('mobile');
 check('浏览器未观察到脚本异常',errors.length===0);
 results.status='observed';
}catch(error){results.status='failed';results.failure=String(error);process.exitCode=1}
finally{
 fs.writeFileSync(path.join(root,'browser-check.json'),JSON.stringify(results,null,2));
 try{await cdp('Browser.close')}catch{}
 if(chrome.exitCode===null){await new Promise(resolve=>{chrome.once('exit',resolve);setTimeout(()=>{if(chrome.exitCode===null)chrome.kill('SIGTERM');resolve()},1500)})}
 try{fs.rmSync(profile,{recursive:true,force:true})}catch{}
 console.log(JSON.stringify(results,null,2));
}
