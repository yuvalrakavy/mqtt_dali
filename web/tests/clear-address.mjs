import assert from 'node:assert/strict';
import { join } from 'node:path';

export async function clearAddressFlow({page,origin,artifacts,inventory,control,until,commands,noOverflow,dialogVisible}){
  const idle=()=>until(async()=>await page.getByRole('button',{name:'Add unaddressed',exact:true}).isEnabled(),'individual clear did not settle');
  const row=name=>page.locator('.device-row').filter({has:page.getByRole('textbox',{name:`Name ${name}`,exact:true})});
  const open=async(name,action)=>{await row(name).getByRole('button',{name:`Actions ${name}`,exact:true}).click();await page.getByRole('button',{name:action,exact:true}).click();};
  const native=()=>control({op:'snapshot'});
  const before=await inventory(),output=before.buses[0].outputs.find(o=>o.short_address===0),input=before.buses[0].inputs[0];
  await page.clock.install();await page.setViewportSize({width:390,height:844});await page.goto(`${origin}/buses/0`);await idle();
  await open(output.name,'Clear short address');await dialogVisible();await noOverflow();const count=commands.length;
  await page.screenshot({path:join(artifacts,'phone-clear-address-review.png'),fullPage:true});
  await page.getByRole('button',{name:'Cancel',exact:true}).click();assert.equal(commands.length,count,'cancelled address clear submitted a command');
  // Exercise actual dismissal and route hooks while passive cleanup cannot run.
  // The Stop command finishes on the server; only its admission reply is held.
  for(const dismissal of ['cancel','close','navigate']){
    await page.goto(`${origin}/buses/0`);await idle();
    await row(output.name).getByRole('button',{name:`Blink ${output.name}`,exact:true}).click();await page.getByRole('button',{name:`Stop blink ${output.name}`,exact:true}).waitFor();await open(output.name,'Clear short address');
    let entered,release,id;const admitted=new Promise(r=>entered=r),held=new Promise(r=>release=r);let hold=true;
    await page.route(`${origin}/api/jobs`,async route=>{if(hold&&route.request().postDataJSON().command==='stop_blink'){hold=false;const response=await route.fetch();id=(await response.json()).id;entered();await held;await route.fulfill({response}).catch(()=>{});}else await route.continue();});
    try{
      await page.getByRole('button',{name:'Stop blink and clear address',exact:true}).click();await admitted;
      await until(async()=>{const result=await page.context().request.get(`${origin}/api/jobs/${id}`);return (await result.json()).status==='succeeded';},'held Stop did not finish');
      await page.clock.pauseAt(new Date(await page.evaluate(()=>Date.now()+100)));
      const stopped=commands.length;
      if(dismissal==='navigate')await page.evaluate(()=>{history.pushState(null,'','/buses/0/groups');dispatchEvent(new PopStateEvent('popstate'));});
      else await page.getByRole('button',{name:dismissal==='cancel'?'Cancel':'Close dialog',exact:true}).dispatchEvent('click');
      await until(async()=>await page.locator('dialog').count()===0,'clear dialog remained after dismissal');release();
      await until(async()=>commands.length>stopped||(await page.getByRole('alert').allTextContents()).some(t=>t.includes('Address recovery stopped')),'dismissed clear did not finish its held continuation');
      assert.equal(commands.length,stopped,`individual clear submitted after ${dismissal} while passive cleanup was paused`);
      assert.equal((await native()).buses[0].outputs.find(o=>o.id==='white').short_address,0,'dismissed clear changed hardware');
    }finally{release?.();await page.clock.resume();await page.unroute(`${origin}/api/jobs`);}
  }
  await page.goto(`${origin}/buses/0`);await idle();
  for(const [item,space,id] of [[output,'outputs','white'],[input,'inputs','buttons']]){
    if(space==='inputs')await page.setViewportSize({width:1440,height:1024});
    const physical=(await native()).buses[0][space].find(o=>o.id===id);
    if(space==='outputs'){await row(item.name).getByRole('button',{name:`Blink ${item.name}`,exact:true}).click();await page.getByRole('button',{name:`Stop blink ${item.name}`,exact:true}).waitFor();}
    await open(item.name,'Clear short address');if(space==='inputs')await page.screenshot({path:join(artifacts,'desktop-clear-input-review.png'),fullPage:false});
    const start=commands.length;
    await page.getByRole('button',{name:space==='outputs'?'Stop blink and clear address':'Clear address',exact:true}).click();
    await until(async()=>await page.locator('dialog').count()===0,'individual clear confirmation did not close');await idle();
    assert.deepEqual(commands.slice(start).map(c=>c.command),space==='outputs'?['stop_blink','clear_short_address']:['clear_short_address'],'clear did not stop Blink first or submitted automatic follow-up work');
    assert.deepEqual(await inventory(),before,'individual clear changed saved names, bindings or reservations');
    const cleared=(await native()).buses[0][space].find(o=>o.id===id);assert.equal(cleared.short_address,null,'individual clear left its short address programmed');
    for(const key of space==='outputs'?['level','groups','scenes','colour_state']:['instances','values'])assert.deepEqual(cleared[key],physical[key],`clear changed native ${key}`);
    await open(item.name,'Restore lost address');await page.getByRole('button',{name:'Restore sole unaddressed fixture',exact:true}).click();await page.getByRole('button',{name:'Restore saved address',exact:true}).click();await idle();
    assert.equal((await native()).buses[0][space].find(o=>o.id===id).short_address,0,'preserved reservation could not restore the cleared fixture');assert.deepEqual(await inventory(),before);
  }
  console.log('PASS individual clear: cancelled phone review, confirmed Blink stop, addressed output/input clear, unchanged saved identities/native settings, no automatic rediscovery, explicit recovery');
}
