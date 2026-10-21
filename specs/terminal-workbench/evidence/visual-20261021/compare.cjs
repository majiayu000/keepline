// Compose original PNGs at equal logical scale. Native @2x is displayed at 1x.
const {chromium}=require(process.env.PLAYWRIGHT_MODULE||'playwright');
const fs=require('node:fs'), path=require('node:path');
const names=['main','overview','todonew','tododetail','todostart','cleanup','menubar'];
const style=`<meta charset="utf-8"><style>body{margin:0;background:#181a24;color:#e9ebef;font:14px system-ui}h1,p,nav{margin:16px}a{color:#c3c6ff;margin-right:18px}.pair{display:flex}.frame{width:1440px;flex-shrink:0}.canvas{width:1440px;height:900px;background:#fff;overflow:hidden}.caption{height:40px;padding:10px 16px;box-sizing:border-box}img{display:block;width:1440px;height:900px}.menu{width:360px;height:480px}section{overflow:auto;margin-bottom:32px}button{margin:8px;padding:8px}button.active{background:#c3c6ff}h2{margin:16px}</style>`;
(async()=>{
 const browser=await chromium.launch({headless:true}); const page=await browser.newPage({viewport:{width:2880,height:940},deviceScaleFactor:1});
 let html=`<!doctype html><html lang="zh-CN"><title>R14 七页视觉对照</title>${style}<style>.pair{display:grid;grid-template-columns:repeat(2,minmax(0,1fr))}.frame{width:100%}.canvas{width:100%;height:auto;aspect-ratio:1440/900}img{width:100%;height:auto}img.menu{width:25%;height:auto}</style><h1>R14 · 七页设计 / 原生渲染对照</h1><p>1440×900 逻辑尺寸。六页 GPUI 原生渲染，Menubar 为 AppKit 360×480 原生面板放入同尺度画布。隔离合成会话，无模型调用，不是用户数据。<a href="README.md">差异与验证记录</a></p><nav>${names.map(n=>`<a href="#${n}">${n}</a>`).join('')}</nav><p><button onclick="choose('before')">修复前</button><button onclick="choose('after')">修复后</button>两侧按相同逻辑尺度缩放；点击独立 PNG 可查看原尺寸。</p>`;
 for(const name of names){
   const pair=(phase)=>`<div class="pair"><div class="frame"><div class="caption">${name} · 原设计（未修改）</div><div class="canvas"><img src="${name}-design.png"></div></div><div class="frame"><div class="caption">${name} · ${phase==='before'?'修复前':'修复后'}原生</div><div class="canvas"><img class="${name==='menubar'?'menu':''}" src="${phase}/${name}-native.png"></div></div></div>`;
   for(const phase of ['before','after']){
     const file=path.join(__dirname,'.comparison.html');fs.writeFileSync(file,`<!doctype html><html>${style}${pair(phase)}</html>`);
     await page.goto('file://'+file);await page.evaluate(()=>Promise.all([...document.images].map(i=>i.decode())));
     await page.screenshot({path:path.join(__dirname,phase,name+'-side-by-side.png')});fs.unlinkSync(file);
   }
   html+=`<section id="${name}"><h2>${name}</h2><div data-phase="before" hidden>${pair('before')}</div><div data-phase="after">${pair('after')}</div></section>`;
 }
 html+=`<script>function choose(phase){document.querySelectorAll('[data-phase]').forEach(n=>n.hidden=n.dataset.phase!==phase)}</script></html>`;
 fs.writeFileSync(path.join(__dirname,'index.html'),html);await browser.close();
})();
