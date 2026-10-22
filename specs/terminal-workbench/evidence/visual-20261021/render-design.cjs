// Canvas export lacks support.js. Expand its supplied data/template for a static reference.
// No source design values are edited. Run with PLAYWRIGHT_MODULE pointing to installed playwright.
const {chromium} = require(process.env.PLAYWRIGHT_MODULE || 'playwright');
const fs = require('node:fs');
const path = require('node:path');
(async () => {
  const browser = await chromium.launch({headless:true});
  const page = await browser.newPage({viewport:{width:1440,height:900}, deviceScaleFactor:1});
  await page.route(/^https?:/, route => route.abort());
  for (const name of ['Main', 'Overview', 'TodoNew', 'TodoDetail', 'TodoStart', 'Cleanup', 'Menubar']) {
    await page.goto('file://' + path.resolve(__dirname, '../../design', name+'.dc.html'));
    await page.evaluate(() => {
      const script = document.querySelector('script[data-dc-script]');
      class DCLogic { constructor(props) { this.props=props; } setState(s) {Object.assign(this.state,s);} }
      const Component = new Function('DCLogic', script.textContent+'; return Component;')(DCLogic);
      const data = new Component({}).renderVals();
      function value(expr, context) {return new Function(...Object.keys(context), 'return ('+expr+');')(...Object.values(context));}
      function substitute(text, context) {return text.replace(/\{\{(.*?)\}\}/g, (_,expr) => value(expr,context) ?? '');}
      function expand(node, context) {
        if(node.nodeType===3) {node.textContent=substitute(node.textContent,context); return;}
        if(node.nodeType!==1) return;
        if(node.tagName==='SC-FOR') {
          const list = value(node.getAttribute('list').slice(2,-2),context);
          for(const item of list) for(const child of [...node.childNodes]) {
            const clone=child.cloneNode(true); node.before(clone); expand(clone,{...context,[node.getAttribute('as')]:item});
          }
          node.remove(); return;
        }
        if(node.tagName==='SC-IF') {
          if(value(node.getAttribute('value').slice(2,-2),context)) for(const child of [...node.childNodes]) {node.before(child);expand(child,context);}
          node.remove();return;
        }
        for(const attr of [...node.attributes]) {
          if(attr.name.startsWith('on')) {node.removeAttribute(attr.name);continue;}
          node.setAttribute(attr.name,substitute(attr.value,context));
        }
        for(const child of [...node.childNodes]) expand(child,context);
      }
      expand(document.querySelector('x-dc'),data);
    });
    await page.screenshot({path:path.join(__dirname,name.toLowerCase()+'-design.png')});
  }
  await browser.close();
})();
