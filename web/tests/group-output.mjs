import assert from 'node:assert/strict';
import {join} from 'node:path';

export async function groupOutputFlow({page,context,origin,artifacts,inventory,control,until,commands,noOverflow}){
  const saved=await inventory(),bus=saved.buses[0];
  const native=async()=>(await control({op:'snapshot'})).buses[0].outputs,before=await native();
  const selected=()=>page.getByRole('region',{name:'Group power and dimming',exact:true});
  const idle=()=>until(async()=>await selected().getByRole('button',{name:'Read group values',exact:true}).isEnabled(),'group operation did not settle');
  const slot=async n=>{await page.locator('.slots button').nth(n).click();await idle();};
  const read=async()=>{await selected().getByRole('button',{name:'Read group values',exact:true}).click();await idle();};
  const members=before.filter(o=>(o.groups&1)!==0);assert.equal(members.length,2);
  const job=async(address,command,params)=>{
    const session=await(await context.request.get(`${origin}/api/session`)).json();const current=await(await context.request.get(`${origin}/api/snapshot`)).json(),inventory=current.inventory.Inventory;
    const item=inventory.buses[0].outputs.find(o=>o.short_address===address);assert.ok(item,'fixture setup address has no public inventory projection');
    const admission=await context.request.post(`${origin}/api/jobs`,{headers:{Origin:origin,'X-CSRF-Token':session.csrf},data:{target:item.target,command,params,generation:`${inventory.inventory_epoch}.${item.binding_revision}`}});
    assert.equal(admission.status(),202);const {id}=await admission.json();let result;
    await until(async()=>{result=await(await context.request.get(`${origin}/api/jobs/${id}`)).json();return result.status!=='running';},'external fixture operation did not settle');
    assert.equal(result.status,'succeeded');return result.result;
  };
  const levels=async()=>Object.fromEntries((await native()).map(o=>[o.id,o.level]));
  const expectedMembers=async raw=>{const actual=await native();for(const fixture of actual)assert.equal(fixture.level,(fixture.groups&1)!==0?raw:before.find(o=>o.id===fixture.id).level,'native group destination or unselected fixture level changed');};
  for(const viewport of [{width:390,height:844},{width:1440,height:1024}]){
    await page.setViewportSize(viewport);await page.goto(`${origin}/buses/0/groups`);await idle();await slot(0);
    for(let i=0;i<members.length;i++){const fixture=members[i];await job(fixture.short_address,'apply_output',{values:{Brightness:i?0.8:0.2}});}
    const start=commands.length;await read();assert.deepEqual(commands.slice(start).map(c=>c.command),['read_group_output']);
    const toggle=selected().getByRole('button',{name:'Group power',exact:true});assert.equal(await toggle.getAttribute('aria-pressed'),'true');await selected().getByText('Brightness: Mixed',{exact:true}).waitFor();
    await noOverflow();await page.screenshot({path:join(artifacts,`${viewport.width===390?'phone':'desktop'}-group-output-mixed.png`),fullPage:true});
    await toggle.click();await idle();await expectedMembers(0);assert.equal(await toggle.getAttribute('aria-pressed'),'false');
    await toggle.click();await idle();await expectedMembers(254);assert.equal(await toggle.getAttribute('aria-pressed'),'true');
    const range=selected().getByRole('slider',{name:'Group level command',exact:true});await range.fill('27');await selected().getByRole('button',{name:'Set group level',exact:true}).click();await idle();await expectedMembers(69);await selected().getByText('Brightness: 27%',{exact:true}).waitFor();
    assert.deepEqual(await inventory(),saved,'group power/dimming changed saved names or assignments');
    // Missing an address leaves the group unknown; it cannot fabricate off.
    const missing=members[0];await control({op:'set_address',id:missing.id,short_address:null});
    try{await read();assert.equal(await toggle.count(),0);await selected().getByText('Brightness: Unknown',{exact:true}).waitFor();assert.ok((await selected().innerText()).includes('Some fixtures could not be read.'));await noOverflow();await page.screenshot({path:join(artifacts,`${viewport.width===390?'phone':'desktop'}-group-output-partial.png`),fullPage:true});}
    finally{await control({op:'set_address',id:missing.id,short_address:missing.short_address});}
    // Unknown is not a prerequisite failure for an explicit hardware command.
    const count=commands.length;await selected().getByRole('button',{name:'Group off',exact:true}).click();await idle();await expectedMembers(0);assert.deepEqual(commands.slice(count).map(c=>c.command),['set_group_output']);
    await toggle.click();await idle();await expectedMembers(254);
  }
  // A real read finishing after slot/navigation retirement cannot populate another view.
  for(const move of ['slot','navigate']){
    let entered,release;const admitted=new Promise(r=>entered=r),held=new Promise(r=>release=r);let hold=true;
    const route=async request=>{if(hold&&request.request().postDataJSON().command==='read_group_output'){hold=false;const response=await request.fetch();entered();await held;await request.fulfill({response}).catch(()=>{});}else await request.continue();};
    await page.route('**/api/jobs',route);await selected().getByRole('button',{name:'Read group values',exact:true}).click();await admitted;const count=commands.length;
    if(move==='slot')await page.locator('.slots button').nth(3).click();else await page.getByRole('link',{name:'Devices',exact:true}).click();
    release();await until(async()=>await page.getByRole('button',{name:'Bus actions',exact:true}).isEnabled(),'retired group read did not settle');await page.unroute('**/api/jobs',route);
    assert.equal(commands.slice(count).filter(c=>c.command==='set_group_output').length,0,'retired group read submitted an action');
    if(move==='slot'){await idle();assert.equal(await selected().getByRole('button',{name:'Group power',exact:true}).count(),0,'old group reading populated a different slot');}
    await page.goto(`${origin}/buses/0/groups`);await idle();await slot(0);
  }
  for(const fixture of before){await job(fixture.short_address,'apply_output',{values:{Brightness:fixture.level/254}});}
  assert.deepEqual(await levels(),Object.fromEntries(before.map(o=>[o.id,o.level])));assert.deepEqual(await inventory(),saved);
  await page.setViewportSize({width:390,height:844});await page.goto(`${origin}/buses/0/groups`);await idle();
  console.log('PASS group output: phone/desktop native on/off/level, mixed levels with uniform power, missing-address unknown, explicit command without prior state, other fixtures/config preserved, retired slot/navigation reads and actual restoration');
}
