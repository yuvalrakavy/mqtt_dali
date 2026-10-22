import assert from 'node:assert/strict';
import { join } from 'node:path';

export async function restorationFlow({page,origin,artifacts,inventory,control,until,commands,noOverflow,dialogVisible,closeDialog}){
  const idle=()=>until(async()=>await page.getByRole('button',{name:'Add unaddressed',exact:true}).isEnabled(),'address recovery did not settle');
  const row=name=>page.locator('.device-row').filter({has:page.getByRole('textbox',{name:`Name ${name}`,exact:true})});
  const native=()=>control({op:'snapshot'});
  const move=(id,short_address)=>control({op:'set_address',id,short_address});
  async function open(name){await row(name).getByRole('button',{name:`Actions ${name}`,exact:true}).click();await page.getByRole('button',{name:'Restore lost address',exact:true}).click();await page.getByRole('region',{name:'Lost address recovery',exact:true}).waitFor();}
  async function unique(){await page.getByRole('button',{name:'Restore sole unaddressed fixture',exact:true}).click();await page.getByRole('heading',{name:'Confirm address restoration',exact:true}).waitFor();}
  await page.setViewportSize({width:390,height:844});await page.goto(`${origin}/buses/0`);await idle();
  const before=await inventory(),old=before.buses[0].outputs.find(o=>o.short_address===0),plain=before.buses[0].outputs.find(o=>o.short_address===1);
  const retained=physical=>physical.buses[0].outputs.map(o=>({id:o.id,level:o.level,groups:o.groups,scenes:o.scenes,actual:o.colour_state?.actual}));
  const settings=retained(await native());
  await move('white',null);await open(old.name);await unique();await dialogVisible();const count=commands.length;
  await page.getByRole('button',{name:'Cancel',exact:true}).click();assert.equal(commands.length,count,'cancelled restoration review submitted work');
  await unique();await page.screenshot({path:join(artifacts,'phone-address-restoration-review.png'),fullPage:true});await noOverflow();
  await page.getByRole('button',{name:'Restore saved address',exact:true}).click();await idle();assert.equal((await native()).buses[0].outputs.find(o=>o.id==='white').short_address,0);assert.deepEqual((await inventory()).buses[0].outputs.find(o=>o.short_address===0),old,'unique restoration changed saved identity');assert.deepEqual(retained(await native()),settings);
  // More than one unaddressed fixture must not be picked arbitrarily.
  await move('white',null);await move('plain',null);await open(old.name);await unique();await page.getByRole('button',{name:'Restore saved address',exact:true}).click();await idle();
  assert.ok((await page.getByRole('alert').allTextContents()).some(text=>text.includes('Multiple unaddressed')));assert.ok((await native()).buses[0].outputs.filter(o=>['white','plain'].includes(o.id)).every(o=>o.short_address===null));
  await page.getByRole('button',{name:'Close recovery',exact:true}).click();await page.getByRole('button',{name:'Add unaddressed',exact:true}).click();await idle();let saved=await inventory();
  const scanned=await native(),whiteAddress=scanned.buses[0].outputs.find(o=>o.id==='white').short_address,plainAddress=scanned.buses[0].outputs.find(o=>o.id==='plain').short_address;
  assert.ok(whiteAddress>2&&plainAddress>2&&whiteAddress!==plainAddress,'normal bus scan did not assign both lost-address fixtures');
  let candidate=saved.buses[0].outputs.find(o=>o.short_address===whiteAddress);assert.ok(candidate,'normal bus scan did not keep the newly addressed fixture');await open(old.name);
  await row(candidate.name).getByRole('button',{name:`Blink ${candidate.name}`,exact:true}).click();await idle();await row(candidate.name).getByRole('button',{name:'Use this fixture',exact:true}).click();await dialogVisible();
  assert.ok((await page.locator('dialog').innerText()).includes('becomes available again'));
  await page.getByRole('button',{name:'Stop blink and adopt address',exact:true}).click();await idle();saved=await inventory();assert.ok(!saved.buses[0].outputs.some(o=>o.name===candidate.name),'temporary candidate lifetime survived recovery');assert.ok(!saved.buses[0].outputs.some(o=>o.short_address===0),'previous output address stayed reserved');assert.deepEqual(saved.buses[0].outputs.find(o=>o.name===old.name),{...old,short_address:whiteAddress});assert.equal((await native()).buses[0].outputs.find(o=>o.id==='white').short_address,whiteAddress,'recovery unnecessarily moved the hardware address');assert.deepEqual(retained(await native()),settings,'address adoption or stop changed native settings');
  candidate=saved.buses[0].outputs.find(o=>o.short_address===plainAddress);await open(plain.name);await row(candidate.name).getByRole('button',{name:`Blink ${candidate.name}`,exact:true}).click();await idle();await row(candidate.name).getByRole('button',{name:'Use this fixture',exact:true}).click();await page.getByRole('button',{name:'Stop blink and adopt address',exact:true}).click();await idle();saved=await inventory();assert.deepEqual(saved.buses[0].outputs.find(o=>o.name===plain.name),{...plain,short_address:plainAddress});assert.ok(!saved.buses[0].outputs.some(o=>o.short_address===1),'second previous address stayed reserved');assert.deepEqual(retained(await native()),settings);
  // Inputs use their own temporary address space and display fresh native action.
  await page.setViewportSize({width:1440,height:1024});const originalInput=saved.buses[0].inputs[0];await move('buttons',null);await open(originalInput.name);const inputStart=commands.length;
  await page.getByRole('button',{name:'Find another candidate',exact:true}).click();await idle();saved=await inventory();candidate=saved.buses[0].inputs.find(i=>i.short_address!==0);assert.ok(candidate);assert.ok(commands.slice(inputStart).every(c=>c.command!=='assign_next_output'&&c.params.space!=='outputs'),'input recovery touched outputs');
  const instance=candidate.instances[0];await control({op:'input_event',bus:0,frame:(candidate.short_address<<17)|0x008001});await until(async()=>await row(instance.name).last().innerText().then(t=>t.includes('pressed')),'candidate button activity did not reach the device list');
  await row(candidate.name).getByRole('button',{name:'Use this fixture',exact:true}).click();await page.screenshot({path:join(artifacts,'desktop-input-restoration-review.png'),fullPage:false});await page.getByRole('button',{name:'Adopt identified address',exact:true}).click();await idle();saved=await inventory();assert.deepEqual(saved.buses[0].inputs,[{...originalInput,short_address:candidate.short_address}],'input recovery replaced original module or child identities');assert.equal((await native()).buses[0].inputs[0].short_address,candidate.short_address);assert.ok(!saved.buses[0].inputs.some(i=>i.short_address===0),'previous input address stayed reserved');
  // A completed command can outlive its Devices route; it must not start refresh.
  await move('white',null);await open(old.name);await unique();let entered,release;const admitted=new Promise(r=>entered=r),held=new Promise(r=>release=r);let hold=true;
  await page.route(`${origin}/api/jobs`,async route=>{if(hold&&route.request().postDataJSON().command==='restore_unaddressed_address'){hold=false;const response=await route.fetch();entered();await held;await route.fulfill({response}).catch(()=>{});}else await route.continue();});
  try{await page.getByRole('button',{name:'Restore saved address',exact:true}).click();await admitted;const stopped=commands.length;await page.getByRole('link',{name:'Groups',exact:true}).click();release();await until(async()=>await page.getByRole('button',{name:'Bus actions',exact:true}).isEnabled(),'route changed');await new Promise(r=>setTimeout(r,200));assert.equal(commands.length,stopped,'restoration refreshed after its Devices route ended');}
  finally{release?.();await page.unroute(`${origin}/api/jobs`);}
  await page.getByRole('link',{name:'Devices',exact:true}).click();await idle();assert.deepEqual(retained(await native()),settings);await noOverflow();
  console.log('PASS address restoration: unique unchanged identity, cancelled review, multiple refusal, temporary output Blink and confirmed stop, input activity and child identity, tab cancellation, phone/desktop');
}
