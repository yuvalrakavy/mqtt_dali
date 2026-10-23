import {chromium} from 'playwright';
import {spawn} from 'node:child_process';
import {connect} from 'node:net';
import {mkdtemp,writeFile,readFile,mkdir} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join,resolve} from 'node:path';
import {fileURLToPath} from 'node:url';
import {createInterface} from 'node:readline';
import assert from 'node:assert/strict';
const root=resolve(fileURLToPath(new URL('../..',import.meta.url)));
const binary=process.env.DALI_EMULATOR_BINARY;
if(!binary)throw new Error('Set DALI_EMULATOR_BINARY to a host build of dali_hat_emulator');
const artifacts=await mkdtemp(join(tmpdir(),'dali-emulator-browser-'));await mkdir(join(artifacts,'controls'),{mode:0o700});
let child=spawn(binary,['--scenario',join(root,'examples/emulator/basic-i4.json'),'--hat-listen','127.0.0.1:0','--web-listen','127.0.0.1:0','--control-socket',join(artifacts,'controls/control.sock')],{cwd:root,stdio:['ignore','pipe','pipe'],env:{...process.env,LOG_DESTINATION:''}});
let stderr='';child.stderr.on('data',data=>stderr+=data);
const ready=await Promise.race([new Promise((resolve,reject)=>{createInterface({input:child.stdout}).on('line',line=>{try{const value=JSON.parse(line);if(value.ready)resolve(value);}catch{}});child.once('exit',code=>reject(new Error(`emulator exited ${code}: ${stderr}`)));}),new Promise((_,reject)=>setTimeout(()=>reject(new Error('emulator startup deadline')),8000))]);
const origin=`http://${ready.web_listen}`;
const browser=await chromium.launch({channel:process.env.DALI_BROWSER_CHANNEL||'chrome',headless:true});
const context=await browser.newContext({viewport:{width:390,height:844},acceptDownloads:true});const page=await context.newPage();page.setDefaultTimeout(10000);
const errors=[];page.on('pageerror',e=>errors.push(e.message));page.on('console',m=>{if(m.type()==='error'&&!m.text().includes('422')&&!m.text().includes('ERR_CONNECTION_REFUSED')&&!m.text().includes('Failed to fetch'))errors.push(m.text());});
let peer;
async function snapshot(){return (await context.request.get(`${origin}/api/snapshot`)).json();}
async function until(probe,message){const deadline=Date.now()+10000;while(Date.now()<deadline){if(await probe())return;await new Promise(r=>setTimeout(r,30));}throw new Error(message);}
async function fit(){assert(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth+1),'horizontal overflow');}
async function dialogFit(){const box=await page.getByRole('dialog').boundingBox();const viewport=page.viewportSize();assert(box&&box.x>=0&&box.y>=0&&box.x+box.width<=viewport.width+1&&box.y+box.height<=viewport.height+1,'dialog clipped');}
async function add(type,id,address=3){await page.getByLabel('Add fixture type').first().selectOption(type);await dialogFit();await page.getByLabel('Stable emulator ID').fill(id);await page.getByLabel('Initial short address').fill(String(address));}
async function commit(){await page.getByRole('button',{name:'Save to draft',exact:true}).click();await page.getByRole('dialog').waitFor({state:'hidden'});}
async function apply(){await page.getByRole('button',{name:'Apply draft & reset',exact:true}).click();await page.getByRole('button',{name:'Confirm',exact:true}).click();await page.getByRole('status').filter({hasText:'Starting scenario applied'}).waitFor();}
const card=id=>page.locator('.card').filter({has:page.getByRole('heading',{name:id,exact:true})});
try {
 await page.goto(origin);await page.getByText('Emulator online',{exact:false}).waitFor();await fit();
 await page.getByRole('button',{name:'Starting scenario',exact:true}).click();await page.getByLabel('Start from example').selectOption('Empty installation');await page.getByLabel('Clock',{exact:true}).selectOption('frozen');
 await add('dimmer','ceiling');await page.getByLabel('Initial actual level').fill('42');await page.getByLabel('Group mask').fill('3');await commit();
 await add('relay','relay',4);await commit();await add('colour','colour',5);await page.getByText('Color capabilities & initial values',{exact:true}).waitFor();await commit();
 await add('1','panel',1);await page.getByRole('button',{name:'+ Add button instance',exact:true}).click();await page.getByLabel('Instance 1',{exact:true}).selectOption('2');await page.getByRole('button',{name:'+ Add button instance',exact:true}).click();await page.getByLabel('Instance 2',{exact:true}).selectOption('3');await page.getByRole('button',{name:'+ Add button instance',exact:true}).click();await page.getByLabel('Instance 3',{exact:true}).selectOption('4');await commit();await apply();
 assert.equal((await snapshot()).buses[0].outputs.length,3);assert.equal((await snapshot()).buses[0].inputs[0].instances.length,4);
 await page.getByRole('button',{name:'Live installation',exact:true}).click();await fit();await page.screenshot({path:join(artifacts,'phone-live.png'),fullPage:true});
 const [host,port]=ready.hat_listen.split(':');peer=connect({host,port:Number(port)});let lines='';peer.on('data',data=>lines+=data);await until(()=>lines.includes('B000004\n'),'fresh TCP Boot missing');
 peer.write('h0637\nh08fe\n');await until(async()=>(await snapshot()).buses[0].outputs[0].level===55,'TCP command did not change native brightness');await until(()=>card('ceiling').locator('.level strong').innerText().then(t=>t.includes('55')),'browser did not observe native brightness');assert.equal((await snapshot()).buses[0].outputs[1].level,254);
 await card('panel').getByRole('button',{name:'Press',exact:true}).click();await until(()=>lines.includes('L028001\n'),'semantic Press did not emit configured native source');await page.getByRole('button',{name:'Advanced',exact:true}).click();await page.getByRole('button',{name:'Advance 1 second',exact:true}).click();await until(()=>lines.includes('L028009\n'),'model time did not emit long press');await page.getByRole('button',{name:'Live installation',exact:true}).click();await card('panel').getByRole('button',{name:'Release',exact:true}).click();await until(()=>lines.includes('L02800C\n'),'Release did not stop long press');
 await card('panel').getByLabel('Native raw value (0–1023)').first().fill('513');await card('panel').getByRole('button',{name:'Set physical value',exact:true}).first().click();await until(()=>lines.includes('L028601\n'),'absolute physical change did not emit full10-bit value');
 await page.getByRole('button',{name:'Save current state as scenario',exact:true}).click();await page.getByRole('status').filter({hasText:'Current state saved'}).waitFor();
 await card('ceiling').getByRole('button',{name:'Lose address',exact:true}).click();await until(async()=>(await snapshot()).buses[0].outputs[0].short_address===null,'address loss failed');assert(await card('ceiling').isVisible());
 await page.getByLabel('Add fixture type').first().selectOption('dimmer');await page.getByLabel('Stable emulator ID').fill('live-extra');await page.getByRole('button',{name:'Add live fixture',exact:true}).click();await until(async()=>(await snapshot()).buses[0].outputs.length===4,'live add failed');assert.equal((await snapshot()).buses[0].outputs[0].level,55,'live add reset another fixture');
 await card('live-extra').getByRole('button',{name:'Remove',exact:true}).click();await page.getByRole('button',{name:'Cancel',exact:true}).click();assert.equal((await snapshot()).buses[0].outputs.length,4);await card('live-extra').getByRole('button',{name:'Remove',exact:true}).click();await page.getByRole('button',{name:'Confirm',exact:true}).click();await until(async()=>(await snapshot()).buses[0].outputs.length===3,'live remove failed');
 await page.getByRole('button',{name:'Advanced',exact:true}).click();await page.getByRole('button',{name:'Reset scenario',exact:true}).click();await page.getByRole('button',{name:'Confirm',exact:true}).click();await until(async()=>(await snapshot()).buses[0].outputs[0].short_address===3,'reset failed to restore captured start');assert.equal((await snapshot()).buses[0].outputs[0].level,55);
 await page.getByRole('button',{name:'Starting scenario',exact:true}).click();const downloaded=page.waitForEvent('download');await page.getByRole('button',{name:'Download saved start',exact:true}).click();const download=await downloaded;const exported=JSON.parse(await readFile(await download.path(),'utf8'));assert.equal(exported.buses[0].outputs[0].level,55);assert(exported.buses[0].inputs[0].initial_settings);assert(exported.buses[0].outputs[2].colour_memory);
 const file=join(artifacts,'roundtrip.json');await writeFile(file,JSON.stringify(exported));await page.locator('input[type=file]').setInputFiles(file);await page.getByRole('status').filter({hasText:'Imported into local draft'}).waitFor();await apply();
 // Strict rejection leaves the running native installation unchanged.
 exported.buses[0].outputs[0].groups=70000;await writeFile(file,JSON.stringify(exported));await page.locator('input[type=file]').setInputFiles(file);await apply().catch(()=>{});await page.getByRole('alert').waitFor();assert.equal((await snapshot()).buses[0].outputs[0].groups,3);
 await page.getByRole('button',{name:'Dismiss error',exact:true}).click();await page.getByRole('button',{name:'Reload saved start',exact:true}).click();
 // Malformed nested profiles are refused before replacing the valid draft.
 for(const field of ['values','feedback']){const malformed=structuredClone(exported);malformed.buses[0].outputs[0].groups=3;malformed.buses[0].inputs[0][field]=[null];await writeFile(file,JSON.stringify(malformed));await page.locator('input[type=file]').setInputFiles(file);await page.getByRole('alert').filter({hasText:field==='values'?'Invalid queried value profile':'Invalid feedback profile'}).waitFor();assert(await page.locator('.draft-row').filter({hasText:'panel'}).isVisible());await page.getByRole('button',{name:'Dismiss error',exact:true}).click();}
 // Captured native settings must not make subsequent instance edits invalid.
 const draftPanel=page.locator('.draft-row').filter({hasText:'panel'});
 await draftPanel.getByRole('button',{name:'Edit',exact:true}).click();await page.getByRole('button',{name:'+ Add button instance',exact:true}).click();await commit();await apply();assert.equal((await snapshot()).buses[0].inputs[0].instances.length,5);
 await draftPanel.getByRole('button',{name:'Edit',exact:true}).click();await page.getByRole('button',{name:'Remove instance',exact:true}).last().click();await commit();await apply();assert.equal((await snapshot()).buses[0].inputs[0].instances.length,4);
 // JSON authority follows form edits; a dirty editor explicitly detects conflict.
 await page.getByText('Complete scenario JSON (advanced)',{exact:true}).click();let scenarioJson=page.getByLabel('Scenario JSON',{exact:true});assert(JSON.parse(await scenarioJson.inputValue()).buses[0].inputs[0].instances.length===4);
 await scenarioJson.fill((await scenarioJson.inputValue())+' ');await page.getByRole('button',{name:'+ Add bus to draft',exact:true}).click();await page.getByRole('alert').filter({hasText:'record changed outside'}).waitFor();await page.getByRole('button',{name:'Reload current JSON',exact:true}).click();assert.equal(JSON.parse(await scenarioJson.inputValue()).buses.length,2);await page.getByRole('button',{name:'Use JSON',exact:true}).click();
 // Full-record malformed JSON gives an error and retains the valid item.
 await draftPanel.getByRole('button',{name:'Edit',exact:true}).click();await page.getByText('Full fixture record (advanced)',{exact:true}).click();await page.getByLabel('Fixture JSON',{exact:true}).fill('{}');await page.getByRole('dialog').getByRole('button',{name:'Use JSON',exact:true}).click();await page.getByRole('dialog').getByRole('alert').waitFor();await page.getByRole('button',{name:'Cancel',exact:true}).click();assert(await draftPanel.isVisible());
 // Actual process restart at the same endpoints refreshes CSRF and keeps draft.
 peer?.destroy();child.kill('SIGTERM');await new Promise(resolve=>child.once('exit',resolve));await until(()=>page.getByText('Emulator disconnected',{exact:false}).isVisible(),'offline state did not appear');
 exported.buses[0].outputs[0].groups=3;await writeFile(file,JSON.stringify(exported));
 child=spawn(binary,['--scenario',file,'--hat-listen',ready.hat_listen,'--web-listen',ready.web_listen],{cwd:root,stdio:['ignore','pipe','pipe'],env:{...process.env,LOG_DESTINATION:''}});child.stderr.on('data',data=>stderr+=data);
 await until(()=>page.getByText('Emulator online',{exact:false}).isVisible(),'restart did not reconnect');assert.equal(JSON.parse(await scenarioJson.inputValue()).buses.length,2,'restart replaced an unapplied local draft');
 await page.getByRole('button',{name:'Advanced',exact:true}).click();await page.getByRole('button',{name:'Advance 1 second',exact:true}).click();await page.getByRole('status').filter({hasText:'Model advanced one second'}).waitFor();
 await page.getByRole('button',{name:'Starting scenario',exact:true}).click();
 await page.setViewportSize({width:1440,height:1000});await page.getByRole('button',{name:'Live installation',exact:true}).click();await fit();await page.screenshot({path:join(artifacts,'desktop-live.png'),fullPage:true});
 await page.getByLabel('Add fixture type').first().selectOption('colour');await dialogFit();await page.getByRole('button',{name:'Cancel',exact:true}).click();
 await page.getByRole('button',{name:'Advanced',exact:true}).click();await page.getByText('Raw compatibility controls',{exact:true}).click();await page.getByLabel('Raw control JSON').waitFor();await fit();await page.screenshot({path:join(artifacts,'desktop-advanced.png'),fullPage:true});
 assert.deepEqual(errors,[],'browser runtime/CSP errors');console.log(`PASS emulator phone/desktop workflows; artifacts ${artifacts}`);
} catch(error) {await page.screenshot({path:join(artifacts,'failure.png'),fullPage:true}).catch(()=>{});console.error(await page.locator('body').innerText());console.error(`Artifacts: ${artifacts}`);throw error;}
finally {peer?.destroy();await context.close();await browser.close();child.kill('SIGTERM');await Promise.race([new Promise(resolve=>child.once('exit',resolve)),new Promise(resolve=>setTimeout(()=>{child.kill('SIGKILL');resolve();},3000))]);}
