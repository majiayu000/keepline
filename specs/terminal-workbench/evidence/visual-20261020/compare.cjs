// Compose unaltered reference and native captures at the same logical viewport size.
const {chromium} = require(process.env.PLAYWRIGHT_MODULE || 'playwright');
const fs = require('node:fs'); const path = require('node:path');
(async()=>{
  const prefix='<!doctype html><html lang="zh-CN"><meta charset="utf-8"><title>Keepline 原稿与原生界面对照</title><style>body{margin:0;background:#0b0c10;color:#e9ebef;font:14px system-ui}h1{font-size:18px;margin:16px}p{margin:12px 16px;color:#8b919e}.pair{display:grid;grid-template-columns:repeat(2,minmax(0,1fr))}.caption{padding:12px 16px;background:#1b1e26}img{display:block;width:100%;height:auto}section{margin-bottom:24px}a{color:#c3c6ff}</style>';
  let html=prefix+'<h1>1440×900 · 设计稿 / GPUI 原生渲染</h1><p>原生截图使用隔离模拟会话；不是用户真实数据。Retina 原图按逻辑尺寸显示。<a href="README.md">证据与未对齐项</a></p>';
  const browser=await chromium.launch({headless:true});
  const page=await browser.newPage({viewport:{width:2880,height:944}, deviceScaleFactor:1});
  for(const name of ['main','overview']) {
    const pair=`<div class="pair"><div><div class="caption">${name} · 设计稿 · 1440×900</div><img src="${name}-design.png"></div><div><div class="caption">${name} · GPUI 原生 · 1440×900（2× 导出）</div><img src="${name}-native.png"></div></div>`;
    const file=path.join(__dirname,'.comparison.html');fs.writeFileSync(file,prefix+pair);
    await page.goto('file://'+file); await page.locator('img').last().waitFor();
    await page.screenshot({path:path.join(__dirname,name+'-side-by-side.png'),fullPage:true});
    fs.unlinkSync(file); html+='<section>'+pair+'</section>';
  }
  fs.writeFileSync(path.join(__dirname,'index.html'),html+'</html>');await browser.close();
})();
