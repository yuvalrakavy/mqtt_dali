import assert from 'node:assert/strict';
import {join} from 'node:path';
import {waitForAdmittedJob} from './jobs.mjs';

export async function emergencyFlow({page,origin,artifacts,inventory,control,until,commands,noOverflow,dialogVisible,emulatorPage,emulatorCard}){
  await page.goto(`${origin}/buses/2`);await page.getByRole('button',{name:'Set up bus',exact:true}).click();await page.getByRole('button',{name:'Save bus',exact:true}).click();
  // Bus actions always opens its menu, even during work. Discovery's button
  // reflects the actual controller busy flag.
  const idle=()=>until(async()=>await page.getByRole('button',{name:'Refresh addressed',exact:true}).isEnabled(),'emergency operation did not settle');
  await idle();await page.getByRole('button',{name:'Refresh addressed',exact:true}).click();await until(async()=>await page.getByRole('button',{name:'Refresh addressed',exact:true}).isEnabled(),'emergency discovery did not settle',90000);
  const saved=await inventory(),item=saved.buses.find(b=>b.bus===2).outputs[0];assert.ok(item);
  const native=async()=>(await control({op:'snapshot'})).buses.find(b=>b.number===2).outputs;
  const selected=async()=>(await native()).find(o=>o.id==='emergency-output'),before=await selected();
  const stimulusCheckbox=async(label,checked,predicate)=>{const checkbox=emulatorCard('emergency-output').getByLabel(label,{exact:true});if(await checkbox.isChecked()!==checked)await checkbox.click();await until(async()=>predicate((await selected()).emergency_state),'emergency stimulus did not reach the native model');await until(async()=>await checkbox.isChecked()===checked,'emulator checkbox did not reflect native state');};
  const row=()=>page.locator('.device-row').filter({has:page.getByRole('textbox',{name:`Name ${item.name}`,exact:true})});
  const menu=async()=>row().getByRole('button',{name:`Actions ${item.name}`,exact:true}).click();
  await menu();await page.getByRole('button',{name:'Read current values',exact:true}).click();await idle();await until(()=>row().locator('.device-type').textContent().then(text=>text.includes('DT1 emergency')),'normal Read did not populate retained DT1 support');
  const status=()=>page.getByRole('region',{name:'Emergency status',exact:true});
  const open=async()=>{await menu();await page.getByRole('button',{name:'Emergency lighting',exact:true}).click();await status().waitFor();await idle();await dialogVisible();await noOverflow();};
  const refresh=async()=>{await page.getByRole('button',{name:'Refresh status',exact:true}).click();await status().waitFor();await idle();};
  const settings=async()=>{const summary=page.getByText('Emergency settings',{exact:true});if(!await summary.evaluate(element=>element.parentElement.open))await summary.click();await page.getByLabel('Emergency level (native 1–254)',{exact:true}).waitFor();};
  await page.setViewportSize({width:390,height:844});await open();assert.ok((await status().innerText()).includes('100.0%'));
  await settings();
  for(const[label,value]of [['Emergency level (native 1–254)',123],['Prolong (seconds)',90],['Pending timeout (minutes)',15],['Function interval (days)',7],['First function test (minutes)',480],['Duration interval (weeks)',4],['First duration test (minutes)',960]])await page.getByLabel(label,{exact:true}).fill(String(value));
  await page.getByRole('button',{name:'Save emergency settings',exact:true}).click();await until(async()=>(await selected()).emergency_state.emergency_level===123,'emergency settings did not reach native gear');await idle();
  assert.equal(await page.getByLabel('Prolong (seconds)',{exact:true}).inputValue(),'90');assert.equal((await selected()).emergency_state.function_interval_days,7);assert.equal((await selected()).emergency_state.duration_interval_weeks,4);
  await page.screenshot({path:join(artifacts,'phone-emergency-settings.png'),fullPage:true});assert.ok(await page.locator('.dialog-body').evaluate(element=>element.scrollWidth<=element.clientWidth+1));
  await page.getByRole('button',{name:'Function test',exact:true}).click();await until(async()=>(await selected()).emergency_state.active_test==='function','function test request did not start');await idle();assert.ok((await status().innerText()).includes('function test'));
  if(emulatorPage){await emulatorCard('emergency-output').getByText('Emergency simulation',{exact:true}).click();await emulatorCard('emergency-output').getByRole('button',{name:'Advance 10 seconds',exact:true}).click();}else await control({op:'advance_time',millis:10000});
  await until(async()=>((await selected()).emergency_state.status_raw&2)!==0,'function test did not complete on the model clock');
  await refresh();assert.ok((await status().innerText()).includes('function done valid'));
  await page.getByRole('button',{name:'Duration test',exact:true}).click();await until(async()=>(await selected()).emergency_state.active_test==='duration','duration request held the job or did not start');await idle();await page.getByRole('button',{name:'Stop test',exact:true}).click();await until(async()=>(await selected()).emergency_state.active_test===null,'Stop did not cancel the native duration test');await status().waitFor();await idle();
  if(emulatorPage)await stimulusCheckbox('Test ready emergency-output',false,state=>state.test_ready===false);else await control({op:'set_emergency',id:'emergency-output',settings:{test_ready:false}});
  await page.getByRole('button',{name:'Function test',exact:true}).click();await idle();assert.ok((await status().innerText()).includes('function pending'));
  if(emulatorPage)await emulatorCard('emergency-output').getByRole('button',{name:'Advance 15 minutes',exact:true}).click();else await control({op:'advance_time',millis:900000});
  await until(async()=>((await selected()).emergency_state.failures&16)!==0,'pending timeout did not reach the native model');
  await refresh();const timed=await status().innerText();assert.ok(timed.includes('function pending')&&timed.includes('function max delay exceeded'),'pending timeout terminated the test or hid its fault');
  if(emulatorPage){await stimulusCheckbox('Test ready emergency-output',true,state=>state.test_ready===true);await stimulusCheckbox('Battery failure emergency-output',true,state=>(state.failures&4)!==0);await emulatorCard('emergency-output').getByLabel('Battery charge emergency-output',{exact:true}).fill('64');await emulatorCard('emergency-output').getByRole('button',{name:'Set battery charge',exact:true}).click();await until(async()=>(await selected()).emergency_state.battery_charge===64,'battery stimulus did not reach the native model');}
  else await control({op:'set_emergency',id:'emergency-output',settings:{test_ready:true,failures:20,battery_charge:64}});
  await refresh();assert.ok((await status().innerText()).includes('25.2%'));assert.ok((await status().innerText()).includes('battery failure'));await page.getByRole('button',{name:'Stop test',exact:true}).click();await until(async()=>(await selected()).emergency_state.active_test===null,'Stop did not cancel the resumed function test');await status().waitFor();await idle();
  // Lose an admitted mutation's reply after the actual gear changed. Retain
  // the draft, clear actionable evidence, and require an explicit refresh.
  await page.getByLabel('Prolong (seconds)',{exact:true}).fill('120');let lose=true;
  const intercept=async route=>{if(lose&&route.request().postDataJSON().command==='configure_emergency'){lose=false;await waitForAdmittedJob(route,page,origin,until);assert.equal((await selected()).emergency_state.prolong,4,'lost emergency setting did not reach hardware');await route.fulfill({status:200,contentType:'application/json',json:{error:'Emergency result unavailable'}});}else await route.continue();};
  await page.route(`${origin}/api/jobs`,intercept);const count=commands.length;
  try{await page.getByRole('button',{name:'Save emergency settings',exact:true}).click();await idle();assert.equal(await page.getByRole('region',{name:'Emergency status',exact:true}).count(),0);assert.ok((await page.locator('dialog').innerText()).includes('unsaved settings are retained'));assert.deepEqual(commands.slice(count).map(call=>call.command),['configure_emergency'],'lost emergency reply caused automatic replay');}
  finally{await page.unroute(`${origin}/api/jobs`,intercept);}
  await refresh();await settings();assert.equal(await page.getByLabel('Prolong (seconds)',{exact:true}).inputValue(),'120','lost reply erased the unsaved draft');
  await page.getByRole('button',{name:'Close dialog',exact:true}).click();
  await page.setViewportSize({width:1440,height:1024});await open();await settings();assert.equal(await page.getByLabel('Emergency level (native 1–254)',{exact:true}).inputValue(),'123');await page.screenshot({path:join(artifacts,'desktop-emergency-settings.png'),fullPage:true});await noOverflow();await page.getByRole('button',{name:'Close dialog',exact:true}).click();
  let entered,release;const admitted=new Promise(resolve=>entered=resolve),held=new Promise(resolve=>release=resolve);let hold=true;
  const delayed=async route=>{if(hold&&route.request().postDataJSON().command==='read_emergency'){hold=false;const response=await route.fetch();entered();await held;await route.fulfill({response}).catch(()=>{});}else await route.continue();};
  await page.route(`${origin}/api/jobs`,delayed);
  try{await menu();await page.getByRole('button',{name:'Emergency lighting',exact:true}).click();await admitted;await page.getByRole('button',{name:'Close dialog',exact:true}).click();const count=commands.length;release();await idle();assert.equal(await page.locator('dialog').count(),0);assert.equal(commands.length,count,'retired emergency read authorized more commands');}
  finally{release?.();await page.unroute(`${origin}/api/jobs`,delayed);}
  if(emulatorPage){
    await emulatorPage.setViewportSize({width:390,height:844});const bus=emulatorPage.locator('section.bus').filter({has:emulatorPage.getByRole('heading',{name:/^Bus 3 /})});
    await bus.getByLabel('Add fixture type',{exact:true}).selectOption('emergency');await emulatorPage.getByLabel('Stable emulator ID',{exact:true}).fill('browser-extra-emergency');
    await emulatorPage.getByLabel('Emergency profile JSON',{exact:true}).fill('{"function_test_seconds":2,"duration_test_seconds":120,"test_ready":false}');await emulatorPage.getByRole('button',{name:'Use JSON',exact:true}).click();await emulatorPage.getByRole('button',{name:'Add live fixture',exact:true}).click();
    await until(async()=>(await native()).some(output=>output.id==='browser-extra-emergency'),'emulator emergency profile editor did not add a configured fixture');assert.equal((await native()).find(output=>output.id==='browser-extra-emergency').emergency_state.function_test_seconds,2);
    await emulatorPage.screenshot({path:join(artifacts,'phone-emulator-emergency.png'),fullPage:true});await emulatorCard('browser-extra-emergency').getByRole('button',{name:'Remove',exact:true}).click();await emulatorPage.getByRole('button',{name:'Confirm',exact:true}).click();await until(async()=>(await native()).length===1,'emulator temporary emergency fixture did not remove');
  }
  const after=await selected();for(const field of ['short_address','level','groups','scenes','min_level','max_level','power_on_level','failure_level','fade','lamp_failure'])assert.deepEqual(after[field],before[field],`emergency operations changed unrelated native ${field}`);
  assert.deepEqual(await inventory(),saved,'emergency work changed names/addresses/bindings');await page.goto(`${origin}/buses/${saved.buses.some(bus=>bus.bus===1)?1:2}`);await idle();
  console.log('PASS emergency: phone/desktop status/settings, both test requests/Stop, pending timeout retained, emulator readiness/battery/fault controls and profile creation/removal, lost/retired replies, exact saved inventory and unrelated native data preserved');
}
