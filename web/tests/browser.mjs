import { scenesFlow } from './scenes.mjs';
import { chromium } from 'playwright';
import assert from 'node:assert/strict';
import { connect } from 'node:net';
import { writeFile, readFile } from 'node:fs/promises';
import { join } from 'node:path';

const [origin,socket,artifacts]=process.argv.slice(2);
const browser=await chromium.launch({channel:process.env.DALI_BROWSER_CHANNEL||'chrome',headless:true});
const context=await browser.newContext({viewport:{width:390,height:844},deviceScaleFactor:1});
const page=await context.newPage();page.setDefaultTimeout(15000);
const errors=[];page.on('pageerror',e=>errors.push(e.message));page.on('console',m=>{if(m.type()==='error'&&!m.text().includes('401 (Unauthorized)'))errors.push(m.text());});
page.on('pageerror',error=>console.log('PAGE ERROR',error.stack));
const commands=[];page.on('request',request=>{if(request.url()===`${origin}/api/jobs`&&request.method()==='POST')commands.push(request.postDataJSON());});
let releaseAdmission=()=>{};
async function until(probe,message,ms=20000){const start=Date.now();while(Date.now()-start<ms){if(await probe())return;await new Promise(r=>setTimeout(r,40));}throw new Error(message);}
async function control(action){return new Promise((resolve,reject)=>{const client=connect(socket),timer=setTimeout(()=>{client.destroy();reject(new Error('Emulator control expired'));},2000);let data='';client.on('connect',()=>client.end(`${JSON.stringify(action)}\n`));client.on('data',chunk=>data+=chunk);client.on('error',reject);client.on('end',()=>{clearTimeout(timer);try{const reply=JSON.parse(data);if(reply.error)reject(new Error(reply.error));else resolve(reply.result);}catch(e){reject(e);}});});}
const inventory=async()=>JSON.parse(await readFile(join(artifacts,'dali.v2.json'),'utf8'));
const row=name=>page.locator('.device-row').filter({has:page.getByRole('textbox',{name:`Name ${name}`,exact:true})});
const named=name=>page.getByRole('textbox',{name:`Name ${name}`,exact:true});
async function quiet(){await until(async()=>!(await page.getByRole('status').allTextContents()).some(t=>t.includes('Saving')),'name save remained pending');}
async function closeDialog(){await page.getByRole('button',{name:'Close dialog',exact:true}).click();}
async function noOverflow(){assert.ok(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth+1),'page overflows horizontally');}
async function dialogVisible(){await page.locator('dialog').waitFor({state:'visible'});const box=await page.locator('dialog').boundingBox();assert.ok(box&&box.x>=0&&box.y>=0&&box.x+box.width<=390&&box.y+box.height<=844,'phone dialog is clipped');}
try {
  await page.goto(`${origin}/buses/0`);await until(()=>Promise.resolve(page.url().endsWith('/login')),'unauthenticated page did not redirect');
  assert.equal((await context.request.get(`${origin}/assets/app.js`,{maxRedirects:0})).status(),303);
  await page.getByLabel('Password',{exact:true}).fill('incorrect');await page.getByRole('button',{name:'Sign in',exact:true}).click();await page.getByRole('alert').filter({hasText:'invalid password'}).waitFor();
  await page.getByLabel('Password',{exact:true}).fill('browser-secret');await page.getByRole('button',{name:'Sign in',exact:true}).click();await page.getByRole('heading',{name:'BrowserController',exact:true}).waitFor();
  await page.getByRole('button',{name:'Set up bus',exact:true}).first().waitFor();await page.screenshot({path:join(artifacts,'phone-overview.png'),fullPage:true});await noOverflow();
  await page.getByRole('button',{name:'Set up bus',exact:true}).first().click();await dialogVisible();await page.getByRole('button',{name:'Save bus',exact:true}).click();await page.getByRole('link',{name:'Open bus',exact:true}).first().click();
  await page.getByRole('heading',{name:'BrowserController/Bus1',exact:true}).waitFor();await page.getByRole('button',{name:'Add unaddressed',exact:true}).click();
  await until(async()=>!await page.getByRole('button',{name:'Add unaddressed',exact:true}).isDisabled(),'real discovery did not finish',90000);
  assert.equal((await inventory()).buses[0].outputs.length,3,'discovery omitted existing or unaddressed fixtures');
  await named('Light1').waitFor();await named('Light2').waitFor();await named('Light3').waitFor();
  const physical=await control({op:'snapshot'});assert.deepEqual(physical.buses[0].outputs.map(o=>o.short_address).sort(),[0,1,2],'unaddressed commissioning overwrote an existing address');
  assert.ok((await page.locator('.input-grid.device-row').filter({hasText:'Part 302'}).innerText()).includes('0x0FFF'),'queried Part 302 value is missing before any native event');
  assert.ok((await page.locator('.input-grid.device-row').filter({hasText:'Part 304'}).innerText()).includes('0xFEDCBA9876543210'),'queried Part 304 value lost exact bits before any native event');
  await page.screenshot({path:join(artifacts,'phone-devices.png'),fullPage:true});await noOverflow();
  // Checkmark, Enter and moving focus persist through the real owned job path.
  const before=commands.filter(c=>c.command==='rename').length;
  await page.getByRole('button',{name:'Blink Light1',exact:true}).click();await page.getByRole('button',{name:'Stop blink Light1',exact:true}).waitFor();
  await named('Light1').fill('KitchenMain');await page.getByRole('button',{name:'Save name Light1',exact:true}).click();await named('KitchenMain').waitFor();
  await until(async()=>(await control({op:'snapshot'})).buses[0].outputs[0].level===91,'renaming did not stop blink and restore actual level');
  assert.equal(commands.filter(c=>c.command==='rename').length,before+1,'checkmark and blur sent duplicate renames');
  await named('Light2').fill('KitchenSide');await named('Light3').click();await named('Light3').fill('Hall');await named('Light3').press('Enter');await named('KitchenSide').waitFor();await named('Hall').waitFor();await quiet();
  await until(async()=>(await inventory()).buses[0].outputs.map(o=>o.name).join(',')==='KitchenMain,KitchenSide,Hall','inline names did not persist');
  // Failed disk save keeps the draft and identification, then explicit retry saves.
  await page.getByRole('button',{name:'Blink Hall',exact:true}).click();await page.getByRole('button',{name:'Stop blink Hall',exact:true}).waitFor();
  await writeFile(join(artifacts,'fail-next-save'),'1');await named('Hall').fill('HallNew');await named('Hall').press('Enter');await row('Hall').getByRole('alert').waitFor();
  assert.equal(await named('Hall').inputValue(),'HallNew','failed save lost the draft');assert.ok(await page.getByRole('button',{name:'Stop blink Hall',exact:true}).isVisible(),'failed save stopped identification');
  await page.getByRole('button',{name:'Save name Hall',exact:true}).click();await named('HallNew').waitFor();
  await page.getByRole('button',{name:'Light*',exact:true}).click();assert.equal(await page.locator('.output-grid.device-row').count(),0,'Light* filter did not hide renamed fixtures');await page.getByRole('button',{name:'All',exact:true}).click();
  // Native input activity updates an existing row without replacing its dirty name.
  const input=page.locator('.input-grid.device-row').filter({hasText:'Part 301'}).first(),inputName=input.getByRole('textbox');await inputName.fill('DoorButton');await control({op:'input_event',bus:0,frame:0x008001});await until(async()=>await input.innerText().then(t=>t.includes('Pressed')),'native input state did not reach browser');assert.equal(await inputName.inputValue(),'DoorButton','native activity replaced the dirty draft');await inputName.press('Enter');await named('DoorButton').waitFor();
  // On/off and committed slider changes affect the independent physical model.
  await row('KitchenSide').getByRole('button',{name:'Power KitchenSide',exact:true}).click();await until(async()=>(await control({op:'snapshot'})).buses[0].outputs[1].level===0,'power toggle did not affect physical fixture');
  const range=row('KitchenSide').getByRole('slider');await until(async()=>!await range.isDisabled(),'brightness remained disabled');await range.focus();await range.press('ArrowRight');await range.press('Tab');await until(async()=>(await control({op:'snapshot'})).buses[0].outputs[1].level>0,'brightness commit did not affect fixture');
  // Phone actions and colour form must fit and persist an actual DT8 write.
  await row('KitchenMain').getByRole('button',{name:'Actions KitchenMain',exact:true}).click();await dialogVisible();await page.getByRole('button',{name:'Colour representation and values',exact:true}).click();await dialogVisible();await page.getByLabel('Colour representation',{exact:true}).selectOption('temperature');await until(async()=>await page.locator('dialog').count()===0,'representation save did not finish');
  await row('KitchenMain').getByRole('button',{name:'Actions KitchenMain',exact:true}).click();await page.getByRole('button',{name:'Colour representation and values',exact:true}).click();await page.getByLabel('Temperature (K)',{exact:true}).fill('4000');await page.getByRole('button',{name:'Apply colour',exact:true}).click();await until(async()=>await page.locator('dialog').count()===0,'colour write failed');assert.equal((await control({op:'snapshot'})).buses[0].outputs[0].colour_state.actual.temperature.value,250,'DT8 temperature did not reach independent model');
  await page.getByRole('link',{name:'Groups',exact:true}).click();await page.getByLabel('New group name',{exact:true}).fill('Kitchen');await page.getByRole('button',{name:'Create group',exact:true}).click();await page.getByLabel('Group name pattern',{exact:true}).fill('Kitchen*');await page.getByRole('button',{name:'Preview pattern',exact:true}).click();await dialogVisible();await page.getByRole('button',{name:'Apply reviewed changes',exact:true}).click();await until(async()=>(await control({op:'snapshot'})).buses[0].outputs.filter(o=>(o.groups&1)!==0).length===2,'pattern group assignment did not reach fixtures');
  await page.getByRole('button',{name:'Verify all groups',exact:true}).click();await page.getByRole('heading',{name:'Verify group assignments',exact:true}).waitFor();await dialogVisible();assert.ok((await page.locator('dialog').innerText()).includes('Assignments match'),'group verification did not read hardware');
  const review=name=>page.locator('.verify-row').filter({has:page.getByRole('heading',{name,exact:true})});
  await review('KitchenMain').getByRole('button',{name:'Adopt hardware assignments',exact:true}).click();await until(async()=>(await inventory()).buses[0].groups.some(g=>g.group_address===3&&g.members.includes(0)),'adoption did not save unnamed hardware membership');await closeDialog();
  await page.getByRole('button',{name:'Verify all groups',exact:true}).click();await page.getByRole('heading',{name:'Verify group assignments',exact:true}).waitFor();await review('HallNew').getByRole('button',{name:'Repair to saved assignments',exact:true}).click();await until(async()=>(await control({op:'snapshot'})).buses[0].outputs[2].groups===0,'repair did not remove extraneous hardware membership');await until(async()=>await review('HallNew').innerText().then(t=>t.includes('Assignments match')),'repair did not re-read complete hardware membership');await closeDialog();
  await page.screenshot({path:join(artifacts,'phone-groups.png'),fullPage:true});await noOverflow();
  await scenesFlow({page,context,origin,artifacts,inventory,control,until,commands,noOverflow,dialogVisible,closeDialog});
  await page.setViewportSize({width:1440,height:1024});await page.goto(`${origin}/buses/0`);await named('KitchenMain').waitFor();await noOverflow();await page.screenshot({path:join(artifacts,'desktop-devices.png'),fullPage:true});
  await row('KitchenSide').getByRole('button',{name:'Actions KitchenSide',exact:true}).click();await page.getByRole('button',{name:'Group membership',exact:true}).click();assert.ok(await page.getByRole('checkbox',{name:'Kitchen 0'}).isChecked());await closeDialog();
  await row('HallNew').getByRole('button',{name:'Actions HallNew',exact:true}).click();await page.getByRole('button',{name:'Remove from list',exact:true}).click();await page.getByRole('button',{name:'Remove from list',exact:true}).click();await until(async()=>(await inventory()).buses[0].outputs.length===2,'remove did not persist');assert.equal((await control({op:'snapshot'})).buses[0].outputs.length,3,'remove altered physical fixtures');
  await page.getByRole('button',{name:'Refresh addressed',exact:true}).click();await until(async()=>!await page.getByRole('button',{name:'Refresh addressed',exact:true}).isDisabled(),'rediscovery did not finish',90000);assert.deepEqual((await inventory()).buses[0].outputs.map(o=>o.name),['KitchenMain','KitchenSide','Light1'],'rediscovery failed to preserve existing names and add the removed address');
  await page.getByRole('link',{name:'Settings',exact:true}).click();const downloadPromise=page.waitForEvent('download');await page.getByRole('button',{name:'Download inventory report',exact:true}).click();const download=await downloadPromise;assert.equal(download.suggestedFilename(),'dali-inventory-report.json');await page.getByRole('link',{name:'Diagnostics',exact:true}).click();await page.getByRole('heading',{name:'Recent commands',exact:true}).waitFor();await page.screenshot({path:join(artifacts,'desktop-diagnostics.png'),fullPage:true});
  await page.goto(`${origin}/buses/0`);await named('KitchenMain').waitFor();
  let entered;const admitted=new Promise(resolve=>entered=resolve),held=new Promise(resolve=>releaseAdmission=resolve);let hold=true;
  await page.route(`${origin}/api/jobs`,async route=>{if(hold&&route.request().postDataJSON().command==='discover_address'){hold=false;const response=await route.fetch();entered();await held;await route.fulfill({response}).catch(()=>{});}else await route.continue();});
  await page.getByRole('button',{name:'Refresh addressed',exact:true}).click();await admitted;const beforeLogout=commands.length;
  const logout=page.waitForRequest(`${origin}/api/logout`,{timeout:3000});await page.getByRole('button',{name:'Sign out',exact:true}).click();
  try{await logout;}catch{throw new Error('logout was blocked by an active discovery job');}releaseAdmission();
  await until(()=>Promise.resolve(page.url().endsWith('/login')),'logout did not return to sign-in');await page.unroute(`${origin}/api/jobs`);await new Promise(r=>setTimeout(r,100));assert.equal(commands.length,beforeLogout,'batch submitted another command after logout');assert.equal((await context.request.get(`${origin}/assets/app.js`,{maxRedirects:0})).status(),303,'logout left app assets accessible');
  assert.deepEqual(errors,[],'browser reported runtime or CSP errors');console.log(`PASS phone/desktop: ${commands.length} real owned commands; screenshots ${artifacts}`);
} catch(error){await page.screenshot({path:join(artifacts,'failure.png'),fullPage:true}).catch(()=>{});console.error('LAST COMMANDS',JSON.stringify(commands.slice(-12),null,2));console.error(await page.locator('body').innerText());throw error;} finally{releaseAdmission();await context.close();await browser.close();}
