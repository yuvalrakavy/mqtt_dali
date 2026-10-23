import assert from 'node:assert/strict';
import {waitForAdmittedJob} from './jobs.mjs';
import {join} from 'node:path';

export async function outputSettingsFlow({page,context,origin,artifacts,inventory,control,until,commands,noOverflow,dialogVisible}){
  const saved=await inventory(),bus=saved.buses[0],item=bus.outputs.find(o=>o.name==='KitchenSide');assert.ok(item);
  const native=async()=>(await control({op:'snapshot'})).buses[0].outputs;
  const before=await native(),original=before.find(o=>o.short_address===item.short_address);assert.ok(original);
  const selected=async()=>(await native()).find(o=>o.id===original.id);
  const row=()=>page.locator('.device-row').filter({has:page.getByRole('textbox',{name:`Name ${item.name}`,exact:true})});
  const idle=()=>until(async()=>await page.getByRole('button',{name:'Bus actions',exact:true}).isEnabled(),'fixture settings operation did not settle');
  const job=async(command,params)=>{
    const session=await(await context.request.get(`${origin}/api/session`)).json();const current=await(await context.request.get(`${origin}/api/snapshot`)).json(),inventory=current.inventory.Inventory;
    const target=inventory.buses[0].outputs.find(o=>o.short_address===item.short_address);
    const response=await context.request.post(`${origin}/api/jobs`,{headers:{Origin:origin,'X-CSRF-Token':session.csrf},data:{target:target.target,generation:`${inventory.inventory_epoch}.${target.binding_revision}`,command,params}});
    assert.equal(response.status(),202);const {id}=await response.json();let result;
    await until(async()=>{result=await(await context.request.get(`${origin}/api/jobs/${id}`)).json();return result.status!=='running';},'settings fixture setup did not settle');assert.equal(result.status,'succeeded');return result.result;
  };
  const open=async()=>{await row().getByRole('button',{name:`Actions ${item.name}`,exact:true}).click();await page.getByRole('button',{name:'Fixture settings',exact:true}).click();await until(async()=>await page.getByLabel('Minimum brightness (%)',{exact:true}).isEnabled(),'fixture settings did not load');await dialogVisible();await noOverflow();assert.ok(await page.locator('.dialog-body').evaluate(e=>e.scrollWidth<=e.clientWidth+1),'fixture settings content overflows dialog');};
  const fillLevel=async(label,raw)=>page.getByLabel(label,{exact:true}).fill((raw*100/254).toFixed(1));
  const startup=async(label,raw)=>{await page.getByLabel(`${label} mode`,{exact:true}).selectOption(raw===0?'off':raw===255?'special':'level');if(raw>0&&raw<255)await fillLevel(`${label} brightness (%)`,raw);};
  const edit=async(min,max,power,failure,time,rate)=>{
    await fillLevel('Minimum brightness (%)',min);await fillLevel('Maximum brightness (%)',max);await startup('Power on',power);await startup('Bus failure',failure);await page.getByLabel('Fade time',{exact:true}).selectOption(String(time));await page.getByLabel('Dimming rate',{exact:true}).selectOption(String(rate));
  };
  const save=async()=>{await page.getByRole('button',{name:'Save settings',exact:true}).click();await until(async()=>await page.locator('dialog').count()===0,'fixture settings save did not confirm and close');await idle();};
  for(const viewport of [{width:390,height:844},{width:1440,height:1024}]){
    await page.setViewportSize(viewport);await page.goto(`${origin}/buses/0`);await idle();
    // Exact existing scalar API supplies test setup/restoration, while all
    // settings entry/edit/readback operations below use actual DOM controls.
    await job('apply_output',{values:{PowerState:false}});assert.equal((await selected()).level,0);
    for(const [min,max,power,failure,time,rate] of [[150,200,255,0,15,1],[220,240,0,255,1,15],[5,50,127,128,0,7]]){
      const start=commands.length;await open();assert.deepEqual(commands.slice(start).map(c=>c.command),['read_output_settings'],'opening fixture settings sent a mutation');
      await edit(min,max,power,failure,time,rate);
      await save();const gear=await selected();
      assert.deepEqual([gear.min_level,gear.max_level,gear.power_on_level,gear.failure_level,gear.fade,gear.level],[min,max,power,failure,(time<<4)|rate,0],'fixture settings or crossed-limit ordering failed; Off must remain Off');
      await open();assert.equal(await page.getByLabel('Minimum brightness (%)',{exact:true}).inputValue(),(min*100/254).toFixed(1));assert.equal(await page.getByLabel('Maximum brightness (%)',{exact:true}).inputValue(),(max*100/254).toFixed(1));
      assert.equal(await page.getByLabel('Fade time',{exact:true}).inputValue(),String(time));assert.equal(await page.getByLabel('Dimming rate',{exact:true}).inputValue(),String(rate));
      if(time===0){await page.getByText('Fixture diagnostics',{exact:true}).click();await page.getByText('Actual brightness: 0.0%',{exact:true}).waitFor();await page.screenshot({path:join(artifacts,`${viewport.width===390?'phone':'desktop'}-fixture-settings.png`),fullPage:true});}
      await page.getByRole('button',{name:'Cancel',exact:true}).click();assert.deepEqual(await inventory(),saved,'fixture settings changed saved names, identities or bindings');
    }
    // Lose the response after a real admitted setting has reached hardware.
    // Stale values must disappear, and no second mutation may be auto-submitted.
    await open();await page.getByLabel('Fade time',{exact:true}).selectOption('2');
    let lose=true;const intercept=async route=>{const call=route.request().postDataJSON();if(lose&&call.command==='configure_output'){lose=false;await waitForAdmittedJob(route,page,origin,until);assert.equal((await selected()).fade,0x27,'admitted fixture setting never reached hardware');await route.fulfill({status:200,contentType:'application/json',body:JSON.stringify({error:'Fixture settings result unavailable. Refresh to inspect again.'})});}else await route.continue();};
    await page.route(`${origin}/api/jobs`,intercept);const count=commands.length;
    try{await page.getByRole('button',{name:'Save settings',exact:true}).click();await until(async()=>await page.getByRole('button',{name:'Refresh settings',exact:true}).isEnabled(),'lost settings reply did not settle');assert.equal(await page.getByLabel('Minimum brightness (%)',{exact:true}).count(),0,'lost settings reply retained actionable fields');assert.equal(await page.getByRole('button',{name:'Save settings',exact:true}).isDisabled(),true);assert.deepEqual(commands.slice(count).map(c=>c.command),['configure_output'],'failed settings mutation retried automatically');}
    finally{await page.unroute(`${origin}/api/jobs`,intercept);}
    await page.getByRole('button',{name:'Refresh settings',exact:true}).click();await until(async()=>await page.getByLabel('Fade time',{exact:true}).isEnabled(),'explicit refresh did not restore settings');assert.equal(await page.getByLabel('Fade time',{exact:true}).inputValue(),'2');
    await edit(original.min_level,original.max_level,original.power_on_level,original.failure_level,original.fade>>4,original.fade&15);await save();
    await job('apply_output',{values:{Brightness:original.level/254}});
    const restored=await selected();for(const field of ['min_level','max_level','power_on_level','failure_level','fade','level'])assert.equal(restored[field],original[field],`settings workflow failed to restore ${field}`);
  }
  for(const dismissal of ['cancel','close','navigate']){
    await page.goto(`${origin}/buses/0`);await idle();let entered,release;const admitted=new Promise(r=>entered=r),held=new Promise(r=>release=r);let hold=true;
    const intercept=async route=>{if(hold&&route.request().postDataJSON().command==='read_output_settings'){hold=false;const response=await route.fetch();entered();await held;await route.fulfill({response}).catch(()=>{});}else await route.continue();};
    await page.route(`${origin}/api/jobs`,intercept);
    try{
      await row().getByRole('button',{name:`Actions ${item.name}`,exact:true}).click();await page.getByRole('button',{name:'Fixture settings',exact:true}).click();await admitted;
      if(dismissal==='cancel')await page.getByRole('button',{name:'Cancel',exact:true}).click();else if(dismissal==='close')await page.getByRole('button',{name:'Close dialog',exact:true}).click();else await page.evaluate(()=>{history.pushState(null,'','/buses/0/groups');dispatchEvent(new PopStateEvent('popstate'));});
      const count=commands.length;release();await idle();assert.equal(await page.locator('dialog').count(),0,'retired fixture read revived its dialog');assert.equal(commands.slice(count).filter(c=>c.command==='configure_output').length,0,'retired fixture read submitted a write');
    }finally{release?.();await page.unroute(`${origin}/api/jobs`,intercept);}
  }
  const after=await native();for(const gear of before){const current=after.find(o=>o.id===gear.id);for(const field of ['short_address','groups','scenes','min_level','max_level','power_on_level','failure_level','fade','level'])assert.deepEqual(current[field],gear[field],`settings changed unrelated physical field ${gear.id}/${field}`);}
  assert.deepEqual(await inventory(),saved);await page.goto(`${origin}/buses/0`);await idle();await noOverflow();
  console.log('PASS fixture settings: phone/desktop all fields, actual reopening, both crossed-limit orders, Off retained, special255 and basic fade0, exact native levels, lost reply/explicit refresh, canceled/closed/navigation-held reads and exact restoration without saved inventory changes');
}
