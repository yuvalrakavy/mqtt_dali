import assert from 'node:assert/strict';
import { join } from 'node:path';

export async function reassignmentFlow({page,origin,artifacts,inventory,control,until,commands,noOverflow,dialogVisible,closeDialog}){
  const idle=()=>until(async()=>await page.getByRole('button',{name:'Add unaddressed',exact:true}).isEnabled(),'reassignment did not finish',90000);
  async function review(space){
    await page.getByRole('button',{name:'Bus actions',exact:true}).click();
    await page.getByRole('button',{name:'Reassign addresses',exact:true}).click();
    await page.getByLabel('Reassignment address space',{exact:true}).selectOption(space);
    await page.getByRole('button',{name:'Review reassignment',exact:true}).click();
    await page.getByRole('heading',{name:'Confirm address reassignment',exact:true}).waitFor();
  }
  function retained(native){return native.buses[0].outputs.map(o=>({id:o.id,level:o.level,groups:o.groups,scenes:o.scenes,actual:o.colour_state?.actual}));}
  const native=()=>control({op:'snapshot'});
  await page.setViewportSize({width:390,height:844});await page.goto(`${origin}/buses/0`);await idle();
  const before=await inventory(),physical=retained(await native()),names=before.buses[0].outputs.map(o=>o.name),count=commands.length;
  await review('outputs');await dialogVisible();assert.ok((await page.locator('dialog').innerText()).includes('BrowserController/Bus1'));
  await page.locator('dialog summary').click();for(const name of names)assert.ok((await page.locator('dialog').innerText()).includes(name));
  await page.screenshot({path:join(artifacts,'phone-reassignment-review.png'),fullPage:true});await noOverflow();
  await page.getByRole('button',{name:'Cancel',exact:true}).click();assert.equal(commands.length,count,'cancelled reset preview submitted work');assert.deepEqual((await inventory()).buses[0],before.buses[0]);
  await review('outputs');const start=commands.length;await page.getByRole('button',{name:'Clear and reassign',exact:true}).click();await idle();
  assert.ok(commands.slice(start).length>64,'reset did not perform assignment and addressed refresh');
  assert.ok(commands.slice(start).every(c=>!c.command.includes('input')&&c.params.space!=='inputs'),'outputs-only reset touched input space');
  let saved=await inventory();assert.deepEqual(saved.buses[0].outputs.map(o=>o.name),['Light1','Light2','Light3']);assert.deepEqual(saved.buses[0].inputs,before.buses[0].inputs,'outputs reset changed saved input identities');
  assert.ok(saved.buses[0].groups.every(g=>g.members.length===0),'retired output remained in saved group');assert.ok(saved.buses[0].scenes.every(s=>!s.contents?.length),'retired output remained in private scene intent');assert.deepEqual(retained(await native()),physical,'address reset changed live level, colour, native groups or scenes');
  await page.screenshot({path:join(artifacts,'phone-reassigned-devices.png'),fullPage:true});await noOverflow();
  await page.setViewportSize({width:1440,height:1024});await review('inputs');await page.screenshot({path:join(artifacts,'desktop-reassignment-review.png'),fullPage:false});
  const outputs=saved.buses[0].outputs;await page.getByRole('button',{name:'Clear and reassign',exact:true}).click();await idle();saved=await inventory();assert.deepEqual(saved.buses[0].outputs,outputs,'inputs reset changed saved outputs');assert.equal(saved.buses[0].inputs.length,1);assert.equal(saved.buses[0].inputs[0].instances.length,4);assert.ok(!saved.buses[0].inputs[0].instances.some(i=>i.name==='DoorButton'),'input reset retained retired instance name');assert.deepEqual(retained(await native()),physical);
  await page.setViewportSize({width:390,height:844});await review('both');await dialogVisible();const bothStart=commands.length;await page.getByRole('button',{name:'Clear and reassign',exact:true}).click();await idle();
  const both=commands.slice(bothStart),resets=both.filter(c=>c.command==='reset_address_space');assert.deepEqual(resets.map(c=>c.params.space),['outputs','inputs']);
  const second=both.findIndex(c=>c===resets[1]);assert.ok(both.slice(0,second).filter(c=>c.command==='discover_address'&&c.params.space==='outputs').length>=64,'both reset started inputs before completing output refresh');
  assert.deepEqual(retained(await native()),physical);assert.equal((await inventory()).buses[0].outputs.length,3);assert.equal((await inventory()).buses[0].inputs[0].instances.length,4);
  // Hold an admitted reset response, navigate to another tab, then release it.
  // Retirement/clear may finish; lifetime cancellation must stop assignment.
  let entered,release;const admitted=new Promise(r=>entered=r),held=new Promise(r=>release=r);let hold=true;
  await page.route(`${origin}/api/jobs`,async route=>{if(hold&&route.request().postDataJSON().command==='reset_address_space'){hold=false;const response=await route.fetch();entered();await held;await route.fulfill({response}).catch(()=>{});}else await route.continue();});
  try{await review('outputs');await page.getByRole('button',{name:'Clear and reassign',exact:true}).click();await admitted;const stopAt=commands.length;await page.getByRole('link',{name:'Groups',exact:true}).click();release();await until(async()=>await page.getByRole('button',{name:'Bus actions',exact:true}).isEnabled(),'bus header disappeared');await new Promise(r=>setTimeout(r,200));assert.equal(commands.length,stopAt,'reset continued assignment after leaving its tab');}
  finally{release?.();await page.unroute(`${origin}/api/jobs`);}
  await page.getByRole('link',{name:'Devices',exact:true}).click();await idle();
  console.log('PASS reassignment: cancelled frozen review, outputs/inputs/both, native preservation, fresh names, retirement pruning, phone/desktop, tab cancellation');
}
