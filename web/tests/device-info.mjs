import assert from 'node:assert/strict';
import {join} from 'node:path';
export async function deviceInfoFlow({page,origin,artifacts,inventory,control,until,commands,noOverflow,dialogVisible,setup=false}){
  const idle=()=>until(async()=>{
    const refresh=page.getByRole('button',{name:'Read identity again',exact:true});
    return await refresh.count()?refresh.isEnabled():page.getByRole('button',{name:'Refresh addressed',exact:true}).isEnabled();
  },'memory read did not settle');
  await page.goto(`${origin}/buses/0`);
  if(setup){await page.getByRole('button',{name:'Set up bus',exact:true}).click();await page.getByRole('button',{name:'Save bus',exact:true}).click();await idle();await page.getByRole('button',{name:'Refresh addressed',exact:true}).click();await until(()=>page.getByRole('button',{name:'Refresh addressed',exact:true}).isEnabled(),'memory discovery did not finish',90000);}
  await idle();const saved=await inventory(),bus=saved.buses[0],white=bus.outputs.find(o=>o.short_address===0),plain=bus.outputs.find(o=>o.short_address===1),input=bus.inputs.find(o=>o.short_address===0);
  assert.ok(white&&plain&&input,'memory flow requires saved physical modules');
  const native=async()=>{const value=await control({op:'snapshot'});delete value.time_ms;for(const bus of value.buses){for(const gear of bus.outputs)delete gear.dtr;for(const input of bus.inputs)for(const key of ['dtr0','dtr1','dtr2'])delete input[key];}return value;};
  const before=await native(),row=item=>page.locator('.device-row').filter({has:page.getByRole('textbox',{name:`Name ${item.name}`,exact:true})}).filter({has:page.locator('.address').filter({hasText:new RegExp(`^${item.number===undefined?item.short_address:`${input.short_address} / ${item.number}`}$`)})});
  const result=()=>page.getByRole('region',{name:'Device information result',exact:true});
  const open=async item=>{await row(item).getByRole('button',{name:`Actions ${item.name}`,exact:true}).click();await dialogVisible();await page.getByRole('button',{name:'Device information',exact:true}).click();await result().waitFor();await idle();await dialogVisible();};
  const close=async()=>{await page.getByRole('button',{name:'Close dialog',exact:true}).click();await idle();};
  for(const viewport of [{width:390,height:844},{width:1440,height:1024}]){
    await page.setViewportSize(viewport);await page.goto(`${origin}/buses/0`);await idle();await open(white);
    await result().getByText('FEDCBA9876543210 (64-bit)',{exact:true}).waitFor();await result().getByText('modern',{exact:true}).waitFor();
    assert.equal(await result().locator('dd').nth(2).innerText(),'6.7');assert.equal(await result().locator('dd').nth(3).innerText(),'2.3');
    await noOverflow();assert.ok(await page.locator('.dialog-body').evaluate(e=>e.scrollWidth<=e.clientWidth+1),'device information overflows');
    await page.screenshot({path:join(artifacts,`${viewport.width===390?'phone':'desktop'}-device-information.png`),fullPage:true});
    const firstData=commands.length;await page.getByRole('button',{name:'Read luminaire data',exact:true}).click();
    const data=()=>page.getByRole('region',{name:'Luminaire data',exact:true}),field=key=>data().locator(`[data-luminaire-field="${key}"]`);
    await field('luminaire_id').getByText('FEDCBA9876543210',{exact:true}).waitFor();await idle();
    assert.deepEqual(commands.slice(firstData).map(c=>c.command),['read_luminaire_data']);
    assert.equal(await field('input_power_nominal').innerText(),'42 W');assert.equal(await field('manufacture_year').innerText(),'24');assert.equal(await field('cri').innerText(),'Unavailable');
    assert.equal(await field('cct').innerText(),'Variable (Part 209 indication)');assert.equal(await field('identification').innerText(),'Fixture <b>OEM</b>');assert.equal(await data().locator('b').count(),0,'OEM text was interpreted as HTML');
    await result().getByText('partial',{exact:true}).waitFor();await page.locator('.dialog-body').evaluate(e=>e.scrollTop=0);await noOverflow();await dialogVisible();
    await page.screenshot({path:join(artifacts,`${viewport.width===390?'phone':'desktop'}-luminaire-data.png`),fullPage:true});
    await page.getByText('Captured bank 1 bytes',{exact:true}).click();assert.equal(await result().locator('tbody tr').count(),120);
    await result().locator('tbody tr').last().scrollIntoViewIfNeeded();assert.equal(await result().locator('tbody tr').last().locator('td').first().innerText(),'0x77');await dialogVisible();
    assert.ok(await page.locator('.dialog-body').evaluate(e=>e.scrollWidth<=e.clientWidth+1),'luminaire data overflows horizontally');
    await page.screenshot({path:join(artifacts,`${viewport.width===390?'phone':'desktop'}-luminaire-raw.png`),fullPage:true});
    // Lose admission after the real read finishes. No previous capture or retry
    // may survive the failure; an explicit new read restores the report.
    let lostResolve;const lostReply=new Promise(r=>lostResolve=r);let lose=true;const loss=async route=>{const call=route.request().postDataJSON();if(lose&&call.command==='read_luminaire_data'){
      lose=false;const response=await route.fetch(),admission=await response.json();let job;
      await until(async()=>{job=await(await page.context().request.get(`${origin}/api/jobs/${admission.id}`)).json();return job.status!=='running';},'native luminaire read did not finish');assert.equal(job.status,'succeeded');
      await route.fulfill({status:200,contentType:'application/json',body:JSON.stringify({error:'Luminaire result unavailable.'})});
      lostResolve();
    }else await route.continue();};
    await page.route(`${origin}/api/jobs`,loss);const lostStart=commands.length;
    try{await page.getByRole('button',{name:'Read luminaire data',exact:true}).click();await lostReply;await idle();assert.equal(await data().count(),0,'lost luminaire result retained old data');assert.deepEqual(commands.slice(lostStart).map(c=>c.command),['read_luminaire_data'],'lost read retried automatically');}
    finally{await page.unroute(`${origin}/api/jobs`,loss);}
    await page.getByRole('button',{name:'Read luminaire data',exact:true}).click();await field('input_power_nominal').getByText('42 W',{exact:true}).waitFor();await idle();
    const energy=()=>page.getByRole('region',{name:'Energy data',exact:true}),counter=key=>energy().locator(`[data-energy-field="${key}"]`);
    await page.getByText('Energy and power',{exact:true}).click();
    for(const [bank,expected,power]of [['202','123.456 Wh','42.5 W'],['203','281474976710653000000 VAh','4294.967293 VA'],['204','Temporarily unavailable','0 W']]){
      const selecting=commands.length;await page.getByLabel('Energy bank',{exact:true}).selectOption(bank);await page.evaluate(()=>new Promise(requestAnimationFrame));
      assert.equal(commands.length,selecting,'changing energy bank sent a read');
      await page.getByRole('button',{name:'Read energy data',exact:true}).click();await counter('energy').getByText(expected,{exact:true}).waitFor();await idle();
      assert.deepEqual(commands.slice(selecting).map(c=>[c.command,c.params]),[['read_energy_data',{bank:Number(bank)}]]);
      assert.equal(await counter('power').innerText(),power);await energy().getByText('Snapshot confirmed and released.',{exact:true}).waitFor();
      await noOverflow();await dialogVisible();assert.ok(await page.locator('.dialog-body').evaluate(e=>e.scrollWidth<=e.clientWidth+1),'energy decimal overflows horizontally');
      if(bank==='203'){
        await page.getByText('Counter details',{exact:true}).click();await page.locator('.dialog-body').evaluate(e=>e.scrollTop=0);
        await page.screenshot({path:join(artifacts,`${viewport.width===390?'phone':'desktop'}-energy-data.png`),fullPage:true});
      }
    }
    await page.getByText('Captured bank 204 bytes',{exact:true}).click();assert.equal(await result().locator('tbody tr').count(),16);await result().locator('tbody tr').last().scrollIntoViewIfNeeded();assert.equal(await result().locator('tbody tr').last().locator('td').first().innerText(),'0x0F');await dialogVisible();
    let energyLostResolve;const energyLost=new Promise(r=>energyLostResolve=r);let loseEnergy=true;
    const loseEnergyReply=async route=>{const call=route.request().postDataJSON();if(loseEnergy&&call.command==='read_energy_data'){
      loseEnergy=false;const response=await route.fetch(),admission=await response.json();let job;
      await until(async()=>{job=await(await page.context().request.get(`${origin}/api/jobs/${admission.id}`)).json();return job.status!=='running';},'native energy capture did not finish');assert.equal(job.status,'succeeded');
      await route.fulfill({status:200,contentType:'application/json',body:JSON.stringify({error:'Energy result unavailable.'})});energyLostResolve();
    }else await route.continue();};
    await page.route(`${origin}/api/jobs`,loseEnergyReply);const lostEnergyStart=commands.length;
    try{await page.getByRole('button',{name:'Read energy data',exact:true}).click();await energyLost;await idle();assert.equal(await energy().count(),0,'lost energy reply retained previous data');assert.deepEqual(commands.slice(lostEnergyStart).map(c=>c.command),['read_energy_data'],'energy capture retried automatically');assert.ok(await page.getByRole('button',{name:'Read energy data',exact:true}).isVisible(),'energy controls collapsed after failed read');}
    finally{await page.unroute(`${origin}/api/jobs`,loseEnergyReply);}
    await page.getByRole('button',{name:'Read energy data',exact:true}).click();await counter('power').getByText('0 W',{exact:true}).waitFor();await idle();
    await page.getByText('Read a memory bank',{exact:true}).click();await page.getByLabel('Memory bank',{exact:true}).fill('255');await page.getByLabel('Memory byte count',{exact:true}).fill('4');
    const first=commands.length;await page.getByRole('button',{name:'Read memory',exact:true}).click();await until(async()=>await result().locator('tbody tr').count()===4,'raw memory read did not finish');await idle();
    assert.deepEqual(commands.slice(first).map(c=>c.command),['read_memory']);assert.deepEqual(await result().locator('tbody tr td:nth-child(2)').allTextContents(),['0xFF (255)','No response','0x00 (0)','0x7F (127)']);
    await noOverflow();await dialogVisible();await page.screenshot({path:join(artifacts,`${viewport.width===390?'phone':'desktop'}-memory-bytes.png`),fullPage:true});
    await page.getByLabel('First memory location',{exact:true}).fill('254');await page.getByLabel('Memory byte count',{exact:true}).fill('2');const count=commands.length;
    await page.getByRole('button',{name:'Read memory',exact:true}).click();await page.getByRole('alert').filter({hasText:'within locations'}).first().waitFor();assert.equal(commands.length,count,'invalid range reached HAT');assert.equal(await result().count(),0,'invalid new read retained prior evidence');
    await page.getByLabel('Memory bank',{exact:true}).fill('2');await page.getByLabel('First memory location',{exact:true}).fill('0');await page.getByLabel('Memory byte count',{exact:true}).fill('1');await page.getByRole('button',{name:'Read memory',exact:true}).click();await result().getByText('unavailable',{exact:true}).waitFor();await idle();await close();
    await open(plain);await result().getByText('89ABCDEF (32-bit)',{exact:true}).waitFor();await result().getByText('legacy',{exact:true}).waitFor();assert.equal(await result().locator('dd').nth(3).innerText(),'Unknown');
    await page.getByRole('button',{name:'Read luminaire data',exact:true}).click();await result().getByText('Unrecognized luminaire data format 0x0004. Raw bytes are available below.',{exact:true}).waitFor();await idle();
    await page.getByText('Captured bank 1 bytes',{exact:true}).click();assert.equal(await result().locator('tbody tr').count(),19,'unfamiliar data format read extension bytes');
    await page.getByText('Energy and power',{exact:true}).click();await page.getByRole('button',{name:'Read energy data',exact:true}).click();await result().getByText('Unrecognized energy bank version 2. Raw bytes are available below.',{exact:true}).waitFor();await idle();
    await page.getByText('Captured bank 202 bytes',{exact:true}).click();assert.equal(await result().locator('tbody tr').count(),4,'unknown energy version read extension');
    await page.getByLabel('Energy bank',{exact:true}).selectOption('203');await page.getByRole('button',{name:'Read energy data',exact:true}).click();await result().getByText('Energy data is unavailable from these readings.',{exact:true}).waitFor();await idle();await result().getByText('unavailable',{exact:true}).waitFor();
    await page.getByLabel('Energy bank',{exact:true}).selectOption('204');await page.getByRole('button',{name:'Read energy data',exact:true}).click();await result().getByText('partial',{exact:true}).waitFor();await idle();assert.equal(await page.locator('[data-energy-field="power"]').innerText(),'Unavailable');await close();
    await open(input);await result().getByText('unknown',{exact:true}).waitFor();assert.equal(await result().locator('dd').nth(1).innerText(),'Unknown');await result().getByText('partial',{exact:true}).waitFor();assert.equal(await page.getByRole('button',{name:'Read luminaire data',exact:true}).count(),0,'input module exposes luminaire data');assert.equal(await page.getByText('Energy and power',{exact:true}).count(),0,'input module exposes energy data');await close();
    const instance=bus.inputs[0].instances[0];await row(instance).getByRole('button',{name:`Actions ${instance.name}`,exact:true}).click();assert.equal(await page.getByRole('button',{name:'Device information',exact:true}).count(),0,'instance exposes module identity');await close();
  }
  // A held real HTTP reply must lose view authority when its dialog closes.
  let entered,release;const admitted=new Promise(r=>entered=r),held=new Promise(r=>release=r);let hold=true;
  const intercept=async route=>{const response=await route.fetch(),body=await response.json();if(hold&&body.result?.operation==='read_identity'){hold=false;entered();await held;}await route.fulfill({response}).catch(()=>{});};
  await page.route(`${origin}/api/jobs/*`,intercept);
  await row(white).getByRole('button',{name:`Actions ${white.name}`,exact:true}).click();await page.getByRole('button',{name:'Device information',exact:true}).click();await admitted;
  await page.getByRole('button',{name:'Close dialog',exact:true}).click();release();await idle();await page.unroute(`${origin}/api/jobs/*`,intercept);await open(plain);
  assert.equal(await result().getByText('FEDCBA9876543210 (64-bit)',{exact:true}).count(),0,'retired read populated another fixture');await close();
  await open(white);let dataEntered,dataRelease;const dataAdmitted=new Promise(r=>dataEntered=r),dataHeld=new Promise(r=>dataRelease=r);let holdData=true;
  const holdLuminaire=async route=>{const response=await route.fetch(),body=await response.json();if(holdData&&body.result?.operation==='read_luminaire_data'){holdData=false;dataEntered();await dataHeld;}await route.fulfill({response}).catch(()=>{});};
  await page.route(`${origin}/api/jobs/*`,holdLuminaire);await page.getByRole('button',{name:'Read luminaire data',exact:true}).click();await dataAdmitted;
  await page.getByRole('button',{name:'Close dialog',exact:true}).click();dataRelease();await idle();await page.unroute(`${origin}/api/jobs/*`,holdLuminaire);
  await open(plain);assert.equal(await page.locator('[data-luminaire-field]').count(),0,'retired luminaire read populated another fixture');await close();
  await open(white);await page.getByText('Energy and power',{exact:true}).click();let energyEntered,energyRelease;const energyAdmitted=new Promise(r=>energyEntered=r),energyHeld=new Promise(r=>energyRelease=r);let holdEnergy=true;
  const holdEnergyReply=async route=>{const response=await route.fetch(),body=await response.json();if(holdEnergy&&body.result?.operation==='read_energy_data'){holdEnergy=false;energyEntered();await energyHeld;}await route.fulfill({response}).catch(()=>{});};
  await page.route(`${origin}/api/jobs/*`,holdEnergyReply);await page.getByRole('button',{name:'Read energy data',exact:true}).click();await energyAdmitted;
  await page.getByRole('button',{name:'Close dialog',exact:true}).click();energyRelease();await idle();await page.unroute(`${origin}/api/jobs/*`,holdEnergyReply);
  await open(plain);assert.equal(await page.locator('[data-energy-field]').count(),0,'retired energy capture populated another fixture');await close();
  assert.deepEqual(await native(),before,'memory workflow changed native state other than scratch DTRs');assert.deepEqual(await inventory(),saved,'memory workflow changed saved names/bindings');
  await page.setViewportSize({width:390,height:844});
  console.log('PASS device information: phone/desktop identity, Part251 ratings and3 energy banks; exact decimals/units, snapshot release, TMASK/zero/partial/absent/unknown versions, no read on selection, escaped OEM text,120/16-byte scrolling, lost replies/explicit refresh, raw limits, retired dialogs and native/config preservation');
}
